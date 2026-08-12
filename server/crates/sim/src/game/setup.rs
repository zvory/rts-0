use super::resource_placement;
use super::*;
use crate::game::derived_state::DerivedState;
use crate::rules::faction::{catalog_for_or_default_empty, FactionLoadout, StartingFormation};
use std::str::FromStr;

mod checkpoint_start;
mod dev_scenarios;
mod resource_patch_start;

const LIVE_PATHING_DEFAULT_BUDGET: usize = 32_768;
const LIVE_PATHING_CACHE_CAPACITY: usize = 256;
const STEEL_FIELD_COLUMNS: u32 = 6;

impl Game {
    #[allow(dead_code)]
    pub fn new(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(players, None, seed, StartingLoadout::Standard, None)
    }

    /// Compatibility constructor retained for callers that still name live AI profile setup.
    /// AI controllers are owned by the caller, not by `Game`.
    #[allow(dead_code)]
    pub fn new_with_random_ai_profiles(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(players, None, seed, StartingLoadout::Standard, None)
    }

    /// Compatibility helper for tests with one global resource override.
    #[allow(dead_code)]
    pub fn new_with_starting_resources(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            Some((steel, oil)),
            seed,
            StartingLoadout::Standard,
            None,
        )
    }

    /// Compatibility helper for tests with one global resource override.
    #[allow(dead_code)]
    pub fn new_with_starting_resources_and_random_ai_profiles(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            Some((steel, oil)),
            seed,
            StartingLoadout::Standard,
            None,
        )
    }

    /// Compatibility constructor retained for callers that still name live AI profile setup.
    /// AI controllers are owned by the caller, not by `Game`.
    pub fn new_with_random_ai_profiles_and_map(
        players: &[PlayerInit],
        seed: u32,
        map: Map,
    ) -> Game {
        Self::new_with_random_ai_profiles_and_map_metadata(
            players,
            seed,
            map,
            default_map_metadata(),
        )
    }

    pub fn new_with_random_ai_profiles_and_map_metadata(
        players: &[PlayerInit],
        seed: u32,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Game {
        Self::new_inner_with_map(
            players,
            None,
            seed,
            StartingLoadout::Standard,
            None,
            Some(map),
            map_metadata,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_replay(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_without_ai_controllers(players, seed)
    }

    /// Compatibility helper for old tests with one global resource override.
    pub fn new_for_replay_with_starting_resources(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            Some((steel, oil)),
            seed,
            StartingLoadout::Standard,
            None,
        )
    }

    pub fn new_for_replay_with_starting_loadouts(
        players: &[PlayerInit],
        starting_loadouts: &[PlayerStartingLoadout],
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            None,
            seed,
            StartingLoadout::Standard,
            Some(starting_loadouts),
        )
    }

    /// Rebuild a replay from an explicit map and starting loadout. Replay playback owns command
    /// injection externally, so no live AI controllers are attached.
    pub fn new_for_replay_with_map_metadata(
        players: &[PlayerInit],
        seed: u32,
        starting_loadouts: &[PlayerStartingLoadout],
        map: Map,
        map_metadata: MapMetadata,
    ) -> Game {
        Self::new_inner_with_map(
            players,
            None,
            seed,
            StartingLoadout::Standard,
            Some(starting_loadouts),
            Some(map),
            map_metadata,
        )
    }

    /// Create a match that preserves player identity flags but does not attach live
    /// controllers. Used by command-log replay and scripted self-play, where commands come from
    /// an external driver.
    pub fn new_without_ai_controllers(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(players, None, seed, StartingLoadout::Standard, None)
    }

    pub fn seed(&self) -> u32 {
        self.state.seed
    }

    pub fn starting_steel(&self) -> u32 {
        self.state
            .starting_loadouts
            .first()
            .map(|loadout| loadout.starting_steel)
            .unwrap_or(config::STARTING_STEEL)
    }

    pub fn starting_oil(&self) -> u32 {
        self.state
            .starting_loadouts
            .first()
            .map(|loadout| loadout.starting_oil)
            .unwrap_or(config::STARTING_OIL)
    }

    pub fn starting_loadouts(&self) -> &[PlayerStartingLoadout] {
        &self.state.starting_loadouts
    }

    pub fn map_metadata(&self) -> &MapMetadata {
        &self.state.map_metadata
    }

    fn new_inner(
        players: &[PlayerInit],
        resource_override: Option<(u32, u32)>,
        seed: u32,
        starting_loadout: StartingLoadout,
        starting_loadout_overrides: Option<&[PlayerStartingLoadout]>,
    ) -> Game {
        Self::new_inner_with_map(
            players,
            resource_override,
            seed,
            starting_loadout,
            starting_loadout_overrides,
            None,
            default_map_metadata(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_inner_with_map(
        players: &[PlayerInit],
        resource_override: Option<(u32, u32)>,
        seed: u32,
        starting_loadout: StartingLoadout,
        starting_loadout_overrides: Option<&[PlayerStartingLoadout]>,
        map_override: Option<Map>,
        map_metadata: MapMetadata,
    ) -> Game {
        let direct = Self::new_inner_direct_with_map(
            players,
            resource_override,
            seed,
            starting_loadout,
            starting_loadout_overrides,
            map_override,
            map_metadata,
        );
        Self::checkpoint_backed_start_from_direct(direct, "game setup")
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn new_direct_start_for_test(
        players: &[PlayerInit],
        resource_override: Option<(u32, u32)>,
        seed: u32,
        starting_loadout_overrides: Option<&[PlayerStartingLoadout]>,
        map_override: Option<Map>,
        map_metadata: MapMetadata,
    ) -> Game {
        Self::new_inner_direct_with_map(
            players,
            resource_override,
            seed,
            StartingLoadout::Standard,
            starting_loadout_overrides,
            map_override,
            map_metadata,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_inner_direct_with_map(
        players: &[PlayerInit],
        resource_override: Option<(u32, u32)>,
        seed: u32,
        starting_loadout: StartingLoadout,
        starting_loadout_overrides: Option<&[PlayerStartingLoadout]>,
        map_override: Option<Map>,
        map_metadata: MapMetadata,
    ) -> Game {
        let start_players: Vec<_> = players
            .iter()
            .map(|player| {
                (
                    player.id,
                    super::teams::normalize_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map = map_override.unwrap_or_else(|| Map::generate_for_players(&start_players, seed));
        let fog = Fog::new(map.width, map.height);
        let mut entities = EntityStore::new();

        let mut player_states = Vec::with_capacity(players.len() + 1);
        let mut resolved_starting_loadouts = Vec::with_capacity(players.len());
        let mut starts_with_resources = Vec::with_capacity(players.len());
        for (i, p) in players.iter().enumerate() {
            let start = map.starts.get(i).copied().unwrap_or((0, 0));
            let faction_id = if p.faction_id.is_empty() {
                DEFAULT_FACTION_ID.to_string()
            } else {
                p.faction_id.clone()
            };
            let catalog = catalog_for_or_default_empty(&faction_id);
            let loadout = catalog.map(|catalog| catalog.loadout);
            let override_record = starting_loadout_overrides
                .and_then(|records| records.iter().find(|record| record.player_id == p.id));
            let (initial_steel, initial_oil) = override_record
                .filter(|_| catalog.is_some())
                .map(|record| (record.starting_steel, record.starting_oil))
                .or(resource_override)
                .filter(|_| catalog.is_some())
                .or_else(|| loadout.map(|loadout| (loadout.initial_steel, loadout.initial_oil)))
                .unwrap_or((0, 0));
            let mut ps = PlayerState {
                id: p.id,
                team_id: super::teams::normalize_team_id(p.id, p.team_id),
                faction_id: faction_id.clone(),
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile: start,
                steel: initial_steel,
                oil: initial_oil,
                supply_used: 0,
                is_ai: p.is_ai,
                score: ScoreState::default(),
                upgrades: Default::default(),
                ability_cooldowns: Default::default(),
                auto_build: Default::default(),
            };
            if let Some(loadout) = loadout {
                spawn_player_start(&mut entities, &map, &mut ps, start, loadout);
                starts_with_resources.push(start);
            }
            if let Some(loadout) = loadout {
                for &upgrade in loadout.opening_upgrades {
                    if let Ok(kind) = upgrade::UpgradeKind::from_str(upgrade) {
                        ps.upgrades.insert(kind);
                    }
                }
            }
            crate::game::services::production::sync_owned_upgrade_effects(
                &mut entities,
                ps.id,
                &ps.upgrades,
            );
            let loadout_id = override_record
                .filter(|_| catalog.is_some())
                .map(|record| record.loadout_id.clone())
                .or_else(|| loadout.map(|loadout| loadout.id.to_string()))
                .unwrap_or_else(|| format!("{faction_id}.invalid"));
            resolved_starting_loadouts.push(PlayerStartingLoadout {
                player_id: p.id,
                faction_id,
                loadout_id,
                starting_steel: initial_steel,
                starting_oil: initial_oil,
            });
            player_states.push(ps);
        }

        // Every authored base site receives resources. Claimed sites already received theirs
        // alongside the player's Resource Depot; every other site remains an available expansion.
        for site in &map.base_sites {
            if !starts_with_resources.contains(site) {
                spawn_base_resources(&mut entities, &map, *site);
            }
        }

        let authored_neutral_buildings = spawn_authored_map_entities(&map, &mut entities);

        let derived = live_derived_state(&map, &entities, 0);
        let mut game = Game {
            state: GameState::new(
                map,
                entities,
                fog,
                player_states,
                seed,
                resolved_starting_loadouts,
                map_metadata,
                starting_loadout,
            ),
            derived,
        };
        // Initialize supply accounting and fog so the very first snapshot is correct.
        systems::recompute_supply(&mut game.state.players, &game.state.entities);
        let ids = game.state.player_ids();
        game.recompute_live_fog(&ids);
        game.state.fog.explore_building_footprints(
            &ids,
            &authored_neutral_buildings,
            &game.state.entities,
            &game.state.map,
        );
        game.state.building_memory.seed_authored_neutral_buildings(
            &ids,
            &authored_neutral_buildings,
            &game.state.entities,
            &game.state.map,
            game.state.tick,
        );
        game.refresh_fog_memories(&ids);
        game
    }

    /// Static info for the `start` message: terrain grid + each player's start tile. The
    /// `player_id` is left 0; the networking layer overwrites it per recipient.
    pub fn start_payload(&self) -> StartPayload {
        let overlays = self.state.map.protocol_overlay_tiles();
        let resources = self
            .state
            .entities
            .iter()
            .filter(|e| e.kind.is_node())
            .map(|e| ResourceNode {
                id: e.id,
                kind: crate::protocol::kind_to_wire(e.kind).to_string(),
                x: e.pos_x,
                y: e.pos_y,
            })
            .collect();
        let map = MapInfo {
            width: self.state.map.width,
            height: self.state.map.height,
            tile_size: config::TILE_SIZE,
            terrain: self.state.map.terrain.clone(),
            elevation: self
                .state
                .map
                .sun
                .map(|_| self.state.map.elevation.clone())
                .unwrap_or_default(),
            sun: self.state.map.sun,
            resources,
            // Entity-backed authored objects must not bypass fog through the shared start payload.
            // They remain in the authoritative map for Lab/editor round-tripping, while clients
            // learn about their live entities only through recipient-filtered snapshots.
            doodads: self
                .state
                .map
                .doodads
                .iter()
                .filter(|doodad| !crate::game::map::doodads::is_tank_trap(doodad))
                .cloned()
                .collect(),
            concealment_tiles: overlays.concealment,
            no_vehicle_tiles: overlays.no_vehicle,
            no_building_tiles: overlays.no_building,
            no_entrenchment_tiles: overlays.no_entrenchment,
            damage_reduction_tiles: overlays.damage_reduction,
            slow_movement_tiles: overlays.slow_movement,
        };
        let players = self
            .state
            .players
            .iter()
            .map(|p| PlayerStart {
                id: p.id,
                team_id: p.team_id,
                faction_id: p.faction_id.clone(),
                name: p.name.clone(),
                color: p.color.clone(),
                is_ai: p.is_ai,
                start_tile_x: p.start_tile.0,
                start_tile_y: p.start_tile.1,
            })
            .collect();
        StartPayload {
            player_id: 0,
            spectator: false,
            prediction_build_id: None,
            prediction_version: 0,
            match_run_id: None,
            capabilities: Default::default(),
            diagnostics: Default::default(),
            replay: None,
            lab: None,
            observer_view: None,
            tick: self.state.tick,
            map,
            players,
        }
    }
}

/// Materialize entity-backed authored map objects exactly as a new live match does.
///
/// Callers receive the ids of the spawned neutral buildings so match setup can seed shared map
/// knowledge without broadening that treatment to neutral buildings created during play. These
/// remain ordinary entities rather than permanent occupancy bits: once a tank trap is destroyed
/// or deconstructed, rebuilding occupancy from the live store stops blocking it.
pub(in crate::game) fn spawn_authored_map_entities(
    map: &Map,
    entities: &mut EntityStore,
) -> Vec<u32> {
    let mut spawned = Vec::new();
    for doodad in map
        .doodads
        .iter()
        .filter(|doodad| crate::game::map::doodads::is_tank_trap(doodad))
    {
        if let Some(id) = entities.spawn_building(
            0,
            EntityKind::TankTrap,
            doodad.x as f32,
            doodad.y as f32,
            true,
        ) {
            spawned.push(id);
        }
    }
    spawned
}

pub(in crate::game) fn live_derived_state(
    map: &Map,
    entities: &EntityStore,
    tick: u32,
) -> DerivedState {
    let mut derived = DerivedState::new(
        map,
        entities,
        LIVE_PATHING_DEFAULT_BUDGET,
        LIVE_PATHING_CACHE_CAPACITY,
    );
    derived.advance_pathing_tick(tick);
    derived
}

fn default_map_metadata() -> MapMetadata {
    Map::metadata_for_name("Chokes").unwrap_or_else(|_| dev_map_metadata("Chokes"))
}

fn dev_map_metadata(name: &str) -> MapMetadata {
    MapMetadata {
        name: name.to_string(),
        schema_version: crate::game::map::CURRENT_MAP_VERSION,
        content_hash: "dev-generated".to_string(),
    }
}

fn spawn_base_resources(entities: &mut EntityStore, map: &Map, tile: (u32, u32)) -> Vec<u32> {
    let mut spawned = Vec::new();
    let (tx, ty) = tile;
    let (hx, hy) = map.tile_center(tx, ty);
    let ts = config::TILE_SIZE as f32;

    let dx = map.world_width_px() * 0.5 - hx;
    let dy = map.world_height_px() * 0.5 - hy;
    let base_angle = dy.atan2(dx);

    let inward_x = base_angle.cos();
    let inward_y = base_angle.sin();
    let lateral_x = -inward_y;
    let lateral_y = inward_x;

    let counts = map.resource_counts_at(tile);
    let patches = counts.steel_patches;
    let field_counts = [patches.div_ceil(2), patches / 2];
    let mut patch_index = 0;
    for (side, field_patches) in [1.0, -1.0].into_iter().zip(field_counts) {
        if field_patches == 0 {
            continue;
        }
        let block_dist = side * config::STEEL_BLOCK_DIST_TILES * ts;
        let block_cx = hx + block_dist * lateral_x;
        let block_cy = hy + block_dist * lateral_y;
        let rows = field_patches.div_ceil(STEEL_FIELD_COLUMNS);
        let row_center = (rows - 1) as f32 / 2.0;
        let row_spacing = if rows > 1 { ts / (rows - 1) as f32 } else { ts };
        let col_center = (STEEL_FIELD_COLUMNS - 1) as f32 / 2.0;
        for i in 0..field_patches {
            let col = (i % STEEL_FIELD_COLUMNS) as f32;
            let row = (i / STEEL_FIELD_COLUMNS) as f32;
            let off_x = (col - col_center) * ts;
            let off_y = (row - row_center) * row_spacing;
            let px = block_cx + off_x * inward_x + off_y * lateral_x;
            let py = block_cy + off_x * inward_y + off_y * lateral_y;
            let dist_tiles = ((px - hx).powi(2) + (py - hy).powi(2)).sqrt() / ts;
            debug_assert!(
                (config::START_RESOURCE_MIN_DIST_TILES..=config::START_RESOURCE_MAX_DIST_TILES)
                    .contains(&dist_tiles),
                "steel patch {patch_index} at {dist_tiles:.2} tiles from Resource Depot is out of [{:.1}, {:.1}] bounds",
                config::START_RESOURCE_MIN_DIST_TILES,
                config::START_RESOURCE_MAX_DIST_TILES
            );
            if let Some(id) = entities.spawn_node(EntityKind::Steel, px, py) {
                spawned.push(id);
            }
            patch_index += 1;
        }
    }

    let outward_x = -inward_x;
    let outward_y = -inward_y;
    let mut oil_tiles = resource_placement::occupied_resource_tiles(map, entities, EntityKind::Oil);
    let blocked_pump_jack_tiles = resource_placement::resource_blocked_building_tiles(
        map,
        entities,
        EntityKind::PumpJack,
        Some(EntityKind::Oil),
    );
    for i in 0..counts.oil_patches {
        let (outward_tiles, lateral_tiles) = oil_patch_local_offset(i, counts.oil_patches);
        let desired_x = hx + (outward_tiles * outward_x + lateral_tiles * lateral_x) * ts;
        let desired_y = hy + (outward_tiles * outward_y + lateral_tiles * lateral_y) * ts;
        let (px, py, tile) = resource_placement::nearest_oil_patch_tile_center(
            map,
            desired_x,
            desired_y,
            hx,
            hy,
            &oil_tiles,
            &blocked_pump_jack_tiles,
        );
        oil_tiles.insert(tile);
        let dist_tiles = ((px - hx).powi(2) + (py - hy).powi(2)).sqrt() / ts;
        debug_assert!(
            (config::START_RESOURCE_MIN_DIST_TILES..=config::START_RESOURCE_MAX_DIST_TILES)
                .contains(&dist_tiles),
            "oil patch {i} at {dist_tiles:.2} tiles from Resource Depot is out of [{:.1}, {:.1}] bounds",
            config::START_RESOURCE_MIN_DIST_TILES,
            config::START_RESOURCE_MAX_DIST_TILES
        );
        if let Some(id) = entities.spawn_node(EntityKind::Oil, px, py) {
            spawned.push(id);
        }
    }
    spawned
}

fn oil_patch_local_offset(index: u32, count: u32) -> (f32, f32) {
    // Keep the cluster centred on the outward ray. Odd-sized clusters use one centre patch;
    // the remaining patches are added as matching lateral pairs.
    const PAIRS: [(f32, f32); 4] = [(6.0, 2.0), (5.0, 4.0), (3.0, 4.0), (3.0, 2.0)];
    let has_centre = count % 2 == 1;
    if has_centre && index == 0 {
        return (6.0, 0.0);
    }

    let paired_index = index.saturating_sub(u32::from(has_centre));
    let pair = PAIRS[(paired_index / 2).min((PAIRS.len() - 1) as u32) as usize];
    let lateral = if paired_index % 2 == 0 {
        -pair.1
    } else {
        pair.1
    };
    (pair.0, lateral)
}

/// Spawn a player's catalog-defined starting entities and resource clusters.
fn spawn_player_start(
    entities: &mut EntityStore,
    map: &Map,
    player: &mut PlayerState,
    start: (u32, u32),
    loadout: FactionLoadout,
) {
    let (stx, sty) = start;
    let (hx, hy) = map.tile_center(stx, sty);
    let ts = config::TILE_SIZE as f32;

    for group in loadout.starting_entities {
        if group.formation == StartingFormation::ResourcePatches {
            continue;
        }
        for i in 0..group.count {
            let (x, y) = match group.formation {
                StartingFormation::Center => (hx, hy),
                StartingFormation::Ring { radius_tiles_x10 } => {
                    let ring_r = ts * (radius_tiles_x10 as f32 / 10.0);
                    let ang = std::f32::consts::TAU * (i as f32) / (group.count.max(1) as f32);
                    (hx + ring_r * ang.cos(), hy + ring_r * ang.sin())
                }
                StartingFormation::ResourcePatches => continue,
            };
            let spawned = if group.kind.is_building() {
                entities.spawn_building(player.id, group.kind, x, y, group.completed)
            } else if group.kind.is_unit() {
                entities.spawn_unit(player.id, group.kind, x, y)
            } else {
                None
            };
            if spawned.is_some() {
                player.record_entity_created(group.kind);
            }
        }
    }

    let base_resource_ids = spawn_base_resources(entities, map, start);
    for kind in
        resource_patch_start::spawn(entities, player.id, loadout, &base_resource_ids, hx, hy)
    {
        player.record_entity_created(kind);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum StartingLoadout {
    Standard,
}

#[cfg(test)]
mod tests;
