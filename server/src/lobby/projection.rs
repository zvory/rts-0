use super::session_policy::{
    DiagnosticPolicy, MovementPathDiagnosticPolicy, ObserverAnalysisPolicy, VisibilityPolicy,
};
use super::snapshots::{union_events, union_events_without_private_notices};
use crate::protocol::{
    DiagnosticCapabilities, Event, MovementPathDiagnosticScope, ObserverAnalysisPayload, Snapshot,
    VisionSelectionRequest,
};
use rts_sim::game::{Game, ObserverView, SnapshotOptions};
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
    Observer {
        view: ObserverView,
        options: SnapshotOptions,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EventProjection {
    PlayerOnly {
        player_id: u32,
    },
    FullVision {
        include_private_notices: bool,
    },
    PlayerUnion {
        player_ids: Vec<u32>,
        include_private_notices: bool,
    },
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

    fn observer(
        view: ObserverView,
        options: SnapshotOptions,
        include_private_notices: bool,
    ) -> Self {
        let events = match &view {
            ObserverView::Omniscient => EventProjection::FullVision {
                include_private_notices,
            },
            ObserverView::Players(player_ids) => EventProjection::PlayerUnion {
                player_ids: player_ids.clone(),
                include_private_notices,
            },
        };
        Self {
            body: SnapshotBodyProjection::Observer { view, options },
            events,
        }
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
            SnapshotBodyProjection::Observer { view, options } => {
                game.snapshot_for_observer_with_options(view, *options)
            }
        };

        match &self.events {
            EventProjection::PlayerOnly { player_id } => {
                if let Some(mut events) = per_player_events.remove(player_id) {
                    snapshot.events.append(&mut events);
                }
            }
            EventProjection::FullVision {
                include_private_notices,
            } => {
                if *include_private_notices {
                    snapshot.events.extend(full_vision_events.to_vec());
                } else {
                    snapshot.events.extend(union_events_without_private_notices(
                        per_player_events.values(),
                    ));
                }
            }
            EventProjection::PlayerUnion {
                player_ids,
                include_private_notices,
            } => {
                let event_sets = player_ids
                    .iter()
                    .filter_map(|player_id| per_player_events.get(player_id));
                if *include_private_notices {
                    snapshot.events.extend(union_events(event_sets));
                } else {
                    snapshot
                        .events
                        .extend(union_events_without_private_notices(event_sets));
                }
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
        _spectator_visible_player_ids: &[u32],
    ) -> SnapshotProjection {
        if self.visibility == VisibilityPolicy::FullWorldProjection {
            return SnapshotProjection::observer(
                ObserverView::Omniscient,
                self.snapshot_options_for(role),
                true,
            );
        }
        match role {
            RecipientRole::ActivePlayer => SnapshotProjection::player_fog(
                seat_id.unwrap_or(connection_id),
                self.snapshot_options_for(role),
            ),
            RecipientRole::Spectator => SnapshotProjection::observer(
                ObserverView::Omniscient,
                self.snapshot_options_for(role),
                false,
            ),
        }
    }

    pub(super) fn selected_perspective_snapshot_for(
        self,
        view: ObserverView,
    ) -> SnapshotProjection {
        self.observer_snapshot_for(view, true)
    }

    pub(super) fn observer_snapshot_for(
        self,
        view: ObserverView,
        include_private_notices: bool,
    ) -> SnapshotProjection {
        SnapshotProjection::observer(
            view,
            self.snapshot_options_for(RecipientRole::Spectator),
            include_private_notices,
        )
    }

    pub(super) fn dev_watch_snapshot_for(
        self,
        role: RecipientRole,
        view_player_id: u32,
    ) -> SnapshotProjection {
        match role {
            RecipientRole::ActivePlayer => {
                SnapshotProjection::player_fog(view_player_id, self.snapshot_options_for(role))
            }
            RecipientRole::Spectator => SnapshotProjection::observer(
                ObserverView::Omniscient,
                self.snapshot_options_for(role),
                true,
            ),
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

pub(super) fn scope_observer_analysis(
    mut analysis: ObserverAnalysisPayload,
    view: &ObserverView,
) -> ObserverAnalysisPayload {
    if let ObserverView::Players(player_ids) = view {
        analysis
            .players
            .retain(|player| player_ids.contains(&player.id));
        analysis.map_analysis = None;
    }
    analysis
}

pub(super) fn observer_view_from_selection(
    selection: VisionSelectionRequest,
    valid_player_ids: &[u32],
) -> ObserverView {
    match selection {
        VisionSelectionRequest::All => ObserverView::Players(valid_player_ids.to_vec()),
        VisionSelectionRequest::Player { player_id } => ObserverView::Players(vec![player_id]),
        VisionSelectionRequest::Players { player_ids } => ObserverView::Players(player_ids),
        VisionSelectionRequest::Omniscient => ObserverView::Omniscient,
    }
}

pub(super) fn selection_from_observer_view(
    view: &ObserverView,
    valid_player_ids: &[u32],
) -> VisionSelectionRequest {
    match view {
        ObserverView::Omniscient => VisionSelectionRequest::Omniscient,
        ObserverView::Players(player_ids) if player_ids.is_empty() => VisionSelectionRequest::All,
        ObserverView::Players(player_ids)
            if player_ids.len() == valid_player_ids.len()
                && valid_player_ids.iter().all(|id| player_ids.contains(id)) =>
        {
            VisionSelectionRequest::All
        }
        ObserverView::Players(player_ids) if player_ids.len() == 1 => {
            VisionSelectionRequest::Player {
                player_id: player_ids[0],
            }
        }
        ObserverView::Players(player_ids) => VisionSelectionRequest::Players {
            player_ids: player_ids.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_selection_maps_to_player_union_and_omniscient_remains_distinct() {
        let valid_player_ids = [1, 2, 3];

        assert_eq!(
            observer_view_from_selection(VisionSelectionRequest::All, &valid_player_ids),
            ObserverView::Players(valid_player_ids.to_vec())
        );
        assert_eq!(
            observer_view_from_selection(VisionSelectionRequest::Omniscient, &valid_player_ids),
            ObserverView::Omniscient
        );
        assert_eq!(
            selection_from_observer_view(
                &ObserverView::Players(valid_player_ids.to_vec()),
                &valid_player_ids,
            ),
            VisionSelectionRequest::All
        );
        assert_eq!(
            selection_from_observer_view(&ObserverView::Omniscient, &valid_player_ids),
            VisionSelectionRequest::Omniscient
        );
    }

    #[test]
    fn projection_policy_classifies_live_players_spectators_and_branch_aliases() {
        let policy = ProjectionPolicy::new(
            VisibilityPolicy::LiveFog,
            DiagnosticPolicy::SPECTATOR_OBSERVER_ANALYSIS,
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
            SnapshotProjection::observer(
                ObserverView::Omniscient,
                SnapshotOptions::default(),
                false,
            )
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
            replay.selected_perspective_snapshot_for(ObserverView::Players(vec![2])),
            SnapshotProjection::observer(
                ObserverView::Players(vec![2]),
                SnapshotOptions::default(),
                true,
            )
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
            dev.dev_watch_snapshot_for(RecipientRole::ActivePlayer, 7),
            SnapshotProjection::player_fog(
                7,
                SnapshotOptions {
                    include_movement_paths: true,
                    movement_paths_for_all_projected: true,
                },
            )
        );
        assert_eq!(
            dev.dev_watch_snapshot_for(RecipientRole::Spectator, 7),
            SnapshotProjection::observer(
                ObserverView::Omniscient,
                SnapshotOptions {
                    include_movement_paths: true,
                    movement_paths_for_all_projected: true,
                },
                true,
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
    fn projection_policy_classifies_lab_as_fog_union_without_analysis() {
        let lab = ProjectionPolicy::new(VisibilityPolicy::LabPerspective, DiagnosticPolicy::NONE);
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::Spectator, 99, Some(1), &[1, 2]),
            SnapshotProjection::observer(
                ObserverView::Omniscient,
                SnapshotOptions::default(),
                false,
            )
        );
        assert_eq!(
            lab.live_snapshot_for(RecipientRole::ActivePlayer, 99, None, &[1, 2]),
            SnapshotProjection::player_fog(99, SnapshotOptions::default())
        );
        assert_eq!(
            lab.observer_analysis_audience(),
            ObserverAnalysisAudience::None
        );
    }
}
