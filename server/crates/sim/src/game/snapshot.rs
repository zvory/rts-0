use super::*;

#[derive(Clone, Copy)]
pub(in crate::game) enum PlayerResourceProjection<'a> {
    None,
    All,
    Selected(&'a [u32]),
}

#[derive(Clone, Copy)]
pub(in crate::game) struct SnapshotMode<'a> {
    pub(in crate::game) player: u32,
    pub(in crate::game) memory_players: &'a [u32],
    pub(in crate::game) fog: &'a Fog,
    pub(in crate::game) actionable_fog: Option<&'a Fog>,
    pub(in crate::game) fogged: bool,
    pub(in crate::game) player_resource_projection: PlayerResourceProjection<'a>,
    pub(in crate::game) private_detail_projection: projection::PrivateDetailProjection<'a>,
    pub(in crate::game) owner_visible_players: &'a [u32],
    pub(in crate::game) omniscient: bool,
}

impl Game {
    /// Build the fog-filtered snapshot for one player at the current tick. Includes ALL of the
    /// player's own entities plus neutral/enemy entities whose tile is currently visible.
    pub fn snapshot_for(&self, player: u32) -> Snapshot {
        self.snapshot_for_with_options(player, SnapshotOptions::default())
    }

    pub fn snapshot_for_with_options(&self, player: u32, options: SnapshotOptions) -> Snapshot {
        let live_fog = self.team_current_fog_for(player, &self.state.fog);
        let presentation_fog = self.team_presentation_fog_for(player, &self.state.fog);
        let mut snapshot = self.snapshot_for_mode(
            SnapshotMode {
                player,
                memory_players: &[player],
                fog: &live_fog,
                actionable_fog: Some(&live_fog),
                fogged: true,
                player_resource_projection: PlayerResourceProjection::None,
                private_detail_projection: projection::PrivateDetailProjection::ExactViewer,
                owner_visible_players: &[player],
                omniscient: false,
            },
            options,
        );
        snapshot.visible_tiles = presentation_fog.visible_tiles_for(player);
        snapshot
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
            SnapshotMode {
                player,
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
        )
    }

