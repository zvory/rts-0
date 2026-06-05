use super::*;

impl Game {
    const SPECTATOR_VIEWER_ID: u32 = 0;

    /// Build the fog-filtered snapshot for one player at the current tick. Includes ALL of the
    /// player's own entities plus neutral/enemy entities whose tile is currently visible.
    pub fn snapshot_for(&self, player: u32) -> Snapshot {
        self.snapshot_for_mode(player, &self.fog, true, false)
    }

    /// Build a full-world snapshot for a viewer. Used only by dev watch flows where fog is
    /// intentionally disabled; normal gameplay must keep using [`snapshot_for`].
    pub fn snapshot_full_for(&self, player: u32) -> Snapshot {
        self.snapshot_for_mode(player, &self.fog, false, true)
    }

    /// Build a spectator snapshot from the union of all active players' current fog.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot {
        let fog = self
            .fog
            .union_for(Self::SPECTATOR_VIEWER_ID, visible_players);
        self.snapshot_for_mode(Self::SPECTATOR_VIEWER_ID, &fog, true, true)
    }

    fn snapshot_for_mode(
        &self,
        player: u32,
        fog: &Fog,
        fogged: bool,
        include_player_resources: bool,
    ) -> Snapshot {
        let ps = self.player(player);
        let (steel, oil, supply_used, supply_cap) = match ps {
            Some(p) => (p.steel, p.oil, p.supply_used, p.supply_cap),
            None => (0, 0, 0, 0),
        };

        let mut entities = Vec::new();
        let mut resource_deltas = Vec::new();
        // Use the spatial index for interest filtering instead of a full entity scan.
        for id in self.spatial.all_ids() {
            let e = match self.entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            let target = e.target_id().and_then(|target| self.entities.get(target));
            if e.is_node() && (!fogged || fog.is_visible_world(player, e.pos_x, e.pos_y)) {
                if let Some(remaining) = e.remaining() {
                    resource_deltas.push(ResourceDelta {
                        id: e.id,
                        remaining,
                    });
                }
            }
            if let Some(view) = projection::project_entity(player, e, fog, fogged, target) {
                entities.push(view);
            }
        }
        // Deterministic order (stable for tests / replays).
        entities.sort_by_key(|v| v.id);
        resource_deltas.sort_by_key(|d| d.id);

        let player_resources = if include_player_resources {
            self.players
                .iter()
                .map(|p| PlayerResourceSnapshot {
                    id: p.id,
                    steel: p.steel,
                    oil: p.oil,
                    supply_used: p.supply_used,
                    supply_cap: p.supply_cap,
                })
                .collect()
        } else {
            Vec::new()
        };

        Snapshot {
            tick: self.tick,
            steel,
            oil,
            supply_used,
            supply_cap,
            entities,
            resource_deltas,
            // Events are delivered via the `tick()` return value, not the snapshot.
            events: Vec::new(),
            player_resources,
        }
    }

    fn player(&self, id: u32) -> Option<&PlayerState> {
        self.players.iter().find(|p| p.id == id)
    }
}
