use super::*;

#[derive(Clone, Copy)]
enum PlayerResourceProjection<'a> {
    None,
    All,
    Selected(&'a [u32]),
}

impl Game {
    const SPECTATOR_VIEWER_ID: u32 = 0;

    /// Build the fog-filtered snapshot for one player at the current tick. Includes ALL of the
    /// player's own entities plus neutral/enemy entities whose tile is currently visible.
    pub fn snapshot_for(&self, player: u32) -> Snapshot {
        self.snapshot_for_with_options(player, SnapshotOptions::default())
    }

    pub fn snapshot_for_with_options(&self, player: u32, options: SnapshotOptions) -> Snapshot {
        let live_fog = self.team_current_fog_for(player, &self.fog);
        if self.lingering_sight.is_empty() {
            return self.snapshot_for_mode(
                player,
                &[player],
                &live_fog,
                Some(&live_fog),
                true,
                PlayerResourceProjection::None,
                options,
            );
        }
        let snapshot_fog = self.snapshot_fog();
        let team_snapshot_fog = self.team_current_fog_for(player, &snapshot_fog);
        self.snapshot_for_mode(
            player,
            &[player],
            &team_snapshot_fog,
            Some(&live_fog),
            true,
            PlayerResourceProjection::None,
            options,
        )
    }

    /// Build a full-world snapshot for a viewer. Used only by dev watch flows where fog is
    /// intentionally disabled; normal gameplay must keep using [`snapshot_for`].
    pub fn snapshot_full_for(&self, player: u32) -> Snapshot {
        self.snapshot_full_for_with_options(player, SnapshotOptions::default())
    }

    pub fn snapshot_full_for_with_options(
        &self,
        player: u32,
        options: SnapshotOptions,
    ) -> Snapshot {
        self.snapshot_for_mode(
            player,
            &[],
            &self.fog,
            None,
            false,
            PlayerResourceProjection::All,
            options,
        )
    }

