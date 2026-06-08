use super::*;
use crate::game::entity::WeaponSetup;

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
        Self::new_inner_with_map(
            players,
            config::STARTING_STEEL,
            config::STARTING_OIL,
            seed,
            StartingLoadout::Standard,
            Some(map),
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
        Self::new_inner_with_map(
            players,
            steel,
            oil,
            seed,
            StartingLoadout::DebugHuman,
            Some(map),
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

    pub fn new_snaking_corridor_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !unit.is_unit() {
            return Err(format!("unsupported snaking-corridor unit {unit}"));
        }
        if !matches!(unit_count, 1 | 4) {
            return Err(format!(
                "unsupported snaking-corridor unit count {unit_count}"
            ));
        }

        let (map, start_tile, start, goal) = scout_car_snaking_corridor_map();
        let mut entities = EntityStore::new();
        let units = spawn_snaking_corridor_units(&mut entities, unit, unit_count, start)?;
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
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            smokes: SmokeCloudStore::new(),
            seed,
            starting_steel: 0,
            starting_oil: 0,
            debug_path_overlays: true,
            rng,
        };
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog = Fog::new(game.map.size);
        game.fog
            .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
        })
    }

    pub fn new_direct_reverse_order_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(
            unit,
            EntityKind::AtTeam | EntityKind::ScoutCar | EntityKind::Tank
        ) {
            return Err(format!("unsupported direct-reverse-order unit {unit}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported direct-reverse-order unit count {unit_count}"
            ));
        }

        let mut map = flat_dev_map(1);
        let start_tile = (48, 48);
        let start = map.tile_center(start_tile.0, start_tile.1);
        let goal = (start.0 - config::TILE_SIZE as f32 * 15.0, start.1);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }

        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, unit, start.0, start.1)
            .ok_or_else(|| format!("failed to spawn {unit}"))?;
        if let Some(e) = entities.get_mut(unit_id) {
            e.set_facing(0.0);
        }

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
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            smokes: SmokeCloudStore::new(),
            seed,
            starting_steel: 0,
            starting_oil: 0,
            debug_path_overlays: true,
            rng,
        };
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog = Fog::new(game.map.size);
        game.fog
            .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);

        Ok(DevScenarioSetup {
            game,
            player_id,
            units: vec![unit_id],
            goal,
        })
    }

    pub fn new_scout_car_wall_chokepoint_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(
            unit,
            EntityKind::AtTeam | EntityKind::ScoutCar | EntityKind::Tank
        ) {
            return Err(format!("unsupported wall-chokepoint unit {unit}"));
        }
        if !matches!(unit_count, 3 | 5 | 6 | 10 | 15) {
            return Err(format!(
                "unsupported wall-chokepoint unit count {unit_count}"
            ));
        }

        let (map, start_tile, starts, goal) = scout_car_wall_chokepoint_map(unit, unit_count);
        let mut entities = EntityStore::new();
        let units = spawn_wall_chokepoint_units(&mut entities, unit, starts)?;
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
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            smokes: SmokeCloudStore::new(),
            seed,
            starting_steel: 0,
            starting_oil: 0,
            debug_path_overlays: true,
            rng,
        };
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog = Fog::new(game.map.size);
        game.fog
            .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
        })
    }

    pub fn new_vehicle_small_block_baseline_scenario(
        vehicle: EntityKind,
        pair_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(vehicle, EntityKind::ScoutCar | EntityKind::Tank) {
            return Err(format!(
                "unsupported vehicle-small-block-baseline vehicle {vehicle}"
            ));
        }
        if !matches!(pair_count, 1 | 3 | 5) {
            return Err(format!(
                "unsupported vehicle-small-block-baseline pair count {pair_count}"
            ));
        }

        let (map, start_tile, vehicle_starts, blocker_starts, goal) =
            vehicle_small_block_baseline_map(vehicle, pair_count);
        let mut entities = EntityStore::new();
        let units =
            spawn_vehicle_small_block_baseline_units(&mut entities, vehicle, vehicle_starts)?;
        spawn_vehicle_small_block_baseline_blockers(&mut entities, blocker_starts)?;
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
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            smokes: SmokeCloudStore::new(),
            seed,
            starting_steel: 0,
            starting_oil: 0,
            debug_path_overlays: true,
            rng,
        };
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog = Fog::new(game.map.size);
        game.fog
            .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
        })
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

    fn new_inner(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        starting_loadout: StartingLoadout,
    ) -> Game {
        Self::new_inner_with_map(players, steel, oil, seed, starting_loadout, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn new_inner_with_map(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        starting_loadout: StartingLoadout,
        map_override: Option<Map>,
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
            };
            spawn_player_start(&mut entities, &map, &mut ps, start);
            if starting_loadout == StartingLoadout::DebugHuman && !p.is_ai {
                spawn_debug_human_start(&mut entities, &map, &mut ps, start);
            }
            // The starting City Centre contributes supply immediately.
            ps.supply_cap = config::CITY_CENTRE_SUPPLY.min(config::SUPPLY_CAP_MAX);
            player_states.push(ps);
        }

        if starting_loadout == StartingLoadout::DebugHuman {
            spawn_debug_inert_enemy_at_guns(&mut entities, &map, &mut player_states, players);
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
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
            lingering_sight: Vec::new(),
            smokes: SmokeCloudStore::new(),
            seed,
            starting_steel: steel,
            starting_oil: oil,
            debug_path_overlays: starting_loadout == StartingLoadout::DebugHuman,
            rng,
        };
        // Initialize supply accounting and fog so the very first snapshot is correct.
        systems::recompute_supply(&mut game.players, &game.entities);
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog
            .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);
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
            tick: self.tick,
            map,
            players,
        }
    }
}

