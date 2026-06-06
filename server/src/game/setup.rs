use super::*;

impl Game {
    #[allow(dead_code)]
    pub fn new(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            true,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            AiProfileSelection::Default,
            StartingLoadout::Standard,
        )
    }

    /// Create a live lobby match where each AI picks one strategy from the live profile pool.
    pub fn new_with_random_ai_profiles(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            true,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            AiProfileSelection::Random,
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
        Self::new_inner(
            players,
            true,
            steel,
            oil,
            seed,
            AiProfileSelection::Default,
            StartingLoadout::Standard,
        )
    }

    /// Create a live lobby match with explicit starting resources and randomized AI strategies.
    #[allow(dead_code)]
    pub fn new_with_starting_resources_and_random_ai_profiles(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            true,
            steel,
            oil,
            seed,
            AiProfileSelection::Random,
            StartingLoadout::Standard,
        )
    }

    /// Create a debug lobby match with boosted resources and a prebuilt human-only loadout.
    pub fn new_with_debug_starting_loadout_and_random_ai_profiles(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            true,
            steel,
            oil,
            seed,
            AiProfileSelection::Random,
            StartingLoadout::DebugHuman,
        )
    }

    #[cfg(test)]
    pub(crate) fn new_for_replay(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_without_ai_controllers(players, seed)
    }

    /// Like [`Game::new_for_replay`] but with explicit starting resources. Used when replaying a
    /// match that was originally created in debug mode so the initial player economy matches the
    /// live recording.
    pub(crate) fn new_for_replay_with_starting_resources(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
    ) -> Game {
        Self::new_inner(
            players,
            false,
            steel,
            oil,
            seed,
            AiProfileSelection::Default,
            StartingLoadout::Standard,
        )
    }

    /// Create a match that preserves player identity flags but does not attach live
    /// [`AiController`]s. Used by command-log replay and scripted self-play, where commands come
    /// from an external driver.
    pub(crate) fn new_without_ai_controllers(players: &[PlayerInit], seed: u32) -> Game {
        Self::new_inner(
            players,
            false,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            AiProfileSelection::Default,
            StartingLoadout::Standard,
        )
    }

    pub(crate) fn new_scout_car_snaking_corridor_scenario(
        car_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(car_count, 1 | 4) {
            return Err(format!("unsupported scout-car count {car_count}"));
        }

        let (map, start_tile, start, goal) = scout_car_snaking_corridor_map();
        let mut entities = EntityStore::new();
        let scouts = spawn_snaking_corridor_scout_cars(&mut entities, car_count, start)?;
        let player_id = 1;
        let player = PlayerState {
            id: player_id,
            name: "Scenario".to_string(),
            color: "#4878c8".to_string(),
            start_tile,
            steel: 0,
            oil: 10_000,
            supply_used: 0,
            supply_cap: 0,
            is_ai: false,
            score: ScoreState::default(),
        };

        let spatial = services::spatial::SpatialIndex::build(&entities, map.size);
        let pathing = services::pathing::PathingService::new(65_536, 256);
        let rng = SmallRng::seed_from_u64(seed as u64);
        let mut game = Game {
            map,
            entities,
            fog: Fog::new(96),
            players: vec![player],
            ai: Vec::new(),
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            seed,
            starting_steel: 0,
            starting_oil: 0,
            debug_path_overlays: true,
            rng,
        };
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog = Fog::new(game.map.size);
        game.fog.recompute(&ids, &game.entities, &game.map);

        Ok(DevScenarioSetup {
            game,
            player_id,
            units: scouts,
            goal,
        })
    }

    pub(crate) fn seed(&self) -> u32 {
        self.seed
    }

    pub(crate) fn starting_steel(&self) -> u32 {
        self.starting_steel
    }

    pub(crate) fn starting_oil(&self) -> u32 {
        self.starting_oil
    }

    #[cfg(test)]
    pub(super) fn ai_profile_ids(&self) -> Vec<&'static str> {
        self.ai.iter().map(AiController::profile_id).collect()
    }

    fn new_inner(
        players: &[PlayerInit],
        enable_ai: bool,
        steel: u32,
        oil: u32,
        seed: u32,
        ai_profile_selection: AiProfileSelection,
        starting_loadout: StartingLoadout,
    ) -> Game {
        let map = Map::generate(players.len(), seed);
        let fog = Fog::new(map.size);
        let mut entities = EntityStore::new();
        let mut ai_profile_rng = SmallRng::seed_from_u64((seed as u64) ^ 0xA17E_5EED);

        let mut player_states = Vec::with_capacity(players.len());
        let mut ai = Vec::new();
        for (i, p) in players.iter().enumerate() {
            let start = map.starts.get(i).copied().unwrap_or((0, 0));
            if enable_ai && p.is_ai {
                let profile_id = match ai_profile_selection {
                    AiProfileSelection::Default => ai::DEFAULT_LIVE_PROFILE_ID,
                    AiProfileSelection::Random => ai::random_live_profile_id(&mut ai_profile_rng),
                };
                ai.push(AiController::with_profile_id(p.id, profile_id));
            }
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
            };
            spawn_player_start(&mut entities, &map, &mut ps, start);
            if starting_loadout == StartingLoadout::DebugHuman && !p.is_ai {
                spawn_debug_human_start(&mut entities, &map, &mut ps, start);
            }
            // The starting City Centre contributes supply immediately.
            ps.supply_cap = config::CITY_CENTRE_SUPPLY.min(config::SUPPLY_CAP_MAX);
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
            players: player_states,
            ai,
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            seed,
            starting_steel: steel,
            starting_oil: oil,
            debug_path_overlays: starting_loadout == StartingLoadout::DebugHuman,
            rng,
        };
        // Initialize supply accounting and fog so the very first snapshot is correct.
        systems::recompute_supply(&mut game.players, &game.entities);
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog.recompute(&ids, &game.entities, &game.map);
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
                kind: e.kind.to_protocol_str().to_string(),
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
            tick: self.tick,
            map,
            players,
        }
    }
}

