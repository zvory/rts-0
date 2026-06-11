use super::*;
use crate::game::entity::WeaponSetup;

mod dev_scenarios;

impl Game {
    #[allow(dead_code)]
    pub fn new(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            StartingLoadout::Standard,
        )
    }

    /// Compatibility constructor retained for callers that still name live AI profile setup.
    /// AI controllers are owned by the caller, not by `Game`.
    #[allow(dead_code)]
    pub fn new_with_random_ai_profiles(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            StartingLoadout::Standard,
        )
    }

    /// Create a match with explicit starting resources for every player.
    #[allow(dead_code)]
    pub fn new_with_starting_resources(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(players, steel, oil, seed, StartingLoadout::Standard)
    }

    /// Compatibility constructor retained for callers that still name live AI profile setup.
    /// AI controllers are owned by the caller, not by `Game`.
    #[allow(dead_code)]
    pub fn new_with_starting_resources_and_random_ai_profiles(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(players, steel, oil, seed, StartingLoadout::Standard)
    }

    /// Create a debug lobby match with boosted resources and a prebuilt human-only loadout.
    #[allow(dead_code)]
    pub fn new_with_debug_starting_loadout_and_random_ai_profiles(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(players, steel, oil, seed, StartingLoadout::DebugHuman)
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
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            StartingLoadout::Standard,
            Some(map),
            map_metadata,
        )
    }

    /// Like [`Game::new_with_debug_starting_loadout_and_random_ai_profiles`] but uses a
    /// pre-loaded [`Map`].
    pub fn new_with_debug_starting_loadout_and_random_ai_profiles_and_map(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        map: Map,
    ) -> Game {
        Self::new_with_debug_starting_loadout_and_random_ai_profiles_and_map_metadata(
            players,
            steel,
            oil,
            seed,
            map,
            default_map_metadata(),
        )
    }

    pub fn new_with_debug_starting_loadout_and_random_ai_profiles_and_map_metadata(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Game {
        Self::new_inner_with_map(
            players,
            steel,
            oil,
            seed,
            StartingLoadout::DebugHuman,
            Some(map),
            map_metadata,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_replay(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_without_ai_controllers(players, seed)
    }

    /// Like [`Game::new_for_replay`] but with explicit starting resources. Used when replaying a
    /// match that was originally created in debug mode so the initial player economy matches the
    /// live recording.
    pub fn new_for_replay_with_starting_resources(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(players, steel, oil, seed, StartingLoadout::Standard)
    }

    /// Rebuild a replay from an explicit map and starting loadout. Replay playback owns command
    /// injection externally, so no live AI controllers are attached.
    pub fn new_for_replay_with_map_metadata(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        starting_loadout: crate::game::replay::ReplayStartingLoadoutMode,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Game {
        let starting_loadout = match starting_loadout {
            crate::game::replay::ReplayStartingLoadoutMode::Standard => StartingLoadout::Standard,
            crate::game::replay::ReplayStartingLoadoutMode::DebugHuman => {
                StartingLoadout::DebugHuman
            }
        };
        Self::new_inner_with_map(
            players,
            steel,
            oil,
            seed,
            starting_loadout,
            Some(map),
            map_metadata,
        )
    }

    /// Create a match that preserves player identity flags but does not attach live
    /// controllers. Used by command-log replay and scripted self-play, where commands come from
    /// an external driver.
    pub fn new_without_ai_controllers(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            StartingLoadout::Standard,
        )
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn starting_steel(&self) -> u32 {
        self.starting_steel
    }

    pub fn starting_oil(&self) -> u32 {
        self.starting_oil
    }

    pub(crate) fn starting_loadout(&self) -> StartingLoadout {
        self.starting_loadout
    }

    pub fn map_metadata(&self) -> &MapMetadata {
        &self.map_metadata
    }

    fn new_inner(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        starting_loadout: StartingLoadout,
    ) -> Game {
        Self::new_inner_with_map(
            players,
            steel,
            oil,
            seed,
            starting_loadout,
            None,
            default_map_metadata(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_inner_with_map(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        starting_loadout: StartingLoadout,
        map_override: Option<Map>,
        map_metadata: MapMetadata,
    ) -> Game {
        let map = map_override.unwrap_or_else(|| Map::generate(players.len(), seed));
        let fog = Fog::new(map.size);
        let mut entities = EntityStore::new();

        let mut player_states = Vec::with_capacity(players.len() + 1);
        for (i, p) in players.iter().enumerate() {
            let start = map.starts.get(i).copied().unwrap_or((0, 0));
            let mut ps = PlayerState {
                id: p.id,
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile: start,
                steel,
                oil,
                supply_used: 0,
                supply_cap: 0,
                is_ai: p.is_ai,
                score: ScoreState::default(),
                upgrades: Default::default(),
            };
            spawn_player_start(&mut entities, &map, &mut ps, start);
            if starting_loadout == StartingLoadout::DebugHuman && !p.is_ai {
                spawn_debug_human_start(&mut entities, &map, &mut ps, start);
            }
            // The starting City Centre contributes supply immediately.
            ps.set_supply_counts(0, config::CITY_CENTRE_SUPPLY);
            player_states.push(ps);
        }

        if starting_loadout == StartingLoadout::DebugHuman {
            spawn_debug_inert_enemy_mortar_corner(
                &mut entities,
                &map,
                &mut player_states,
                players,
            );
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
            smokes: SmokeCloudStore::new(),
            mortar_shells: crate::game::mortar::MortarShellStore::default(),
            artillery_shells: crate::game::artillery::ArtilleryShellStore::default(),
            seed,
            starting_steel: steel,
            starting_oil: oil,
            map_metadata,
            debug_path_overlays: starting_loadout == StartingLoadout::DebugHuman,
            starting_loadout,
            rng,
        };
        // Initialize supply accounting and fog so the very first snapshot is correct.
        systems::recompute_supply(&mut game.players, &game.entities);
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog
            .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);
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
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile_x: p.start_tile.0,
                start_tile_y: p.start_tile.1,
            })
            .collect();
        StartPayload {
            player_id: 0,
            spectator: false,
            debug_mode: self.debug_path_overlays,
            replay: None,
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
    let oil_perp_x = -oil_angle.sin();
    let oil_perp_y = oil_angle.cos();
    let oil_dist = config::OIL_DIST_TILES * ts;
    let oil_cx = hx + oil_dist * oil_angle.cos();
    let oil_cy = hy + oil_dist * oil_angle.sin();
    for i in 0..config::OIL_PATCHES_PER_BASE {
        let (off_x, off_y) = match i {
            0 => (-0.5 * ts, -0.5 * ts),
            1 => (0.5 * ts, -0.5 * ts),
            _ => (0.0, 0.5 * ts),
        };
        let px = oil_cx + off_x * oil_perp_x + off_y * oil_angle.cos();
        let py = oil_cy + off_x * oil_perp_y + off_y * oil_angle.sin();
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

/// Spawn a City Centre, starting workers, and resource clusters for one player.
fn spawn_player_start(
    entities: &mut EntityStore,
    map: &Map,
    player: &mut PlayerState,
    start: (u32, u32),
) {
    let (stx, sty) = start;
    let (hx, hy) = map.tile_center(stx, sty);

    if entities
        .spawn_building(player.id, EntityKind::CityCentre, hx, hy, true)
        .is_some()
    {
        player.record_entity_created(EntityKind::CityCentre);
    }

    let ts = config::TILE_SIZE as f32;
    let ring_r = ts * 2.5;
    let count = config::STARTING_WORKERS;
    for i in 0..count {
        let ang = std::f32::consts::TAU * (i as f32) / (count.max(1) as f32);
        let wx = hx + ring_r * ang.cos();
        let wy = hy + ring_r * ang.sin();
        if entities
            .spawn_unit(player.id, EntityKind::Worker, wx, wy)
            .is_some()
        {
            player.record_entity_created(EntityKind::Worker);
        }
    }

    spawn_base_resources(entities, map, start);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartingLoadout {
    Standard,
    DebugHuman,
}

/// Spawn the debug-mode extras for a human player. Default starts already include four workers,
/// so this adds one more worker plus five of every combat unit for a final five of each unit kind.
/// It also seeds a side-corner stash of extra depots for fast supply-cap testing.
fn spawn_debug_human_start(
    entities: &mut EntityStore,
    map: &Map,
    player: &mut PlayerState,
    start: (u32, u32),
) {
    const DEBUG_BUILDINGS: &[(EntityKind, f32, f32)] = &[
        (EntityKind::Depot, -8.0, 7.0),
        (EntityKind::Depot, -4.0, 7.0),
        (EntityKind::Depot, 0.0, 7.0),
        (EntityKind::Depot, 4.0, 7.0),
        (EntityKind::Depot, 8.0, 7.0),
        (EntityKind::TrainingCentre, -7.0, 12.0),
        (EntityKind::Barracks, -1.5, 12.0),
        (EntityKind::Barracks, 4.0, 12.0),
        (EntityKind::Steelworks, 9.0, 12.0),
        (EntityKind::ResearchComplex, 9.0, 15.0),
        (EntityKind::Factory, -4.0, 17.0),
        (EntityKind::Factory, 4.0, 17.0),
    ];
    const DEBUG_CORNER_DEPOT_COLUMNS: u32 = 5;
    const DEBUG_CORNER_DEPOT_ROWS: u32 = 2;
    const DEBUG_UNITS: &[(EntityKind, u32)] = &[
        (EntityKind::Worker, 1),
        (EntityKind::Rifleman, 5),
        (EntityKind::MachineGunner, 5),
        (EntityKind::MortarTeam, 5),
        (EntityKind::AtTeam, 5),
        (EntityKind::Artillery, 5),
        (EntityKind::ScoutCar, 5),
        (EntityKind::Tank, 5),
    ];

    for &(kind, side_tiles, back_tiles) in DEBUG_BUILDINGS {
        let (x, y) = debug_offset_world(map, start, side_tiles, back_tiles);
        if entities
            .spawn_building(player.id, kind, x, y, true)
            .is_some()
        {
            player.record_entity_created(kind);
        }
    }

    for row in 0..DEBUG_CORNER_DEPOT_ROWS {
        for col in 0..DEBUG_CORNER_DEPOT_COLUMNS {
            let (x, y) = debug_side_corner_world(map, start, col, row);
            if entities
                .spawn_building(player.id, EntityKind::Depot, x, y, true)
                .is_some()
            {
                player.record_entity_created(EntityKind::Depot);
            }
        }
    }

    let mut slot = 0u32;
    for &(kind, count) in DEBUG_UNITS {
        for _ in 0..count {
            let row = slot / 6;
            let col = slot % 6;
            let side_tiles = if col < 3 {
                -16.0 + col as f32 * 3.0
            } else {
                10.0 + (col - 3) as f32 * 3.0
            };
            let back_tiles = -2.0 + row as f32 * 2.0;
            let (x, y) = debug_offset_world(map, start, side_tiles, back_tiles);
            if entities.spawn_unit(player.id, kind, x, y).is_some() {
                player.record_entity_created(kind);
            }
            slot += 1;
        }
    }
}

const DEBUG_INERT_ENEMY_ID: u32 = 900_001;
const DEBUG_INERT_MORTAR_COUNT: usize = 5;
const DEBUG_INERT_MORTAR_CLUMP_RADIUS_TILES: f32 = 2.0;
const DEBUG_INERT_DEPOT_CARDINAL_OFFSET_TILES: f32 = 5.0;

/// Add a static enemy mortar/scout-car clump to debug starts without creating an AI
/// controller/profile.
fn spawn_debug_inert_enemy_mortar_corner(
    entities: &mut EntityStore,
    map: &Map,
    players: &mut Vec<PlayerState>,
    inits: &[PlayerInit],
) {
    if inits.iter().any(|p| p.id == DEBUG_INERT_ENEMY_ID) || inits.iter().all(|p| p.is_ai) {
        return;
    }

    let Some((human_index, _)) = inits.iter().enumerate().find(|(_, p)| !p.is_ai) else {
        return;
    };
    let Some(&human_start) = map.starts.get(human_index) else {
        return;
    };

    let clump_tile = debug_clockwise_adjacent_corner_tile(map, human_start);
    let clump_center = map.tile_center(clump_tile.0, clump_tile.1);
    let map_center = map.world_size_px() * 0.5;
    let to_center = (map_center - clump_center.0, map_center - clump_center.1);
    let center_facing = to_center.1.atan2(to_center.0);
    if !center_facing.is_finite() {
        return;
    }

    let ts = config::TILE_SIZE as f32;
    const MORTAR_OFFSETS: [(f32, f32); DEBUG_INERT_MORTAR_COUNT] =
        [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (1.0, -1.0)];
    for (dx, dy) in MORTAR_OFFSETS {
        let x = clump_center.0 + dx * DEBUG_INERT_MORTAR_CLUMP_RADIUS_TILES * ts;
        let y = clump_center.1 + dy * DEBUG_INERT_MORTAR_CLUMP_RADIUS_TILES * ts;
        let facing = (map_center - y).atan2(map_center - x);
        let Some(id) = entities.spawn_unit(DEBUG_INERT_ENEMY_ID, EntityKind::MortarTeam, x, y)
        else {
            continue;
        };
        if let Some(e) = entities.get_mut(id) {
            e.set_facing(facing);
            e.set_weapon_facing(facing);
            e.set_desired_weapon_facing(facing);
            e.set_weapon_setup(WeaponSetup::Deployed);
        }
    }

    if let Some(id) =
        entities.spawn_unit(DEBUG_INERT_ENEMY_ID, EntityKind::ScoutCar, clump_center.0, clump_center.1)
    {
        if let Some(e) = entities.get_mut(id) {
            e.set_facing(center_facing);
            e.set_weapon_facing(center_facing);
            e.set_desired_weapon_facing(center_facing);
        }
    }

    const DEPOT_OFFSETS: [(f32, f32); 4] = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)];
    for (dx, dy) in DEPOT_OFFSETS {
        let x = clump_center.0 + dx * DEBUG_INERT_DEPOT_CARDINAL_OFFSET_TILES * ts;
        let y = clump_center.1 + dy * DEBUG_INERT_DEPOT_CARDINAL_OFFSET_TILES * ts;
        entities.spawn_building(DEBUG_INERT_ENEMY_ID, EntityKind::Depot, x, y, true);
    }

    players.push(PlayerState {
        id: DEBUG_INERT_ENEMY_ID,
        name: "Inert Mortar Corner".to_string(),
        color: "#8d2f2f".to_string(),
        start_tile: clump_tile,
        steel: 0,
        oil: 0,
        supply_used: 0,
        supply_cap: 0,
        is_ai: true,
        score: ScoreState::default(),
        upgrades: Default::default(),
    });
}

fn debug_clockwise_adjacent_corner_tile(map: &Map, start: (u32, u32)) -> (u32, u32) {
    let max_tile = map.size.saturating_sub(1);
    let start_x = start.0.min(max_tile);
    let start_y = start.1.min(max_tile);
    (max_tile.saturating_sub(start_y), start_x)
}

fn debug_offset_world(
    map: &Map,
    start: (u32, u32),
    side_tiles: f32,
    back_tiles: f32,
) -> (f32, f32) {
    let (hx, hy) = map.tile_center(start.0, start.1);
    let mid = map.size / 2;
    let back_x = if start.0 < mid { -1.0 } else { 1.0 };
    let back_y = if start.1 < mid { -1.0 } else { 1.0 };
    let side_x = -back_y;
    let side_y = back_x;
    let ts = config::TILE_SIZE as f32;
    (
        hx + (side_x * side_tiles + back_x * back_tiles) * ts,
        hy + (side_y * side_tiles + back_y * back_tiles) * ts,
    )
}

fn debug_side_corner_world(map: &Map, start: (u32, u32), col: u32, row: u32) -> (f32, f32) {
    const CORNER_INSET_TILES: u32 = 4;
    const CORNER_SPACING_TILES: u32 = 4;

    let max_tile = map.size.saturating_sub(1);
    let mid = map.size / 2;
    let x_tile = if start.0 < mid {
        CORNER_INSET_TILES + col * CORNER_SPACING_TILES
    } else {
        max_tile.saturating_sub(CORNER_INSET_TILES + col * CORNER_SPACING_TILES)
    };
    let y_tile = if start.1 < mid {
        max_tile.saturating_sub(CORNER_INSET_TILES + row * CORNER_SPACING_TILES)
    } else {
        CORNER_INSET_TILES + row * CORNER_SPACING_TILES
    };
    map.tile_center(x_tile.min(max_tile), y_tile.min(max_tile))
}

#[cfg(test)]
mod tests;