pub struct DevScenarioSetup {
    pub game: Game,
    pub player_id: u32,
    pub units: Vec<u32>,
    pub goal: (f32, f32),
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

#[allow(clippy::type_complexity)]
fn scout_car_wall_chokepoint_map(
    unit: EntityKind,
    unit_count: usize,
) -> (Map, (u32, u32), Vec<(f32, f32)>, (f32, f32)) {
    let mut map = flat_dev_map(1);
    let center_x = map.size / 2;
    let wall_y = map.size - 18;
    let start_tile = (center_x, wall_y + 10);
    let gap_left_x = center_x - 1;
    let gap_right_x = center_x;
    let max_tile = map.size - 1;

    block_rect_tiles(&mut map, 0, wall_y, max_tile, wall_y);
    carve_rect_tiles(&mut map, gap_left_x, wall_y, gap_right_x, wall_y);

    let ts = config::TILE_SIZE as f32;
    let center_world_x = gap_right_x as f32 * ts;
    let start_y = (start_tile.1 as f32 + 0.5) * ts;
    let spacing = wall_chokepoint_spawn_spacing(unit);
    let center_index = (unit_count.saturating_sub(1)) as f32 * 0.5;
    let starts = (0..unit_count)
        .map(|i| {
            let offset = (i as f32 - center_index) * spacing;
            (center_world_x + offset, start_y)
        })
        .collect();
    let goal_y = (wall_y as f32 + 0.5) * ts - ts * 10.0;
    let goal = (center_world_x, goal_y);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }

    (map, start_tile, starts, goal)
}

#[allow(clippy::type_complexity)]
fn vehicle_small_block_baseline_map(
    vehicle: EntityKind,
    pair_count: usize,
) -> (
    Map,
    (u32, u32),
    Vec<(f32, f32)>,
    Vec<(f32, f32)>,
    (f32, f32),
) {
    let mut map = flat_dev_map(1);
    let center_tile = (map.size / 2, map.size / 2 + 18);
    let ts = config::TILE_SIZE as f32;
    let start_y = (center_tile.1 as f32 + 0.5) * ts;
    let center_x = (center_tile.0 as f32 + 0.5) * ts;
    let spacing = vehicle_small_block_baseline_vehicle_spacing(vehicle);
    let center_index = (pair_count.saturating_sub(1)) as f32 * 0.5;
    let vehicle_starts: Vec<(f32, f32)> = (0..pair_count)
        .map(|i| {
            let offset = (i as f32 - center_index) * spacing;
            (center_x + offset, start_y)
        })
        .collect();
    let blocker_starts = vehicle_starts.iter().map(|(x, y)| (*x, *y - ts)).collect();
    let goal = (center_x, start_y - ts * 20.0);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = center_tile;
    }

    (map, center_tile, vehicle_starts, blocker_starts, goal)
}