pub(crate) struct DevScenarioSetup {
    pub(crate) game: Game,
    pub(crate) player_id: u32,
    pub(crate) units: Vec<u32>,
    pub(crate) goal: (f32, f32),
}

fn flat_dev_map(player_count: usize) -> Map {
    let mut map = Map::generate(player_count, 0xC0FF_EE01);
    for terrain in &mut map.terrain {
        *terrain = crate::protocol::terrain::GRASS;
    }
    map.expansion_sites.clear();
    map
}

fn block_rect_tiles(map: &mut Map, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::ROCK;
        }
    }
}

fn carve_rect_tiles(map: &mut Map, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
    for ty in min_y..=max_y {
        for tx in min_x..=max_x {
            let idx = map.index(tx, ty);
            map.terrain[idx] = crate::protocol::terrain::GRASS;
        }
    }
}

fn carve_horizontal_corridor(map: &mut Map, min_x: u32, max_x: u32, center_y: u32) {
    carve_rect_tiles(map, min_x, center_y - 1, max_x, center_y + 1);
}

fn carve_vertical_corridor(map: &mut Map, center_x: u32, min_y: u32, max_y: u32) {
    carve_rect_tiles(map, center_x - 1, min_y, center_x + 1, max_y);
}

type ScoutCarCorridorLayout = (Map, (u32, u32), (f32, f32), (f32, f32));

fn scout_car_snaking_corridor_map() -> ScoutCarCorridorLayout {
    let mut map = flat_dev_map(1);
    let stone_min_y = 15u32;
    let stone_max_y = 75u32;
    let exit_x = 36u32;
    let first_left_x = 26u32;
    let right_x = 56u32;
    let lower_lane_y = 68u32;
    let middle_lane_y = 64u32;
    let upper_lane_y = 60u32;

    let stone_max_x = map.size - 1;
    block_rect_tiles(&mut map, 0, stone_min_y, stone_max_x, stone_max_y);

    carve_vertical_corridor(&mut map, exit_x, lower_lane_y, stone_max_y);
    carve_horizontal_corridor(&mut map, first_left_x, exit_x, lower_lane_y);
    carve_vertical_corridor(&mut map, first_left_x, middle_lane_y, lower_lane_y);
    carve_horizontal_corridor(&mut map, first_left_x, right_x, middle_lane_y);
    carve_vertical_corridor(&mut map, right_x, upper_lane_y, middle_lane_y);
    carve_horizontal_corridor(&mut map, exit_x, right_x, upper_lane_y);
    carve_vertical_corridor(&mut map, exit_x, stone_min_y, upper_lane_y);

    let ts = config::TILE_SIZE as f32;
    let start_tile = (exit_x, stone_max_y + 5);
    let start = map.tile_center(start_tile.0, start_tile.1);
    let exit = map.tile_center(exit_x, stone_min_y - 1);
    let goal = (exit.0 + ts * 10.0, exit.1 - ts * 10.0);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }

    (map, start_tile, start, goal)
}

