use super::session_policy::VisionPolicy;
use super::snapshots::union_events;
use crate::protocol::{Event, Snapshot};
use rts_sim::game::Game;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecipientRole {
    ActivePlayer,
    Spectator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SnapshotProjection {
    PlayerFog { player_id: u32 },
    SpectatorUnion { player_ids: Vec<u32> },
    ReplayVision { player_ids: Vec<u32> },
    FullWorld { player_id: u32 },
}

impl SnapshotProjection {
    pub(super) fn snapshot_with_events(
        &self,
        game: &Game,
        per_player_events: &mut HashMap<u32, Vec<Event>>,
        full_vision_events: &[Event],
    ) -> Snapshot {
        let mut snapshot = match self {
            SnapshotProjection::PlayerFog { player_id } => game.snapshot_for(*player_id),
            SnapshotProjection::SpectatorUnion { player_ids } => {
                game.snapshot_for_spectator(player_ids)
            }
            SnapshotProjection::ReplayVision { player_ids } => game.snapshot_for_replay(player_ids),
            SnapshotProjection::FullWorld { player_id } => game.snapshot_full_for(*player_id),
        };

        match self {
            SnapshotProjection::PlayerFog { player_id }
            | SnapshotProjection::FullWorld { player_id } => {
                if let Some(mut events) = per_player_events.remove(player_id) {
                    snapshot.events.append(&mut events);
                }
            }
            SnapshotProjection::SpectatorUnion { .. } => {
                snapshot.events.extend(full_vision_events.to_vec());
            }
            SnapshotProjection::ReplayVision { player_ids } => {
                snapshot.events.extend(union_events(
                    player_ids
                        .iter()
                        .filter_map(|player_id| per_player_events.get(player_id)),
                ));
            }
        }

        snapshot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObserverAnalysisAudience {
    None,
    LiveSpectators,
    ReplayViewers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProjectionPolicy {
    vision: VisionPolicy,
}

impl ProjectionPolicy {
    pub(super) fn new(vision: VisionPolicy) -> Self {
        Self { vision }
    }

    pub(super) fn live_snapshot_for(
        self,
        role: RecipientRole,
        connection_id: u32,
        seat_id: Option<u32>,
        spectator_visible_player_ids: &[u32],
    ) -> SnapshotProjection {
        if self.vision == VisionPolicy::LabFullWorld {
            return SnapshotProjection::FullWorld {
                player_id: seat_id.unwrap_or(connection_id),
            };
        }
        match role {
            RecipientRole::ActivePlayer => SnapshotProjection::PlayerFog {
                player_id: seat_id.unwrap_or(connection_id),
            },
            RecipientRole::Spectator => SnapshotProjection::SpectatorUnion {
                player_ids: spectator_visible_player_ids.to_vec(),
            },
        }
    }

    pub(super) fn replay_snapshot_for(self, visible_player_ids: Vec<u32>) -> SnapshotProjection {
        SnapshotProjection::ReplayVision {
            player_ids: visible_player_ids,
        }
    }

    pub(super) fn dev_snapshot_for(self, view_player_id: u32) -> SnapshotProjection {
        SnapshotProjection::FullWorld {
            player_id: view_player_id,
        }
    }

    pub(super) fn observer_analysis_audience(self) -> ObserverAnalysisAudience {
        match self.vision {
            VisionPolicy::LiveFog => ObserverAnalysisAudience::LiveSpectators,
            VisionPolicy::ReplayVision => ObserverAnalysisAudience::ReplayViewers,
            VisionPolicy::LobbyState
            | VisionPolicy::BranchStagingState
            | VisionPolicy::DevFullWorld
            | VisionPolicy::LabFullWorld => ObserverAnalysisAudience::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_policy_classifies_live_players_spectators_and_branch_aliases() {
        let policy = ProjectionPolicy::new(VisionPolicy::LiveFog);

        assert_eq!(
            policy.live_snapshot_for(RecipientRole::ActivePlayer, 10, None, &[1, 2]),
            SnapshotProjection::PlayerFog { player_id: 10 }
        );
        assert_eq!(
            policy.live_snapshot_for(RecipientRole::ActivePlayer, 100, Some(1), &[1, 2]),
            SnapshotProjection::PlayerFog { player_id: 1 }
        );
        assert_eq!(
            policy.live_snapshot_for(RecipientRole::Spectator, 102, None, &[1, 2]),
            SnapshotProjection::SpectatorUnion {
                player_ids: vec![1, 2],
            }
        );
        assert_eq!(
            policy.observer_analysis_audience(),
            ObserverAnalysisAudience::LiveSpectators
        );
    }

    #[test]
    fn projection_policy_classifies_replay_and_dev_snapshots() {
        let replay = ProjectionPolicy::new(VisionPolicy::ReplayVision);
        assert_eq!(
            replay.replay_snapshot_for(vec![2]),
            SnapshotProjection::ReplayVision {
                player_ids: vec![2],
            }
        );
        assert_eq!(
            replay.observer_analysis_audience(),
            ObserverAnalysisAudience::ReplayViewers
        );

        let dev = ProjectionPolicy::new(VisionPolicy::DevFullWorld);
        assert_eq!(
            dev.dev_snapshot_for(7),
            SnapshotProjection::FullWorld { player_id: 7 }
        );
        assert_eq!(
            dev.observer_analysis_audience(),
            ObserverAnalysisAudience::None
        );
    }

    #[test]
    fn projection_policy_classifies_lab_as_full_world_without_analysis() {
        let lab = ProjectionPolicy::new(VisionPolicy::LabFullWorld);
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::Spectator, 99, Some(1), &[1, 2]),
            SnapshotProjection::FullWorld { player_id: 1 }
        );
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::ActivePlayer, 99, None, &[1, 2]),
            SnapshotProjection::FullWorld { player_id: 99 }
        );
        assert_eq!(
            lab.observer_analysis_audience(),
            ObserverAnalysisAudience::None
        );
    }
}