fn spawn_snaking_corridor_units(
    entities: &mut EntityStore,
    unit: EntityKind,
    unit_count: usize,
    start: (f32, f32),
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let (x_spacing, y_spacing) = snaking_corridor_spawn_spacing(unit)?;
    let positions: Vec<(f32, f32)> = match unit_count {
        1 => vec![start],
        4 => {
            vec![
                (start.0 - x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 - y_spacing * 0.5),
                (start.0 - x_spacing * 0.5, start.1 + y_spacing * 0.5),
                (start.0 + x_spacing * 0.5, start.1 + y_spacing * 0.5),
            ]
        }
        _ => {
            return Err(format!(
                "unsupported snaking-corridor unit count {unit_count}"
            ))
        }
    };

    let mut units = Vec::with_capacity(positions.len());
    for (x, y) in positions {
        let spawned = entities
            .spawn_unit(1, unit, x, y)
            .ok_or_else(|| format!("failed to spawn {unit}"))?;
        if let Some(e) = entities.get_mut(spawned) {
            e.set_facing(north);
        }
        units.push(spawned);
    }
    Ok(units)
}

fn spawn_wall_chokepoint_units(
    entities: &mut EntityStore,
    unit: EntityKind,
    starts: Vec<(f32, f32)>,
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let mut units = Vec::with_capacity(starts.len());
    for (x, y) in starts {
        let spawned = entities
            .spawn_unit(1, unit, x, y)
            .ok_or_else(|| format!("failed to spawn {unit}"))?;
        if let Some(e) = entities.get_mut(spawned) {
            e.set_facing(north);
        }
        units.push(spawned);
    }
    Ok(units)
}

fn spawn_vehicle_small_block_baseline_units(
    entities: &mut EntityStore,
    vehicle: EntityKind,
    starts: Vec<(f32, f32)>,
) -> Result<Vec<u32>, String> {
    let north = -std::f32::consts::FRAC_PI_2;
    let mut units = Vec::with_capacity(starts.len());
    for (x, y) in starts {
        let spawned = entities
            .spawn_unit(1, vehicle, x, y)
            .ok_or_else(|| format!("failed to spawn {vehicle}"))?;
        if let Some(e) = entities.get_mut(spawned) {
            e.set_facing(north);
        }
        units.push(spawned);
    }
    Ok(units)
}

fn spawn_vehicle_small_block_baseline_blockers(
    entities: &mut EntityStore,
    starts: Vec<(f32, f32)>,
) -> Result<(), String> {
    for (x, y) in starts {
        entities
            .spawn_unit(1, EntityKind::Worker, x, y)
            .ok_or_else(|| "failed to spawn worker blocker".to_string())?;
    }
    Ok(())
}

fn wall_chokepoint_spawn_spacing(unit: EntityKind) -> f32 {
    match unit {
        EntityKind::AtTeam => config::AT_GUN_BODY_WIDTH_PX + config::AT_GUN_BODY_CLEARANCE_PX * 4.0,
        EntityKind::ScoutCar => {
            config::SCOUT_CAR_BODY_WIDTH_PX + config::SCOUT_CAR_BODY_CLEARANCE_PX * 4.0
        }
        EntityKind::Tank => config::TANK_BODY_WIDTH_PX + config::TANK_BODY_CLEARANCE_PX * 4.0,
        _ => unreachable!("wall chokepoint only supports vehicles"),
    }
}

fn vehicle_small_block_baseline_vehicle_spacing(vehicle: EntityKind) -> f32 {
    match vehicle {
        EntityKind::ScoutCar => config::SCOUT_CAR_BODY_WIDTH_PX + 2.0,
        EntityKind::Tank => config::TANK_BODY_WIDTH_PX + 2.0,
        _ => unreachable!("vehicle small-block baseline only supports vehicles"),
    }
}

fn snaking_corridor_spawn_spacing(unit: EntityKind) -> Result<(f32, f32), String> {
    match unit {
        EntityKind::AtTeam => Ok((
            config::AT_GUN_BODY_WIDTH_PX * 1.5,
            config::AT_GUN_BODY_LENGTH_PX * 1.5,
        )),
        EntityKind::ScoutCar => Ok((
            config::SCOUT_CAR_BODY_WIDTH_PX * 1.5,
            config::SCOUT_CAR_BODY_LENGTH_PX * 1.5,
        )),
        EntityKind::Tank => Ok((
            config::TANK_BODY_WIDTH_PX * 1.5,
            config::TANK_BODY_LENGTH_PX * 1.5,
        )),
        _ => {
            let radius = config::unit_stats(unit)
                .ok_or_else(|| format!("missing stats for snaking-corridor unit {unit}"))?
                .radius;
            let spacing = radius * 3.0;
            Ok((spacing, spacing))
        }
    }
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
        (EntityKind::Factory, -4.0, 17.0),
        (EntityKind::Factory, 4.0, 17.0),
    ];
    const DEBUG_CORNER_DEPOT_COLUMNS: u32 = 5;
    const DEBUG_CORNER_DEPOT_ROWS: u32 = 2;
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
const DEBUG_INERT_AT_GUN_COUNT: usize = 5;
const DEBUG_INERT_AT_GUN_SPACING_TILES: f32 = 4.0;
const DEBUG_INERT_RIFLEMAN_SCREEN_COUNT: usize = 12;
const DEBUG_INERT_RIFLEMAN_SCREEN_SPACING_TILES: f32 = 2.0;
const DEBUG_INERT_RIFLEMAN_SCREEN_FRONT_TILES: f32 = 5.0;

