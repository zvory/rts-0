use super::session_policy::{
    DiagnosticPolicy, MovementPathDiagnosticPolicy, ObserverAnalysisPolicy, VisibilityPolicy,
};
use super::snapshots::union_events;
use crate::protocol::{DiagnosticCapabilities, Event, MovementPathDiagnosticScope, Snapshot};
use rts_sim::game::{Game, SnapshotOptions};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecipientRole {
    ActivePlayer,
    Spectator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SnapshotBodyProjection {
    PlayerFog {
        player_id: u32,
        options: SnapshotOptions,
    },
    SpectatorUnion {
        player_ids: Vec<u32>,
        options: SnapshotOptions,
    },
    ReplayVision {
        player_ids: Vec<u32>,
        options: SnapshotOptions,
    },
    FullWorld {
        player_id: u32,
        options: SnapshotOptions,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EventProjection {
    PlayerOnly { player_id: u32 },
    FullVision,
    PlayerUnion { player_ids: Vec<u32> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SnapshotProjection {
    body: SnapshotBodyProjection,
    events: EventProjection,
}

impl SnapshotProjection {
    fn player_fog(player_id: u32, options: SnapshotOptions) -> Self {
        Self {
            body: SnapshotBodyProjection::PlayerFog { player_id, options },
            events: EventProjection::PlayerOnly { player_id },
        }
    }

    fn spectator_union(player_ids: Vec<u32>, options: SnapshotOptions) -> Self {
        Self {
            body: SnapshotBodyProjection::SpectatorUnion {
                player_ids,
                options,
            },
            events: EventProjection::FullVision,
        }
    }

    fn replay_vision(player_ids: Vec<u32>, options: SnapshotOptions) -> Self {
        Self {
            body: SnapshotBodyProjection::ReplayVision {
                player_ids: player_ids.clone(),
                options,
            },
            events: EventProjection::PlayerUnion { player_ids },
        }
    }

    fn full_world(player_id: u32, options: SnapshotOptions) -> Self {
        Self {
            body: SnapshotBodyProjection::FullWorld { player_id, options },
            events: EventProjection::PlayerOnly { player_id },
        }
    }

    pub(super) fn with_event_projection(mut self, events: EventProjection) -> Self {
        self.events = events;
        self
    }

    pub(super) fn snapshot_with_events(
        &self,
        game: &Game,
        per_player_events: &mut HashMap<u32, Vec<Event>>,
        full_vision_events: &[Event],
    ) -> Snapshot {
        let mut snapshot = match &self.body {
            SnapshotBodyProjection::PlayerFog { player_id, options } => {
                game.snapshot_for_with_options(*player_id, *options)
            }
            SnapshotBodyProjection::SpectatorUnion {
                player_ids,
                options,
            }
            | SnapshotBodyProjection::ReplayVision {
                player_ids,
                options,
            } => game.snapshot_for_spectator_with_options(player_ids, *options),
            SnapshotBodyProjection::FullWorld { player_id, options } => {
                game.snapshot_full_for_with_options(*player_id, *options)
            }
        };

        match &self.events {
            EventProjection::PlayerOnly { player_id } => {
                if let Some(mut events) = per_player_events.remove(player_id) {
                    snapshot.events.append(&mut events);
                }
            }
            EventProjection::FullVision => {
                snapshot.events.extend(full_vision_events.to_vec());
            }
            EventProjection::PlayerUnion { player_ids } => {
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
    visibility: VisibilityPolicy,
    diagnostics: DiagnosticPolicy,
}

impl ProjectionPolicy {
    pub(super) fn new(visibility: VisibilityPolicy, diagnostics: DiagnosticPolicy) -> Self {
        Self {
            visibility,
            diagnostics,
        }
    }

    pub(super) fn live_snapshot_for(
        self,
        role: RecipientRole,
        connection_id: u32,
        seat_id: Option<u32>,
        spectator_visible_player_ids: &[u32],
    ) -> SnapshotProjection {
        if self.visibility == VisibilityPolicy::LabFullWorld {
            return SnapshotProjection::full_world(
                seat_id.unwrap_or(connection_id),
                self.snapshot_options_for(role),
            );
        }
        match role {
            RecipientRole::ActivePlayer => SnapshotProjection::player_fog(
                seat_id.unwrap_or(connection_id),
                self.snapshot_options_for(role),
            ),
            RecipientRole::Spectator => SnapshotProjection::spectator_union(
                spectator_visible_player_ids.to_vec(),
                self.snapshot_options_for(role),
            ),
        }
    }

    pub(super) fn replay_snapshot_for(self, visible_player_ids: Vec<u32>) -> SnapshotProjection {
        SnapshotProjection::replay_vision(
            visible_player_ids,
            self.snapshot_options_for(RecipientRole::Spectator),
        )
    }

    pub(super) fn dev_snapshot_for(self, view_player_id: u32) -> SnapshotProjection {
        SnapshotProjection::full_world(
            view_player_id,
            self.snapshot_options_for(RecipientRole::Spectator),
        )
    }

    pub(super) fn observer_analysis_audience(self) -> ObserverAnalysisAudience {
        match self.diagnostics.observer_analysis {
            ObserverAnalysisPolicy::LiveSpectators => ObserverAnalysisAudience::LiveSpectators,
            ObserverAnalysisPolicy::ReplayViewers => ObserverAnalysisAudience::ReplayViewers,
            ObserverAnalysisPolicy::None => ObserverAnalysisAudience::None,
        }
    }

    pub(super) fn diagnostic_capabilities_for(self, role: RecipientRole) -> DiagnosticCapabilities {
        DiagnosticCapabilities {
            movement_paths: self.movement_path_scope_for(role),
            observer_analysis: matches!(
                (self.diagnostics.observer_analysis, role),
                (
                    ObserverAnalysisPolicy::LiveSpectators,
                    RecipientRole::Spectator
                ) | (ObserverAnalysisPolicy::ReplayViewers, _)
            ),
        }
    }

    fn snapshot_options_for(self, role: RecipientRole) -> SnapshotOptions {
        match self.movement_path_scope_for(role) {
            MovementPathDiagnosticScope::None => SnapshotOptions::default(),
            MovementPathDiagnosticScope::OwnerOnly => SnapshotOptions {
                include_movement_paths: true,
                movement_paths_for_all_projected: false,
            },
            MovementPathDiagnosticScope::All => SnapshotOptions {
                include_movement_paths: true,
                movement_paths_for_all_projected: true,
            },
        }
    }

    fn movement_path_scope_for(self, role: RecipientRole) -> MovementPathDiagnosticScope {
        match (self.diagnostics.movement_paths, role) {
            (MovementPathDiagnosticPolicy::None, _) => MovementPathDiagnosticScope::None,
            (MovementPathDiagnosticPolicy::AllProjected, _) => MovementPathDiagnosticScope::All,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_policy_classifies_live_players_spectators_and_branch_aliases() {
        let policy = ProjectionPolicy::new(
            VisibilityPolicy::LiveFog,
            DiagnosticPolicy::LIVE_SPECTATOR_OBSERVER_ANALYSIS,
        );

        assert_eq!(
            policy.live_snapshot_for(RecipientRole::ActivePlayer, 10, None, &[1, 2]),
            SnapshotProjection::player_fog(10, SnapshotOptions::default())
        );
        assert_eq!(
            policy.live_snapshot_for(RecipientRole::ActivePlayer, 100, Some(1), &[1, 2]),
            SnapshotProjection::player_fog(1, SnapshotOptions::default())
        );
        assert_eq!(
            policy.live_snapshot_for(RecipientRole::Spectator, 102, None, &[1, 2]),
            SnapshotProjection::spectator_union(vec![1, 2], SnapshotOptions::default())
        );
        assert_eq!(
            policy.observer_analysis_audience(),
            ObserverAnalysisAudience::LiveSpectators
        );
        assert!(
            policy
                .diagnostic_capabilities_for(RecipientRole::Spectator)
                .observer_analysis
        );
        assert!(
            !policy
                .diagnostic_capabilities_for(RecipientRole::ActivePlayer)
                .observer_analysis
        );
    }

    #[test]
    fn projection_policy_classifies_replay_and_dev_snapshots() {
        let replay = ProjectionPolicy::new(
            VisibilityPolicy::ReplayVision,
            DiagnosticPolicy::REPLAY_OBSERVER_ANALYSIS,
        );
        assert_eq!(
            replay.replay_snapshot_for(vec![2]),
            SnapshotProjection::replay_vision(vec![2], SnapshotOptions::default())
        );
        assert_eq!(
            replay.observer_analysis_audience(),
            ObserverAnalysisAudience::ReplayViewers
        );

        let dev = ProjectionPolicy::new(
            VisibilityPolicy::DevFullWorld,
            DiagnosticPolicy::DEV_MOVEMENT_PATHS,
        );
        assert_eq!(
            dev.dev_snapshot_for(7),
            SnapshotProjection::full_world(
                7,
                SnapshotOptions {
                    include_movement_paths: true,
                    movement_paths_for_all_projected: true,
                },
            )
        );
        assert_eq!(
            dev.observer_analysis_audience(),
            ObserverAnalysisAudience::None
        );
        assert_eq!(
            dev.diagnostic_capabilities_for(RecipientRole::Spectator)
                .movement_paths,
            MovementPathDiagnosticScope::All
        );
    }

    #[test]
    fn projection_policy_classifies_lab_as_full_world_without_analysis() {
        let lab = ProjectionPolicy::new(VisibilityPolicy::LabFullWorld, DiagnosticPolicy::NONE);
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::Spectator, 99, Some(1), &[1, 2]),
            SnapshotProjection::full_world(1, SnapshotOptions::default())
        );
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::ActivePlayer, 99, None, &[1, 2]),
            SnapshotProjection::full_world(99, SnapshotOptions::default())
        );
        assert_eq!(
            lab.observer_analysis_audience(),
            ObserverAnalysisAudience::None
        );
    }
}
