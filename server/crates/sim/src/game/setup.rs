use super::resource_placement;
use super::*;
use crate::rules::faction::{catalog_for_or_default_empty, FactionLoadout, StartingFormation};
use std::str::FromStr;

mod dev_scenarios;

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
        self.seed
    }

    pub fn starting_steel(&self) -> u32 {
        self.starting_loadouts
            .first()
            .map(|loadout| loadout.starting_steel)
            .unwrap_or(config::STARTING_STEEL)
    }

    pub fn starting_oil(&self) -> u32 {
        self.starting_loadouts
            .first()
            .map(|loadout| loadout.starting_oil)
            .unwrap_or(config::STARTING_OIL)
    }

    pub fn starting_loadouts(&self) -> &[PlayerStartingLoadout] {
        &self.starting_loadouts
    }

    pub fn map_metadata(&self) -> &MapMetadata {
        &self.map_metadata
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
            };
            if let Some(loadout) = loadout {
                spawn_player_start(&mut entities, &map, &mut ps, start, loadout);
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

        // Always spawn resources on the neutral expansion sites. Claimed sites get a full start;
        // unclaimed sites still get their resource clusters so every player has somewhere to
        // expand.
        for site in &map.expansion_sites {
            if !map.starts.contains(site) {
                spawn_base_resources(&mut entities, &map, *site);
            }
        }

        let spatial = services::spatial::SpatialIndex::build(&entities, map.size);
        let pathing = services::pathing::PathingService::new(65_536, 256);
        let rng = SmallRng::seed_from_u64(seed as u64);
        let mut game = Game {
            map,
            entities,
            fog,
            building_memory: BuildingMemory::default(),
            players: player_states,
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            firing_reveals: Vec::new(),
            smokes: SmokeCloudStore::new(),
            ability_runtime: crate::game::ability_runtime::AbilityRuntime::new(),
            mortar_shells: crate::game::mortar::MortarShellStore::default(),
            artillery_shells: crate::game::artillery::ArtilleryShellStore::default(),
            seed,
            starting_loadouts: resolved_starting_loadouts,
            map_metadata,
            active_construction_sites: BTreeSet::new(),
            lab_god_mode_players: BTreeSet::new(),
            starting_loadout,
            rng,
        };
        // Initialize supply accounting and fog so the very first snapshot is correct.
        systems::recompute_supply(&mut game.players, &game.entities);
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.recompute_live_fog(&ids);
        game.refresh_building_memory(&ids);
        game
    }

    /// Static info for the `start` message: terrain grid + each player's start tile. The
    /// `player_id` is left 0; the networking layer overwrites it per recipient.
    pub fn start_payload(&self) -> StartPayload {
        let resources = self
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
            width: self.map.size,
            height: self.map.size,
            tile_size: config::TILE_SIZE,
            terrain: self.map.terrain.clone(),
            resources,
        };
        let players = self
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
            tick: self.tick,
            map,
            players,
        }
    }
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

    let block_dist = config::STEEL_BLOCK_DIST_TILES * ts;
    let block_cx = hx + block_dist * base_angle.cos();
    let block_cy = hy + block_dist * base_angle.sin();
    let perp_x = -base_angle.sin();
    let perp_y = base_angle.cos();

    let patches = config::STEEL_PATCHES_PER_BASE;
    let cols = 6u32;
    let rows = patches.div_ceil(cols);
    let row_center = (rows - 1) as f32 / 2.0;
    for i in 0..patches {
        let col = (i % cols) as f32;
        let row = (i / cols) as f32;
        let off_x = (col - 2.5) * ts;
        let off_y = (row - row_center) * ts;
        let px = block_cx + off_x * perp_x + off_y * base_angle.cos();
        let py = block_cy + off_x * perp_y + off_y * base_angle.sin();
        let dist_tiles = ((px - hx).powi(2) + (py - hy).powi(2)).sqrt() / ts;
        debug_assert!(
            (config::CC_RESOURCE_MIN_DIST_TILES..=config::CC_RESOURCE_MAX_DIST_TILES)
                .contains(&dist_tiles),
            "steel patch {i} at {dist_tiles:.2} tiles from City Centre is out of [{:.1}, {:.1}] bounds",
            config::CC_RESOURCE_MIN_DIST_TILES,
            config::CC_RESOURCE_MAX_DIST_TILES
        );
        entities.spawn_node(EntityKind::Steel, px, py);
    }

    let oil_angle = base_angle + std::f32::consts::FRAC_PI_2;
    let oil_step_x = tile_step(oil_angle.cos());
    let oil_step_y = tile_step(oil_angle.sin());
    let mut oil_tiles = existing_oil_tiles(map, entities);
    for i in 0..config::OIL_PATCHES_PER_BASE {
        let (tile_dx, tile_dy) = oil_patch_tile_offset(i, oil_step_x, oil_step_y);
        let (desired_x, desired_y) = offset_tile_center(map, tx, ty, tile_dx, tile_dy);
        let (px, py, tile) = oil_patch_tile_center(map, desired_x, desired_y, hx, hy, &oil_tiles);
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

fn existing_oil_tiles(map: &Map, entities: &EntityStore) -> BTreeSet<(u32, u32)> {
    entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Oil && entity.is_node())
        .map(|entity| map.tile_of(entity.pos_x, entity.pos_y))
        .collect()
}

fn tile_step(value: f32) -> i32 {
    if value < 0.0 { -1 } else { 1 }
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

fn oil_patch_tile_center(
    map: &Map,
    x: f32,
    y: f32,
    anchor_x: f32,
    anchor_y: f32,
    occupied_tiles: &BTreeSet<(u32, u32)>,
) -> (f32, f32, (u32, u32)) {
    let ts = config::TILE_SIZE as f32;
    resource_placement::nearest_oil_tile_center(map, x, y, |tile, cx, cy| {
        if !resource_placement::tile_has_one_tile_oil_gap(tile, occupied_tiles) {
            return false;
        }
        let dist_tiles = ((cx - anchor_x).powi(2) + (cy - anchor_y).powi(2)).sqrt() / ts;
        (config::CC_RESOURCE_MIN_DIST_TILES..=config::CC_RESOURCE_MAX_DIST_TILES)
            .contains(&dist_tiles)
    })
    .unwrap_or_else(|| resource_placement::nearest_tile_center(map, x, y))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartingLoadout {
    Standard,
}

#[cfg(test)]
mod tests;
