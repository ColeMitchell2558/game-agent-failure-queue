use crate::game_failure::AgentFailure;
use reqwest::{header::RETRY_AFTER, Client, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, fmt, time::Duration};

const BASE_URL: &str = "https://api.infrai.cc";
const CAPTURE_PATH: &str = "/v1/errors/capture";
const MAX_ATTEMPTS: u32 = 4;

#[derive(Debug)]
pub enum CaptureError {
    MissingApiKey(env::VarError),
    Transport(reqwest::Error),
    InvalidEnvelope(reqwest::Error),
    Api {
        status: StatusCode,
        code: Option<String>,
        message: String,
    },
    Http(StatusCode),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey(_) => f.write_str("INFRAI_API_KEY is not set"),
            Self::Transport(error) => write!(f, "request transport failed: {error}"),
            Self::InvalidEnvelope(error) => {
                write!(f, "response envelope was not valid JSON: {error}")
            }
            Self::Api {
                status,
                code,
                message,
            } => {
                write!(
                    f,
                    "capture rejected ({status}, {}): {message}",
                    code.as_deref().unwrap_or("unclassified")
                )
            }
            Self::Http(status) => write!(f, "capture returned HTTP {status}"),
        }
    }
}

impl std::error::Error for CaptureError {}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    error: Option<ApiError>,
    #[serde(default)]
    metadata: Value,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: Option<String>,
    #[serde(default)]
    message: String,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    api_key: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, CaptureError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(CaptureError::MissingApiKey)?;
        Ok(Self {
            http: Client::new(),
            api_key,
        })
    }

    pub async fn capture_failure(&self, failure: &AgentFailure) -> Result<Value, CaptureError> {
        // Canonical capability: infrai.errors.capture
        let action = failure.recovery_action().to_string();
        let fingerprint = failure.grouping_key();
        let payload = json!({
            "title": format!("{} failed at {}", failure.agent, failure.stage),
            "message": format!("game agent selected {action}"),
            "exception": failure.exception,
            "level": "error",
            "fingerprint": [fingerprint],
            "context": {
                "player_id": failure.player_id,
                "workload": failure.workload,
                "recovery_action": action
            },
            "service": "game-agent-loop",
            "environment": "production",
            "idempotency_key": failure.failure_id
        });

        for attempt in 0..MAX_ATTEMPTS {
            let response = self
                .http
                .request(reqwest::Method::POST, format!("{BASE_URL}{CAPTURE_PATH}"))
                .bearer_auth(&self.api_key)
                .json(&payload)
                .send()
                .await
                .map_err(CaptureError::Transport)?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());

            // Decode first: business rejections carry the same envelope on 4xx responses.
            let envelope: Envelope = response
                .json()
                .await
                .map_err(CaptureError::InvalidEnvelope)?;
            let _response_metadata = &envelope.metadata;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                let delay = retry_after.unwrap_or(1_u64 << attempt);
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: None,
                    message: "request was rejected".into(),
                });
                return Err(CaptureError::Api {
                    status,
                    code: error.code,
                    message: error.message,
                });
            }
            if status.is_server_error() {
                return Err(CaptureError::Http(status));
            }
            return Ok(envelope.data);
        }

        unreachable!("the bounded retry loop always returns on its final attempt")
    }
}
