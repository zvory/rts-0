use super::snapshot::{PlayerResourceProjection, SnapshotMode};
use super::*;

const OBSERVER_FOG_VIEWER_ID: u32 = 0;

/// A read-only observer's authoritative perspective. This value never conveys command authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserverView {
    /// Full world state, including private details for every real owner.
    Omniscient,
    /// The combined perspective of the explicitly selected real players.
    Players(Vec<u32>),
}

impl Game {
    /// Build a spectator snapshot from the union of all active players' current fog.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot {
        self.snapshot_for_spectator_with_options(visible_players, SnapshotOptions::default())
    }

    pub fn snapshot_for_spectator_with_options(
        &self,
        visible_players: &[u32],
        options: SnapshotOptions,
    ) -> Snapshot {
        self.snapshot_for_observer_with_options(
            &ObserverView::Players(visible_players.to_vec()),
            options,
        )
    }

    pub fn snapshot_for_observer(&self, view: &ObserverView) -> Snapshot {
        self.snapshot_for_observer_with_options(view, SnapshotOptions::default())
    }

    pub fn snapshot_for_observer_with_options(
        &self,
        view: &ObserverView,
        options: SnapshotOptions,
    ) -> Snapshot {
        let ObserverView::Players(selected_players) = view else {
            return self.snapshot_for_mode(
                SnapshotMode {
                    player: OBSERVER_FOG_VIEWER_ID,
                    memory_players: &[],
                    fog: &self.state.fog,
                    actionable_fog: None,
                    fogged: false,
                    player_resource_projection: PlayerResourceProjection::All,
                    private_detail_projection: projection::PrivateDetailProjection::AllProjected,
                    owner_visible_players: &[],
                    omniscient: true,
                },
                options,
            );
        };
        let mut fog_players = Vec::new();
        for &selected in selected_players {
            let mut team_players = self.living_team_player_ids_for_vision(selected);
            if team_players.is_empty() {
                team_players.push(selected);
            }
            for player_id in team_players {
                if !fog_players.contains(&player_id) {
                    fog_players.push(player_id);
                }
            }
        }
        let actionable_fog = self
            .state
            .fog
            .union_for(OBSERVER_FOG_VIEWER_ID, &fog_players);
        self.snapshot_for_mode(
            SnapshotMode {
                player: OBSERVER_FOG_VIEWER_ID,
                memory_players: selected_players,
                fog: &actionable_fog,
                actionable_fog: Some(&actionable_fog),
                fogged: true,
                player_resource_projection: PlayerResourceProjection::Selected(selected_players),
                private_detail_projection: projection::PrivateDetailProjection::SelectedOwners(
                    selected_players,
                ),
                owner_visible_players: selected_players,
                omniscient: false,
            },
            options,
        )
    }
}