    pub(in crate::game) fn snapshot_for_mode(
        &self,
        mode: SnapshotMode<'_>,
        options: SnapshotOptions,
    ) -> Snapshot {
        let SnapshotMode {
            player,
            memory_players,
            fog,
            actionable_fog,
            fogged,
            player_resource_projection,
            private_detail_projection,
            owner_visible_players,
            omniscient,
        } = mode;
        let ps = self.player(player);
        let teams = self.team_relations();
        let (steel, oil, supply_used, supply_cap) = match ps {
            Some(p) => (p.steel, p.oil, p.supply_used, config::PLAYER_SUPPLY_CAP),
            None => (0, 0, 0, 0),
        };

        let mut entities = Vec::new();
        let mut resource_deltas = Vec::new();
        // Use the spatial index for interest filtering instead of a full entity scan.
        for id in self.final_spatial().all_ids() {
            let e = match self.state.entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            let target = e
                .target_id()
                .and_then(|target| self.state.entities.get(target));
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
                    private_detail_fog: Some(&self.state.fog),
                    private_detail_projection,
                    smokes: Some(&self.state.smokes),
                    fogged,
                    entities: &self.state.entities,
                    target,
                    debug_path_projection: options.debug_path_projection(),
                    active_construction_sites: Some(&self.state.active_construction_sites),
                    extractor_active: (e.kind == EntityKind::PumpJack).then(|| {
                        services::economy::pump_jack_is_active(&self.state.entities, &teams, e.id)
                    }),
                    teams: Some(&teams),
                    owner_faction_id: self.player(e.owner).map(|p| p.faction_id.as_str()),
                    ability_runtime: Some(&self.state.ability_runtime),
                    tick: self.state.tick,
                },
            ) {
                entities.push(view);
            }
        }
        // Deterministic order (stable for tests / replays).
        entities.sort_by_key(|v| v.id);
        resource_deltas.sort_by_key(|d| d.id);
        let remembered_buildings = if fogged {
            self.remembered_building_views_for(player, memory_players, fog, &teams)
        } else {
            Vec::new()
        };
        let remembered_anti_tank_guns = if fogged {
            self.remembered_anti_tank_gun_views_for(player, memory_players, fog, &teams)
        } else {
            Vec::new()
        };
        let mut smokes = if fogged && !omniscient {
            self.state
                .smokes
                .iter()
                .filter(|cloud| {
                    if owner_visible_players.is_empty() {
                        self.state.smokes.visible_to_player(
                            cloud,
                            player,
                            fog,
                            &self.state.entities,
                            &teams,
                        )
                    } else {
                        owner_visible_players.iter().any(|selected| {
                            let selected_fog =
                                self.team_current_fog_for(*selected, &self.state.fog);
                            self.state.smokes.visible_to_player(
                                cloud,
                                *selected,
                                &selected_fog,
                                &self.state.entities,
                                &teams,
                            )
                        })
                    }
                })
                .map(|cloud| crate::protocol::SmokeCloudView {
                    id: cloud.id,
                    x: cloud.x,
                    y: cloud.y,
                    radius_tiles: cloud.radius_tiles,
                    expires_in: cloud.expires_in(self.state.tick),
                })
                .collect::<Vec<_>>()
        } else {
            self.state
                .smokes
                .iter()
                .map(|cloud| crate::protocol::SmokeCloudView {
                    id: cloud.id,
                    x: cloud.x,
                    y: cloud.y,
                    radius_tiles: cloud.radius_tiles,
                    expires_in: cloud.expires_in(self.state.tick),
                })
                .collect::<Vec<_>>()
        };
        smokes.sort_by_key(|smoke| smoke.id);
        let mut ability_objects = ability_projection::ability_object_views_for(
            self,
            player,
            fog,
            fogged,
            owner_visible_players,
            omniscient,
        );
        ability_objects.sort_by_key(|object| object.id);
        let trenches = self
            .state
            .trenches
            .views_for(player, fog, fogged, memory_players);

        let player_resources = self.player_resource_snapshots(player_resource_projection);

        Snapshot {
            tick: self.state.tick,
            world_combat_position: world_combat::signal_position(
                self.state.tick,
                self.state.world_combat_active_through_tick,
                self.state.world_combat_position,
            ),
            steel,
            oil,
            supply_used,
            supply_cap,
            entities,
            resource_deltas,
            smokes,
            ability_objects,
            trenches,
            visible_tiles: if omniscient {
                self.state.fog.all_visible_tiles()
            } else if fogged {
                fog.visible_tiles_for(player)
            } else {
                Vec::new()
            },
            explored_tiles: if omniscient {
                self.state.fog.all_visible_tiles()
            } else if fogged {
                fog.explored_tiles_for(player)
            } else {
                Vec::new()
            },
            remembered_buildings,
            remembered_anti_tank_guns,
            // Events are delivered via the `tick()` return value, not the snapshot.
            events: Vec::new(),
            upgrades: match player_resource_projection {
                PlayerResourceProjection::Selected([only]) => self.player(*only),
                PlayerResourceProjection::None | PlayerResourceProjection::All => ps,
                PlayerResourceProjection::Selected(_) => None,
            }
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
        self.state.players.iter().find(|p| p.id == id)
    }

    fn remembered_building_views_for(
        &self,
        player: u32,
        memory_players: &[u32],
        fog: &Fog,
        teams: &crate::game::teams::TeamRelations,
    ) -> Vec<RememberedBuildingView> {
        let mut views: Vec<RememberedBuildingView> = Vec::new();
        for &memory_player in memory_players {
            for entry in self.state.building_memory.entries_for_player(memory_player) {
                if self.live_entity_projects(player, memory_players, entry.id, fog, teams) {
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

    pub(in crate::game) fn live_entity_projects(
        &self,
        player: u32,
        selected_players: &[u32],
        entity_id: u32,
        fog: &Fog,
        teams: &crate::game::teams::TeamRelations,
    ) -> bool {
        self.state.entities.get(entity_id).is_some_and(|entity| {
            projection::project_entity(
                player,
                entity,
                projection::EntityProjectionContext {
                    fog,
                    actionable_fog: Some(fog),
                    private_detail_fog: Some(&self.state.fog),
                    private_detail_projection: projection::PrivateDetailProjection::SelectedOwners(
                        selected_players,
                    ),
                    smokes: Some(&self.state.smokes),
                    fogged: true,
                    entities: &self.state.entities,
                    target: None,
                    debug_path_projection: projection::DebugPathProjection::None,
                    active_construction_sites: Some(&self.state.active_construction_sites),
                    extractor_active: None,
                    teams: Some(teams),
                    owner_faction_id: self.player(entity.owner).map(|p| p.faction_id.as_str()),
                    ability_runtime: Some(&self.state.ability_runtime),
                    tick: self.state.tick,
                },
            )
            .is_some()
        })
    }

    fn player_resource_snapshots(
        &self,
        projection: PlayerResourceProjection<'_>,
    ) -> Vec<PlayerResourceSnapshot> {
        self.state
            .players
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
                supply_cap: config::PLAYER_SUPPLY_CAP,
                apm: self.current_apm(player.id),
                upgrades: player
                    .upgrades
                    .iter()
                    .map(|upgrade| upgrade.to_protocol_str().to_string())
                    .collect(),
            })
            .collect()
    }

    pub(in crate::game) fn team_current_fog_for(&self, player: u32, fog: &Fog) -> Fog {
        let mut visible_players = self.living_team_player_ids_for_vision(player);
        if visible_players.is_empty() {
            visible_players.push(player);
        }
        fog.union_for(player, &visible_players)
    }

    pub(in crate::game) fn team_presentation_fog_for(&self, player: u32, fog: &Fog) -> Fog {
        let mut visible_players = self.living_team_player_ids_for_vision(player);
        if visible_players.is_empty() {
            visible_players.push(player);
        }
        fog.presentation_union_for(player, &visible_players)
    }
}