/// Add a static enemy AT-gun battery and rifle screen to debug starts without creating an AI
/// controller/profile.
fn spawn_debug_inert_enemy_at_guns(
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

    let max_tile = map.size.saturating_sub(1);
    let battery_tile = (
        max_tile.saturating_sub(human_start.0.min(max_tile)),
        max_tile.saturating_sub(human_start.1.min(max_tile)),
    );
    let battery_center = map.tile_center(battery_tile.0, battery_tile.1);
    let map_center = map.world_size_px() * 0.5;
    let to_center = (map_center - battery_center.0, map_center - battery_center.1);
    let center_facing = to_center.1.atan2(to_center.0);
    if !center_facing.is_finite() {
        return;
    }

    let side = (-center_facing.sin(), center_facing.cos());
    let forward = (center_facing.cos(), center_facing.sin());
    let ts = config::TILE_SIZE as f32;
    let center_index = (DEBUG_INERT_AT_GUN_COUNT.saturating_sub(1)) as f32 * 0.5;
    for i in 0..DEBUG_INERT_AT_GUN_COUNT {
        let offset = (i as f32 - center_index) * DEBUG_INERT_AT_GUN_SPACING_TILES * ts;
        let x = battery_center.0 + side.0 * offset;
        let y = battery_center.1 + side.1 * offset;
        let facing = (map_center - y).atan2(map_center - x);
        let Some(id) = entities.spawn_unit(DEBUG_INERT_ENEMY_ID, EntityKind::AtTeam, x, y) else {
            continue;
        };
        if let Some(e) = entities.get_mut(id) {
            e.set_facing(facing);
            e.set_weapon_facing(facing);
            e.set_desired_weapon_facing(facing);
            e.set_emplacement_facing(Some(facing));
            e.set_weapon_setup(WeaponSetup::Deployed);
        }
    }

    let screen_center = (
        battery_center.0 + forward.0 * DEBUG_INERT_RIFLEMAN_SCREEN_FRONT_TILES * ts,
        battery_center.1 + forward.1 * DEBUG_INERT_RIFLEMAN_SCREEN_FRONT_TILES * ts,
    );
    let center_index = (DEBUG_INERT_RIFLEMAN_SCREEN_COUNT.saturating_sub(1)) as f32 * 0.5;
    for i in 0..DEBUG_INERT_RIFLEMAN_SCREEN_COUNT {
        let offset = (i as f32 - center_index) * DEBUG_INERT_RIFLEMAN_SCREEN_SPACING_TILES * ts;
        let x = screen_center.0 + side.0 * offset;
        let y = screen_center.1 + side.1 * offset;
        if let Some(id) = entities.spawn_unit(DEBUG_INERT_ENEMY_ID, EntityKind::Rifleman, x, y) {
            if let Some(e) = entities.get_mut(id) {
                e.set_facing(center_facing);
            }
        }
    }

    players.push(PlayerState {
        id: DEBUG_INERT_ENEMY_ID,
        name: "Inert AT Battery".to_string(),
        color: "#8d2f2f".to_string(),
        start_tile: battery_tile,
        steel: 0,
        oil: 0,
        supply_used: 0,
        supply_cap: 0,
        is_ai: true,
        score: ScoreState::default(),
    });
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
mod tests {
    use super::*;

    fn owned_kind_count(game: &Game, owner: u32, kind: EntityKind) -> usize {
        game.entities
            .iter()
            .filter(|e| e.owner == owner && e.kind == kind)
            .count()
    }

    #[test]
    fn direct_reverse_order_scenario_faces_unit_east_and_orders_goal_behind() {
        for unit in [EntityKind::AtTeam, EntityKind::ScoutCar, EntityKind::Tank] {
            let setup = Game::new_direct_reverse_order_scenario(unit, 1, 0x5150_0003)
                .expect("scenario setup should succeed");
            let unit_id = *setup.units.first().expect("scenario should spawn one unit");
            let entity = setup
                .game
                .entities
                .get(unit_id)
                .expect("scenario unit should exist");
            let goal_delta_x = entity.pos_x - setup.goal.0;
            assert!(
                (goal_delta_x - config::TILE_SIZE as f32 * 15.0).abs() <= 0.001,
                "{unit} should receive a goal 15 tiles behind, delta {goal_delta_x:.2}"
            );
            assert!(
                (entity.pos_y - setup.goal.1).abs() <= 0.001,
                "{unit} goal should be directly behind on the same y axis"
            );
            assert!(
                entity.facing().abs() <= 0.001,
                "{unit} should begin facing east, facing {:.4}",
                entity.facing()
            );
        }
    }

    fn assert_vehicle_small_block_baseline_setup(vehicle: EntityKind, pair_count: usize) {
        let setup =
            Game::new_vehicle_small_block_baseline_scenario(vehicle, pair_count, 0x5150_0004)
                .expect("scenario setup should succeed");
        assert_eq!(
            setup.units.len(),
            pair_count,
            "{vehicle} scenario should command one vehicle per pair"
        );
        assert_eq!(owned_kind_count(&setup.game, 1, vehicle), pair_count);
        assert_eq!(
            owned_kind_count(&setup.game, 1, EntityKind::Worker),
            pair_count,
            "{vehicle} scenario should spawn one small blocker per vehicle"
        );

        let north = -std::f32::consts::FRAC_PI_2;
        let mut vehicle_positions: Vec<(f32, f32)> = setup
            .units
            .iter()
            .map(|id| {
                let entity = setup
                    .game
                    .entities
                    .get(*id)
                    .expect("scenario vehicle should exist");
                assert_eq!(entity.kind, vehicle);
                assert!(
                    (entity.facing() - north).abs() <= 0.001,
                    "{vehicle} should begin facing north, facing {:.4}",
                    entity.facing()
                );
                (entity.pos_x, entity.pos_y)
            })
            .collect();
        vehicle_positions.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut blocker_positions: Vec<(f32, f32)> = setup
            .game
            .entities
            .iter()
            .filter(|e| e.owner == 1 && e.kind == EntityKind::Worker)
            .map(|e| (e.pos_x, e.pos_y))
            .collect();
        blocker_positions.sort_by(|a, b| a.0.total_cmp(&b.0));

        let expected_spacing = vehicle_small_block_baseline_vehicle_spacing(vehicle);
        for pair in vehicle_positions.windows(2) {
            let gap = pair[1].0 - pair[0].0;
            assert!(
                (gap - expected_spacing).abs() <= 0.001,
                "{vehicle} adjacent vehicle spacing should be {expected_spacing:.2}px, got {gap:.2}px"
            );
        }
        for (vehicle_pos, blocker_pos) in vehicle_positions.iter().zip(blocker_positions.iter()) {
            assert!(
                (vehicle_pos.0 - blocker_pos.0).abs() <= 0.001,
                "{vehicle} blocker should be directly north on the same x"
            );
            let north_delta = vehicle_pos.1 - blocker_pos.1;
            assert!(
                (north_delta - config::TILE_SIZE as f32).abs() <= 0.001,
                "{vehicle} blocker should be one tile north, got {north_delta:.2}px"
            );
        }

        let center_vehicle = vehicle_positions[pair_count / 2];
        let goal_delta_y = center_vehicle.1 - setup.goal.1;
        assert!(
            (goal_delta_y - config::TILE_SIZE as f32 * 20.0).abs() <= 0.001,
            "{vehicle} move goal should be 20 tiles north of the formation center, delta {goal_delta_y:.2}"
        );
        assert!(
            (center_vehicle.0 - setup.goal.0).abs() <= 0.001,
            "{vehicle} move goal should stay on the formation center x axis"
        );
    }

    macro_rules! vehicle_small_block_baseline_test {
        ($name:ident, $vehicle:expr, $pair_count:expr) => {
            #[test]
            fn $name() {
                assert_vehicle_small_block_baseline_setup($vehicle, $pair_count);
            }
        };
    }

    vehicle_small_block_baseline_test!(
        vehicle_small_block_baseline_scout_car_one_pair,
        EntityKind::ScoutCar,
        1
    );
    vehicle_small_block_baseline_test!(
        vehicle_small_block_baseline_scout_car_three_pairs,
        EntityKind::ScoutCar,
        3
    );
    vehicle_small_block_baseline_test!(
        vehicle_small_block_baseline_scout_car_five_pairs,
        EntityKind::ScoutCar,
        5
    );
    vehicle_small_block_baseline_test!(
        vehicle_small_block_baseline_tank_one_pair,
        EntityKind::Tank,
        1
    );
    vehicle_small_block_baseline_test!(
        vehicle_small_block_baseline_tank_three_pairs,
        EntityKind::Tank,
        3
    );
    vehicle_small_block_baseline_test!(
        vehicle_small_block_baseline_tank_five_pairs,
        EntityKind::Tank,
        5
    );

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

        assert_eq!(owned_kind_count(&game, 1, EntityKind::Depot), 15);
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

    #[test]
    fn debug_starting_loadout_adds_inert_enemy_at_gun_battery_without_profile() {
        let players = [PlayerInit {
            id: 1,
            name: "Human".to_string(),
            color: "#cc1111".to_string(),
            is_ai: false,
        }];
        let game = Game::new_with_debug_starting_loadout_and_random_ai_profiles(
            &players,
            config::QUICKSTART_STEEL,
            config::QUICKSTART_OIL,
            1,
        );
        game.assert_invariants();

        let battery_player = game
            .players
            .iter()
            .find(|p| p.id == DEBUG_INERT_ENEMY_ID)
            .expect("debug battery should be represented as an AI player");
        assert!(battery_player.is_ai);

        let human_start = game.players.iter().find(|p| p.id == 1).unwrap().start_tile;
        let max_tile = game.map.size - 1;
        assert_eq!(
            battery_player.start_tile,
            (max_tile - human_start.0, max_tile - human_start.1)
        );

        let map_center = game.map.world_size_px() * 0.5;
        let battery_center = game
            .map
            .tile_center(battery_player.start_tile.0, battery_player.start_tile.1);
        let center_facing = (map_center - battery_center.1).atan2(map_center - battery_center.0);
        let forward = (center_facing.cos(), center_facing.sin());
        let side = (-center_facing.sin(), center_facing.cos());
        let ts = config::TILE_SIZE as f32;
        let guns: Vec<_> = game
            .entities
            .iter()
            .filter(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::AtTeam)
            .collect();
        assert_eq!(guns.len(), DEBUG_INERT_AT_GUN_COUNT);
        for gun in guns {
            let facing_to_center = (map_center - gun.pos_y).atan2(map_center - gun.pos_x);
            assert_eq!(gun.weapon_setup(), WeaponSetup::Deployed);
            assert!(
                (gun.emplacement_facing().unwrap_or(f32::NAN) - facing_to_center).abs() <= 0.001,
                "gun emplacement should point toward map center"
            );
            assert!(
                (gun.weapon_facing().unwrap_or(f32::NAN) - facing_to_center).abs() <= 0.001,
                "gun barrel should point toward map center"
            );
            assert!(
                (gun.facing() - facing_to_center).abs() <= 0.001,
                "gun should face map center"
            );
        }

        let mut rifle_offsets: Vec<_> = game
            .entities
            .iter()
            .filter(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::Rifleman)
            .map(|e| {
                let dx = e.pos_x - battery_center.0;
                let dy = e.pos_y - battery_center.1;
                let front_tiles = (dx * forward.0 + dy * forward.1) / ts;
                let side_tiles = (dx * side.0 + dy * side.1) / ts;
                (front_tiles, side_tiles, e.facing())
            })
            .collect();
        assert_eq!(rifle_offsets.len(), DEBUG_INERT_RIFLEMAN_SCREEN_COUNT);
        rifle_offsets.sort_by(|a, b| a.1.total_cmp(&b.1));

        let center_index = (DEBUG_INERT_RIFLEMAN_SCREEN_COUNT.saturating_sub(1)) as f32 * 0.5;
        for (i, (front_tiles, side_tiles, facing)) in rifle_offsets.into_iter().enumerate() {
            let expected_side =
                (i as f32 - center_index) * DEBUG_INERT_RIFLEMAN_SCREEN_SPACING_TILES;
            assert!(
                (front_tiles - DEBUG_INERT_RIFLEMAN_SCREEN_FRONT_TILES).abs() <= 0.001,
                "rifleman screen should stand five tiles in front of the AT guns"
            );
            assert!(
                (side_tiles - expected_side).abs() <= 0.001,
                "rifleman screen should form an evenly spaced line"
            );
            assert!(
                (facing - center_facing).abs() <= 0.001,
                "rifleman screen should face map center"
            );
        }
    }
}
