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
pub(super) enum SnapshotProjection {
    PlayerFog {
        player_id: u32,
        options: SnapshotOptions,
    },
    SpectatorUnion {
        player_ids: Vec<u32>,
        options: SnapshotOptions,
    },
    SelectablePerspective {
        player_ids: Vec<u32>,
        options: SnapshotOptions,
    },
    FullWorld {
        player_id: u32,
        options: SnapshotOptions,
    },
}

impl SnapshotProjection {
    pub(super) fn snapshot_with_events(
        &self,
        game: &Game,
        per_player_events: &mut HashMap<u32, Vec<Event>>,
        full_vision_events: &[Event],
    ) -> Snapshot {
        let mut snapshot = match self {
            SnapshotProjection::PlayerFog { player_id, options } => {
                game.snapshot_for_with_options(*player_id, *options)
            }
            SnapshotProjection::SpectatorUnion {
                player_ids,
                options,
            }
            | SnapshotProjection::SelectablePerspective {
                player_ids,
                options,
            } => game.snapshot_for_spectator_with_options(player_ids, *options),
            SnapshotProjection::FullWorld { player_id, options } => {
                game.snapshot_full_for_with_options(*player_id, *options)
            }
        };

        match self {
            SnapshotProjection::PlayerFog { player_id, .. }
            | SnapshotProjection::FullWorld { player_id, .. } => {
                if let Some(mut events) = per_player_events.remove(player_id) {
                    snapshot.events.append(&mut events);
                }
            }
            SnapshotProjection::SpectatorUnion { .. } => {
                snapshot.events.extend(full_vision_events.to_vec());
            }
            SnapshotProjection::SelectablePerspective { player_ids, .. } => {
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
    SpectatorRecipients,
    AllRecipients,
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
        if self.visibility == VisibilityPolicy::FullWorldProjection {
            return SnapshotProjection::FullWorld {
                player_id: seat_id.unwrap_or(connection_id),
                options: self.snapshot_options_for(role),
            };
        }
        match role {
            RecipientRole::ActivePlayer => SnapshotProjection::PlayerFog {
                player_id: seat_id.unwrap_or(connection_id),
                options: self.snapshot_options_for(role),
            },
            RecipientRole::Spectator => SnapshotProjection::SpectatorUnion {
                player_ids: spectator_visible_player_ids.to_vec(),
                options: self.snapshot_options_for(role),
            },
        }
    }

    pub(super) fn selected_perspective_snapshot_for(
        self,
        visible_player_ids: Vec<u32>,
    ) -> SnapshotProjection {
        SnapshotProjection::SelectablePerspective {
            player_ids: visible_player_ids,
            options: self.snapshot_options_for(RecipientRole::Spectator),
        }
    }

    pub(super) fn full_world_snapshot_for(self, view_player_id: u32) -> SnapshotProjection {
        SnapshotProjection::FullWorld {
            player_id: view_player_id,
            options: self.snapshot_options_for(RecipientRole::Spectator),
        }
    }

    pub(super) fn observer_analysis_audience(self) -> ObserverAnalysisAudience {
        match self.diagnostics.observer_analysis {
            ObserverAnalysisPolicy::SpectatorRecipients => {
                ObserverAnalysisAudience::SpectatorRecipients
            }
            ObserverAnalysisPolicy::AllRecipients => ObserverAnalysisAudience::AllRecipients,
            ObserverAnalysisPolicy::None => ObserverAnalysisAudience::None,
        }
    }

    pub(super) fn diagnostic_capabilities_for(self, role: RecipientRole) -> DiagnosticCapabilities {
        DiagnosticCapabilities {
            movement_paths: self.movement_path_scope_for(role),
            observer_analysis: matches!(
                (self.diagnostics.observer_analysis, role),
                (
                    ObserverAnalysisPolicy::SpectatorRecipients,
                    RecipientRole::Spectator
                ) | (ObserverAnalysisPolicy::AllRecipients, _)
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
            DiagnosticPolicy::SPECTATOR_OBSERVER_ANALYSIS,
        );

        assert_eq!(
            policy.live_snapshot_for(RecipientRole::ActivePlayer, 10, None, &[1, 2]),
            SnapshotProjection::PlayerFog {
                player_id: 10,
                options: SnapshotOptions::default()
            }
        );
        assert_eq!(
            policy.live_snapshot_for(RecipientRole::ActivePlayer, 100, Some(1), &[1, 2]),
            SnapshotProjection::PlayerFog {
                player_id: 1,
                options: SnapshotOptions::default()
            }
        );
        assert_eq!(
            policy.live_snapshot_for(RecipientRole::Spectator, 102, None, &[1, 2]),
            SnapshotProjection::SpectatorUnion {
                player_ids: vec![1, 2],
                options: SnapshotOptions::default()
            }
        );
        assert_eq!(
            policy.observer_analysis_audience(),
            ObserverAnalysisAudience::SpectatorRecipients
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
            VisibilityPolicy::SelectablePerspective,
            DiagnosticPolicy::ALL_RECIPIENT_OBSERVER_ANALYSIS,
        );
        assert_eq!(
            replay.selected_perspective_snapshot_for(vec![2]),
            SnapshotProjection::SelectablePerspective {
                player_ids: vec![2],
                options: SnapshotOptions::default()
            }
        );
        assert_eq!(
            replay.observer_analysis_audience(),
            ObserverAnalysisAudience::AllRecipients
        );

        let dev = ProjectionPolicy::new(
            VisibilityPolicy::FullWorldProjection,
            DiagnosticPolicy::PROJECTED_MOVEMENT_PATHS,
        );
        assert_eq!(
            dev.full_world_snapshot_for(7),
            SnapshotProjection::FullWorld {
                player_id: 7,
                options: SnapshotOptions {
                    include_movement_paths: true,
                    movement_paths_for_all_projected: true,
                }
            }
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
        let lab = ProjectionPolicy::new(
            VisibilityPolicy::FullWorldProjection,
            DiagnosticPolicy::NONE,
        );
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::Spectator, 99, Some(1), &[1, 2]),
            SnapshotProjection::FullWorld {
                player_id: 1,
                options: SnapshotOptions::default()
            }
        );
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::ActivePlayer, 99, None, &[1, 2]),
            SnapshotProjection::FullWorld {
                player_id: 99,
                options: SnapshotOptions::default()
            }
        );
        assert_eq!(
            lab.observer_analysis_audience(),
            ObserverAnalysisAudience::None
        );
    }
}
