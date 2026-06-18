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

    pub fn new_tank_trap_line_build_scenario(
        scenario_id: &str,
        vehicle: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        let layout = TankTrapLineLayout::from_scenario_id(scenario_id)
            .ok_or_else(|| format!("unsupported Tank Trap line scenario {scenario_id}"))?;
        if !matches!(
            vehicle,
            EntityKind::AntiTankGun
                | EntityKind::MortarTeam
                | EntityKind::Artillery
                | EntityKind::ScoutCar
                | EntityKind::Tank
                | EntityKind::CommandCar
        ) {
            return Err(format!("unsupported Tank Trap line vehicle {vehicle}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported Tank Trap line unit count {unit_count}"
            ));
        }

        let (map, start_tile, training_pos, worker_starts, unit_starts, goal) =
            tank_trap_line_build_map(layout, vehicle);
        let mut entities = EntityStore::new();
        entities
            .spawn_building(1, EntityKind::TrainingCentre, training_pos.0, training_pos.1, true)
            .ok_or_else(|| "failed to spawn Training Centre".to_string())?;
        spawn_tank_trap_line_workers(&mut entities, worker_starts)?;
        let units = spawn_tank_trap_line_test_units(&mut entities, vehicle, unit_starts)?;
        let player_id = 1;
        let mut game =
            build_dev_scenario_game(map, entities, player_id, start_tile, seed, layout.scenario_id());
        if let Some(player) = game.players.iter_mut().find(|p| p.id == player_id) {
            player.refund_resources(1_000, 0);
            let _ = player.spend_resources(0, 9_000);
        }
        if let Some(loadout) = game
            .starting_loadouts
            .iter_mut()
            .find(|loadout| loadout.player_id == player_id)
        {
            loadout.starting_steel = 1_000;
            loadout.starting_oil = 1_000;
        }

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: config::TICK_HZ * 30,
        })
    }

    pub fn new_tank_trap_pathing_scenario(
        scenario_case: &str,
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        let layout = TankTrapPathingLayout::from_case(scenario_case)
            .ok_or_else(|| format!("unsupported Tank Trap pathing case {scenario_case}"))?;
        let supported = match layout {
            TankTrapPathingLayout::FriendlyVehicleReroute => {
                matches!(unit, EntityKind::ScoutCar | EntityKind::Tank)
            }
            TankTrapPathingLayout::EnemyVehicleBreach => matches!(
                unit,
                EntityKind::AntiTankGun
                    | EntityKind::MortarTeam
                    | EntityKind::Artillery
                    | EntityKind::ScoutCar
                    | EntityKind::Tank
            ),
            TankTrapPathingLayout::InfantryPassThrough => {
                matches!(
                    unit,
                    EntityKind::Worker | EntityKind::Rifleman | EntityKind::MachineGunner
                )
            }
            TankTrapPathingLayout::ExplicitInfantryAttack => unit == EntityKind::Rifleman,
        };
        if !supported {
            return Err(format!(
                "unsupported Tank Trap pathing unit {unit} for {scenario_case}"
            ));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported Tank Trap pathing unit count {unit_count}"
            ));
        }

        let (map, start_tile, unit_start, traps, enemy_base, goal) =
            tank_trap_pathing_map(layout, unit);
        let mut entities = EntityStore::new();
        spawn_tank_trap_pathing_wall(&mut entities, traps)?;
        if let Some((x, y)) = enemy_base {
            entities
                .spawn_building(2, EntityKind::CityCentre, x, y, true)
                .ok_or_else(|| "failed to spawn remote enemy City Centre".to_string())?;
        }
        let units = spawn_tank_trap_pathing_unit(&mut entities, unit, unit_start)?;
        let player_id = 1;
        let game = build_dev_scenario_game_with_teams(
            map,
            entities,
            layout.player_teams(),
            player_id,
            start_tile,
            seed,
            &format!("dev:tank_trap_pathing_matrix:{}", layout.scenario_case()),
        );

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: config::TICK_HZ,
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
    build_dev_scenario_game_with_teams(
        map,
        entities,
        [(player_id, player_id)],
        player_id,
        start_tile,
        seed,
        metadata_name,
    )
}

fn build_dev_scenario_game_with_teams<const N: usize>(
    map: Map,
    entities: EntityStore,
    teams: [(u32, u32); N],
    player_id: u32,
    start_tile: (u32, u32),
    seed: u32,
    metadata_name: &str,
) -> Game {
    let colors = ["#4878c8", "#c84848", "#48a868", "#c8a848"];
    let players: Vec<PlayerInit> = teams
        .into_iter()
        .enumerate()
        .map(|(index, (id, team_id))| PlayerInit {
            id,
            team_id,
            faction_id: "kriegsia".to_string(),
            name: format!("Scenario {id}"),
            color: colors[index % colors.len()].to_string(),
            is_ai: false,
        })
        .collect();
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
    game.starting_loadouts = players
        .iter()
        .map(|player| PlayerStartingLoadout {
            player_id: player.id,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            loadout_id: "dev_scenario".to_string(),
            starting_steel: 0,
            starting_oil: 0,
        })
        .collect();
    game.map_metadata = super::dev_map_metadata(metadata_name);
    game.active_construction_sites.clear();
    game.starting_loadout = StartingLoadout::DebugHuman;
    game.rng = rng;
    if let Some(player) = game.players.iter_mut().find(|player| player.id == player_id) {
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
