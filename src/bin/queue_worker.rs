use game_agent_failure_queue::{
    game_failure::{AgentFailure, Workload},
    infrai_client::{CaptureError, InfraiClient},
};

#[derive(Debug)]
enum WorkerError {
    Capture(CaptureError),
}

impl From<CaptureError> for WorkerError {
    fn from(value: CaptureError) -> Self {
        Self::Capture(value)
    }
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capture(error) => write!(f, "capture step failed: {error}"),
        }
    }
}

impl std::error::Error for WorkerError {}

#[tokio::main]
async fn main() -> Result<(), WorkerError> {
    let failure = AgentFailure {
        failure_id: "failure-2026-10-01-0017".into(),
        agent: "ugc-safety-agent".into(),
        stage: "classify".into(),
        player_id: "player-17".into(),
        workload: Workload::PlayerAsset {
            asset_kind: "guild-emblem".into(),
        },
        exception: "classifier response could not be applied".into(),
    };

    let action = failure.recovery_action();
    InfraiClient::from_env()?.capture_failure(&failure).await?;
    println!("captured={} action={action}", failure.grouping_key());
    Ok(())
}
