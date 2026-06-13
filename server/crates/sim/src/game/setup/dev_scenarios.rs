use super::*;

mod layouts;

use layouts::*;

impl Game {
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
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:scout_car_snaking_corridor",
        );

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        })
    }

    pub fn new_direct_reverse_order_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(
            unit,
            EntityKind::AntiTankGun | EntityKind::ScoutCar | EntityKind::Tank
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
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:direct_reverse_order",
        );

        Ok(DevScenarioSetup {
            game,
            player_id,
            units: vec![unit_id],
            goal,
            issue_after_ticks: 0,
        })
    }

    pub fn new_scout_car_wall_chokepoint_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(
            unit,
            EntityKind::AntiTankGun | EntityKind::ScoutCar | EntityKind::Tank
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
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:scout_car_wall_chokepoint",
        );

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        })
    }

    pub fn new_vehicle_corner_wall_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(
            unit,
            EntityKind::AntiTankGun | EntityKind::ScoutCar | EntityKind::Tank
        ) {
            return Err(format!("unsupported vehicle-corner-wall unit {unit}"));
        }
        if !matches!(unit_count, 1 | 3 | 5) {
            return Err(format!(
                "unsupported vehicle-corner-wall unit count {unit_count}"
            ));
        }

        let (map, start_tile, starts, goal) = vehicle_corner_wall_map(unit, unit_count);
        let mut entities = EntityStore::new();
        let units = spawn_wall_chokepoint_units(&mut entities, unit, starts)?;
        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:vehicle_corner_wall",
        );

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        })
    }

    pub fn new_vehicle_small_block_baseline_scenario(
        vehicle: EntityKind,
        pair_count: usize,
        blocker: Option<EntityKind>,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(vehicle, EntityKind::ScoutCar | EntityKind::Tank) {
            return Err(format!(
                "unsupported vehicle-small-block-baseline vehicle {vehicle}"
            ));
        }
        if let Some(blocker) = blocker {
            if !matches!(
                blocker,
                EntityKind::Worker
                    | EntityKind::Rifleman
                    | EntityKind::MachineGunner
                    | EntityKind::AntiTankGun
            ) {
                return Err(format!(
                    "unsupported vehicle-small-block-baseline blocker {blocker}"
                ));
            }
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
        spawn_vehicle_small_block_baseline_blockers(&mut entities, blocker, blocker_starts)?;
        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:vehicle_small_block_baseline",
        );

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        })
    }

    pub fn new_factory_zero_gap_perpendicular_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(
            unit,
            EntityKind::AntiTankGun | EntityKind::ScoutCar | EntityKind::Tank
        ) {
            return Err(format!("unsupported factory-zero-gap unit {unit}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported factory-zero-gap unit count {unit_count}"
            ));
        }

        let (map, start_tile, factory_pos, unit_start, goal) =
            factory_zero_gap_perpendicular_map(unit);
        let mut entities = EntityStore::new();
        entities
            .spawn_building(1, EntityKind::Factory, factory_pos.0, factory_pos.1, true)
            .ok_or_else(|| "failed to spawn factory".to_string())?;
        let units = spawn_factory_zero_gap_perpendicular_units(&mut entities, unit, unit_start)?;
        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:factory_zero_gap_perpendicular",
        );

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: config::TICK_HZ / 2,
        })
    }
}

pub struct DevScenarioSetup {
    pub game: Game,
    pub player_id: u32,
    pub units: Vec<u32>,
    pub goal: (f32, f32),
    pub issue_after_ticks: u32,
}

fn build_dev_scenario_game(
    map: Map,
    entities: EntityStore,
    player_id: u32,
    start_tile: (u32, u32),
    seed: u32,
    metadata_name: &str,
) -> Game {
    let players = [PlayerInit {
        id: player_id,
        team_id: player_id,
        faction_id: "steel_vanguard".to_string(),
        name: "Scenario".to_string(),
        color: "#4878c8".to_string(),
        is_ai: false,
    }];
    let spatial = services::spatial::SpatialIndex::build(&entities, map.size);
    let pathing = services::pathing::PathingService::new(65_536, 256);
    let rng = SmallRng::seed_from_u64(seed as u64);
    let mut game = Game::new_without_ai_controllers(&players, seed);
    game.map = map;
    game.entities = entities;
    game.fog = Fog::new(game.map.size);
    game.pending.clear();
    game.command_log.clear();
    game.tick = 0;
    game.spatial = spatial;
    game.pathing = pathing;
    game.lingering_sight.clear();
    game.smokes = SmokeCloudStore::new();
    game.starting_steel = 0;
    game.starting_oil = 0;
    game.map_metadata = super::dev_map_metadata(metadata_name);
    game.debug_path_overlays = true;
    game.starting_loadout = StartingLoadout::DebugHuman;
    game.rng = rng;
    if let Some(player) = game.players.first_mut() {
        player.reset_for_dev_scenario(start_tile);
    }
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog = Fog::new(game.map.size);
    game.fog
        .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);
    game
}

/// Spawn the steel and oil clusters for a base site. The clusters point inward toward the map
/// center so the layout is the same regardless of whether a player occupies the site.

#[cfg(test)]
mod tests;
