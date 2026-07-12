use super::resource_placement;
use super::*;
use crate::game::derived_state::DerivedState;
use crate::rules::faction::{catalog_for_or_default_empty, FactionLoadout, StartingFormation};
use std::str::FromStr;

mod checkpoint_start;
mod dev_scenarios;

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
        let fog = Fog::new(map.size);
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
                supply_cap: 0,
                is_ai: p.is_ai,
                score: ScoreState::default(),
                upgrades: Default::default(),
                ability_cooldowns: Default::default(),
                production_requests: Default::default(),
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
        // alongside the player's City Centre; every other site remains an available expansion.
        for site in &map.base_sites {
            if !starts_with_resources.contains(site) {
                spawn_base_resources(&mut entities, &map, *site);
            }
        }

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
        game.refresh_building_memory(&ids);
        game.refresh_trench_memory(&ids);
        game
    }

    /// Static info for the `start` message: terrain grid + each player's start tile. The
    /// `player_id` is left 0; the networking layer overwrites it per recipient.
    pub fn start_payload(&self) -> StartPayload {
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
            width: self.state.map.size,
            height: self.state.map.size,
            tile_size: config::TILE_SIZE,
            terrain: self.state.map.terrain.clone(),
            resources,
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
            tick: self.state.tick,
            map,
            players,
        }
    }
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
    Map::metadata_for_name("Default").unwrap_or_else(|_| dev_map_metadata("Default"))
}

fn dev_map_metadata(name: &str) -> MapMetadata {
    MapMetadata {
        name: name.to_string(),
        schema_version: crate::game::map::CURRENT_MAP_VERSION,
        content_hash: "dev-generated".to_string(),
    }
}

fn spawn_base_resources(entities: &mut EntityStore, map: &Map, tile: (u32, u32)) {
    let (tx, ty) = tile;
    let (hx, hy) = map.tile_center(tx, ty);
    let ts = config::TILE_SIZE as f32;

    let center = map.world_size_px() * 0.5;
    let dx = center - hx;
    let dy = center - hy;
    let base_angle = dy.atan2(dx);

    let perp_x = -base_angle.sin();
    let perp_y = base_angle.cos();

    let patches = config::STEEL_PATCHES_PER_BASE;
    let field_counts = [patches.div_ceil(2), patches / 2];
    let mut patch_index = 0;
    for (side, field_patches) in [1.0, -1.0].into_iter().zip(field_counts) {
        if field_patches == 0 {
            continue;
        }
        let block_dist = side * config::STEEL_BLOCK_DIST_TILES * ts;
        let block_cx = hx + block_dist * base_angle.cos();
        let block_cy = hy + block_dist * base_angle.sin();
        let rows = field_patches.div_ceil(STEEL_FIELD_COLUMNS);
        let row_center = (rows - 1) as f32 / 2.0;
        let col_center = (STEEL_FIELD_COLUMNS - 1) as f32 / 2.0;
        for i in 0..field_patches {
            let col = (i % STEEL_FIELD_COLUMNS) as f32;
            let row = (i / STEEL_FIELD_COLUMNS) as f32;
            let off_x = (col - col_center) * ts;
            let off_y = (row - row_center) * ts;
            let px = block_cx + off_x * perp_x + off_y * base_angle.cos();
            let py = block_cy + off_x * perp_y + off_y * base_angle.sin();
            let dist_tiles = ((px - hx).powi(2) + (py - hy).powi(2)).sqrt() / ts;
            debug_assert!(
                (config::CC_RESOURCE_MIN_DIST_TILES..=config::CC_RESOURCE_MAX_DIST_TILES)
                    .contains(&dist_tiles),
                "steel patch {patch_index} at {dist_tiles:.2} tiles from City Centre is out of [{:.1}, {:.1}] bounds",
                config::CC_RESOURCE_MIN_DIST_TILES,
                config::CC_RESOURCE_MAX_DIST_TILES
            );
            entities.spawn_node(EntityKind::Steel, px, py);
            patch_index += 1;
        }
    }

    let oil_angle = base_angle + std::f32::consts::FRAC_PI_2;
    let oil_step_x = tile_step(oil_angle.cos());
    let oil_step_y = tile_step(oil_angle.sin());
    let mut oil_tiles = resource_placement::occupied_resource_tiles(map, entities, EntityKind::Oil);
    let blocked_pump_jack_tiles = resource_placement::resource_blocked_building_tiles(
        map,
        entities,
        EntityKind::PumpJack,
        Some(EntityKind::Oil),
    );
    for i in 0..config::OIL_PATCHES_PER_BASE {
        let (tile_dx, tile_dy) = oil_patch_tile_offset(i, oil_step_x, oil_step_y);
        let (desired_x, desired_y) = offset_tile_center(map, tx, ty, tile_dx, tile_dy);
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
            (config::CC_RESOURCE_MIN_DIST_TILES..=config::CC_RESOURCE_MAX_DIST_TILES)
                .contains(&dist_tiles),
            "oil patch {i} at {dist_tiles:.2} tiles from City Centre is out of [{:.1}, {:.1}] bounds",
            config::CC_RESOURCE_MIN_DIST_TILES,
            config::CC_RESOURCE_MAX_DIST_TILES
        );
        entities.spawn_node(EntityKind::Oil, px, py);
    }
}

fn tile_step(value: f32) -> i32 {
    if value < 0.0 {
        -1
    } else {
        1
    }
}

fn oil_patch_tile_offset(index: u32, step_x: i32, step_y: i32) -> (i32, i32) {
    // Integer offsets keep mirrored starts at identical CC distances after tile snapping.
    match index {
        0 => (4 * step_x, 4 * step_y),
        1 => (4 * step_x, 2 * step_y),
        _ => (6 * step_x, 3 * step_y),
    }
}

fn offset_tile_center(map: &Map, tx: u32, ty: u32, dx: i32, dy: i32) -> (f32, f32) {
    let max_tile = map.size.saturating_sub(1) as i32;
    let desired_tx = (tx as i32 + dx).clamp(0, max_tile) as u32;
    let desired_ty = (ty as i32 + dy).clamp(0, max_tile) as u32;
    map.tile_center(desired_tx, desired_ty)
}

/// Spawn a City Centre, starting workers, and resource clusters for one player.
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
        for i in 0..group.count {
            let (x, y) = match group.formation {
                StartingFormation::Center => (hx, hy),
                StartingFormation::Ring { radius_tiles_x10 } => {
                    let ring_r = ts * (radius_tiles_x10 as f32 / 10.0);
                    let ang = std::f32::consts::TAU * (i as f32) / (group.count.max(1) as f32);
                    (hx + ring_r * ang.cos(), hy + ring_r * ang.sin())
                }
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

    spawn_base_resources(entities, map, start);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum StartingLoadout {
    Standard,
}

#[cfg(test)]
mod tests;