fn spawn_snaking_corridor_scout_cars(
    entities: &mut EntityStore,
    car_count: usize,
    start: (f32, f32),
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let positions: Vec<(f32, f32)> = match car_count {
        1 => vec![start],
        4 => {
            let x_spacing = config::SCOUT_CAR_BODY_WIDTH_PX * 1.5;
            let y_spacing = config::SCOUT_CAR_BODY_LENGTH_PX * 1.5;
            vec![
                (start.0 - x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 - x_spacing * 0.5, start.1 + y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 + y_spacing * 0.5),
            ]
        }
        _ => return Err(format!("unsupported scout-car count {car_count}")),
    };

    let mut scouts = Vec::with_capacity(positions.len());
    for (x, y) in positions {
        let scout = entities
            .spawn_unit(1, EntityKind::ScoutCar, x, y)
            .ok_or_else(|| "failed to spawn scout car".to_string())?;
        if let Some(e) = entities.get_mut(scout) {
            e.set_facing(north);
        }
        scouts.push(scout);
    }
    Ok(scouts)
}

/// Spawn a player's full starting layout: a free, fully-built City Centre on the start tile, a ring of
/// workers around it, and a nearby neutral resource cluster (steel + one oil node).
///
/// Spawn the steel and oil clusters for a base site. The clusters point inward toward the map
/// center so the layout is the same regardless of whether a player occupies the site.
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum StartingLoadout {
    Standard,
    DebugHuman,
}

/// Spawn the debug-mode extras for a human player. Default starts already include four workers,
/// so this adds one more worker plus five of every combat unit for a final five of each unit kind.
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
        (EntityKind::Factory, -4.0, 17.0),
        (EntityKind::Factory, 4.0, 17.0),
    ];
    const DEBUG_UNITS: &[(EntityKind, u32)] = &[
        (EntityKind::Worker, 1),
        (EntityKind::Rifleman, 5),
        (EntityKind::MachineGunner, 5),
        (EntityKind::AtTeam, 5),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn owned_kind_count(game: &Game, owner: u32, kind: EntityKind) -> usize {
        game.entities
            .iter()
            .filter(|e| e.owner == owner && e.kind == kind)
            .count()
    }

    #[test]
    fn debug_starting_loadout_applies_to_humans_only() {
        let players = [
            PlayerInit {
                id: 1,
                name: "Human".to_string(),
                color: "#cc1111".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                name: "AI".to_string(),
                color: "#1133bb".to_string(),
                is_ai: true,
            },
        ];
        let game = Game::new_with_debug_starting_loadout_and_random_ai_profiles(
            &players,
            config::QUICKSTART_STEEL,
            config::QUICKSTART_OIL,
            1,
        );

        assert_eq!(owned_kind_count(&game, 1, EntityKind::Depot), 5);
        assert_eq!(owned_kind_count(&game, 1, EntityKind::Steelworks), 1);
        assert_eq!(owned_kind_count(&game, 1, EntityKind::TrainingCentre), 1);
        assert_eq!(owned_kind_count(&game, 1, EntityKind::Barracks), 2);
        assert_eq!(owned_kind_count(&game, 1, EntityKind::Factory), 2);
        for kind in [
            EntityKind::Worker,
            EntityKind::Rifleman,
            EntityKind::MachineGunner,
            EntityKind::AtTeam,
            EntityKind::ScoutCar,
            EntityKind::Tank,
        ] {
            assert_eq!(owned_kind_count(&game, 1, kind), 5, "{kind}");
        }

        assert_eq!(owned_kind_count(&game, 2, EntityKind::Depot), 0);
        assert_eq!(owned_kind_count(&game, 2, EntityKind::Barracks), 0);
        assert_eq!(
            owned_kind_count(&game, 2, EntityKind::Worker),
            config::STARTING_WORKERS as usize
        );
    }
}