    /// Build a spectator snapshot from the union of all active players' current fog.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot {
        self.snapshot_for_spectator_with_options(visible_players, SnapshotOptions::default())
    }

    pub fn snapshot_for_spectator_with_options(
        &self,
        visible_players: &[u32],
        options: SnapshotOptions,
    ) -> Snapshot {
        let actionable_fog = self
            .fog
            .union_for(Self::SPECTATOR_VIEWER_ID, visible_players);
        if self.lingering_sight.is_empty() {
            return self.snapshot_for_mode(
                Self::SPECTATOR_VIEWER_ID,
                visible_players,
                &actionable_fog,
                Some(&actionable_fog),
                true,
                PlayerResourceProjection::Selected(visible_players),
                options,
            );
        }
        let snapshot_fog = self
            .snapshot_fog()
            .union_for(Self::SPECTATOR_VIEWER_ID, visible_players);
        self.snapshot_for_mode(
            Self::SPECTATOR_VIEWER_ID,
            visible_players,
            &snapshot_fog,
            Some(&actionable_fog),
            true,
            PlayerResourceProjection::Selected(visible_players),
            options,
        )
    }

    fn snapshot_fog(&self) -> Fog {
        if self.lingering_sight.is_empty() {
            return self.fog.clone();
        }
        let mut fog = self.fog.clone();
        fog.stamp_lingering_sources_with_smoke(
            &self.lingering_sight,
            &self.map,
            &self.entities,
            &self.smokes,
        );
        fog
    }

    fn snapshot_for_mode(
        &self,
        player: u32,
        remembered_building_players: &[u32],
        fog: &Fog,
        actionable_fog: Option<&Fog>,
        fogged: bool,
        player_resource_projection: PlayerResourceProjection<'_>,
        options: SnapshotOptions,
    ) -> Snapshot {
        let ps = self.player(player);
        let teams = self.team_relations();
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
            if let Some(view) = projection::project_entity(
                player,
                e,
                projection::EntityProjectionContext {
                    fog,
                    actionable_fog,
                    private_detail_fog: Some(&self.fog),
                    smokes: Some(&self.smokes),
                    fogged,
                    entities: &self.entities,
                    target,
                    debug_path_projection: options.debug_path_projection(),
                    active_construction_sites: Some(&self.active_construction_sites),
                    teams: Some(&teams),
                    owner_faction_id: self.player(e.owner).map(|p| p.faction_id.as_str()),
                    ability_runtime: Some(&self.ability_runtime),
                    tick: self.tick,
                },
            ) {
                entities.push(view);
            }
        }
        // Deterministic order (stable for tests / replays).
        entities.sort_by_key(|v| v.id);
        resource_deltas.sort_by_key(|d| d.id);
        let remembered_buildings = if fogged {
            self.remembered_building_views_for(player, remembered_building_players, fog, &teams)
        } else {
            Vec::new()
        };
        let mut smokes =
            if fogged && matches!(player_resource_projection, PlayerResourceProjection::None) {
                self.smokes
                    .iter()
                    .filter(|cloud| {
                        self.smokes
                            .visible_to_player(cloud, player, fog, &self.entities, &teams)
                    })
                    .map(|cloud| crate::protocol::SmokeCloudView {
                        id: cloud.id,
                        x: cloud.x,
                        y: cloud.y,
                        radius_tiles: cloud.radius_tiles,
                        expires_in: cloud.expires_in(self.tick),
                    })
                    .collect::<Vec<_>>()
            } else {
                self.smokes
                    .iter()
                    .map(|cloud| crate::protocol::SmokeCloudView {
                        id: cloud.id,
                        x: cloud.x,
                        y: cloud.y,
                        radius_tiles: cloud.radius_tiles,
                        expires_in: cloud.expires_in(self.tick),
                    })
                    .collect::<Vec<_>>()
            };
        smokes.sort_by_key(|smoke| smoke.id);
        let mut ability_objects = ability_projection::ability_object_views_for(
            self,
            player,
            fog,
            fogged,
            !matches!(player_resource_projection, PlayerResourceProjection::None),
        );
        ability_objects.sort_by_key(|object| object.id);

        let player_resources = self.player_resource_snapshots(player_resource_projection);

        Snapshot {
            tick: self.tick,
            steel,
            oil,
            supply_used,
            supply_cap,
            entities,
            resource_deltas,
            smokes,
            ability_objects,
            visible_tiles: if fogged {
                fog.visible_tiles_for(player)
            } else {
                Vec::new()
            },
            remembered_buildings,
            // Events are delivered via the `tick()` return value, not the snapshot.
            events: Vec::new(),
            upgrades: ps
                .map(|p| {
                    p.upgrades
                        .iter()
                        .map(|upgrade| upgrade.to_protocol_str().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            player_resources,
            net_status: crate::protocol::SnapshotNetStatus::default(),
        }
    }

    fn player(&self, id: u32) -> Option<&PlayerState> {
        self.players.iter().find(|p| p.id == id)
    }

    fn remembered_building_views_for(
        &self,
        player: u32,
        remembered_building_players: &[u32],
        fog: &Fog,
        teams: &crate::game::teams::TeamRelations,
    ) -> Vec<RememberedBuildingView> {
        let mut views: Vec<RememberedBuildingView> = Vec::new();
        for &memory_player in remembered_building_players {
            for entry in self.building_memory.entries_for_player(memory_player) {
                if self.live_entity_projects_for_remembered_building(player, entry.id, fog, teams) {
                    continue;
                }
                let view = RememberedBuildingView {
                    id: entry.id,
                    owner: entry.owner,
                    kind: crate::protocol::kind_to_wire(entry.kind).to_string(),
                    x: entry.x,
                    y: entry.y,
                    footprint: entry.footprint.iter().map(|&(tx, ty)| [tx, ty]).collect(),
                    observed_tick: entry.observed_tick,
                };
                match views.iter_mut().find(|existing| existing.id == view.id) {
                    Some(existing) if view.observed_tick > existing.observed_tick => {
                        *existing = view;
                    }
                    Some(_) => {}
                    None => views.push(view),
                }
            }
        }
        views.sort_by_key(|view| view.id);
        views
    }

    fn live_entity_projects_for_remembered_building(
        &self,
        player: u32,
        entity_id: u32,
        fog: &Fog,
        teams: &crate::game::teams::TeamRelations,
    ) -> bool {
        self.entities.get(entity_id).is_some_and(|entity| {
            projection::project_entity(
                player,
                entity,
                projection::EntityProjectionContext {
                    fog,
                    actionable_fog: Some(fog),
                    private_detail_fog: Some(&self.fog),
                    smokes: Some(&self.smokes),
                    fogged: true,
                    entities: &self.entities,
                    target: None,
                    debug_path_projection: projection::DebugPathProjection::None,
                    active_construction_sites: Some(&self.active_construction_sites),
                    teams: Some(teams),
                    owner_faction_id: self.player(entity.owner).map(|p| p.faction_id.as_str()),
                    ability_runtime: Some(&self.ability_runtime),
                    tick: self.tick,
                },
            )
            .is_some()
        })
    }

    fn player_resource_snapshots(
        &self,
        projection: PlayerResourceProjection<'_>,
    ) -> Vec<PlayerResourceSnapshot> {
        self.players
            .iter()
            .filter(|player| match projection {
                PlayerResourceProjection::None => false,
                PlayerResourceProjection::All => true,
                PlayerResourceProjection::Selected(player_ids) => player_ids.contains(&player.id),
            })
            .map(|player| PlayerResourceSnapshot {
                id: player.id,
                steel: player.steel,
                oil: player.oil,
                supply_used: player.supply_used,
                supply_cap: player.supply_cap,
            })
            .collect()
    }

    fn team_current_fog_for(&self, player: u32, fog: &Fog) -> Fog {
        let mut visible_players = self.living_team_player_ids_for_vision(player);
        if visible_players.is_empty() {
            visible_players.push(player);
        }
        fog.union_for(player, &visible_players)
    }
}
