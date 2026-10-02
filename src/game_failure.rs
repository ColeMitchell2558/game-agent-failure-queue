use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Workload {
    PlayerAsset { asset_kind: String },
    LiveEvent { event_kind: String },
    ModerationQueue { policy: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentFailure {
    pub failure_id: String,
    pub agent: String,
    pub stage: String,
    pub player_id: String,
    pub workload: Workload,
    pub exception: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    HoldForModeration,
    ReplayLiveEvent,
    QuarantineQueueItem,
}

impl fmt::Display for RecoveryAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::HoldForModeration => "hold_for_moderation",
            Self::ReplayLiveEvent => "replay_live_event",
            Self::QuarantineQueueItem => "quarantine_queue_item",
        };
        f.write_str(value)
    }
}

impl AgentFailure {
    pub fn recovery_action(&self) -> RecoveryAction {
        match self.workload {
            Workload::PlayerAsset { .. } => RecoveryAction::HoldForModeration,
            Workload::LiveEvent { .. } => RecoveryAction::ReplayLiveEvent,
            Workload::ModerationQueue { .. } => RecoveryAction::QuarantineQueueItem,
        }
    }

    pub fn grouping_key(&self) -> String {
        let workload = match &self.workload {
            Workload::PlayerAsset { asset_kind } => format!("asset:{asset_kind}"),
            Workload::LiveEvent { event_kind } => format!("live:{event_kind}"),
            Workload::ModerationQueue { policy } => format!("moderation:{policy}"),
        };
        format!("{}:{}:{workload}", self.agent, self.stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(workload: Workload) -> AgentFailure {
        AgentFailure {
            failure_id: "failure-2026-10-01-0017".into(),
            agent: "ugc-safety-agent".into(),
            stage: "classify".into(),
            player_id: "player-17".into(),
            workload,
            exception: "classifier response could not be applied".into(),
        }
    }

    #[test]
    fn routes_failures_by_game_workload() {
        let asset = failure(Workload::PlayerAsset {
            asset_kind: "guild-emblem".into(),
        });
        let live = failure(Workload::LiveEvent {
            event_kind: "world-boss-spawn".into(),
        });
        let moderation = failure(Workload::ModerationQueue {
            policy: "player-generated-image".into(),
        });

        assert_eq!(asset.recovery_action(), RecoveryAction::HoldForModeration);
        assert_eq!(live.recovery_action(), RecoveryAction::ReplayLiveEvent);
        assert_eq!(
            moderation.recovery_action(),
            RecoveryAction::QuarantineQueueItem
        );
        assert_eq!(
            asset.grouping_key(),
            "ugc-safety-agent:classify:asset:guild-emblem"
        );
    }
}
