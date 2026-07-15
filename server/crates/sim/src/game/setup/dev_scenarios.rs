use super::*;
use crate::game::state::TrackedRng;

mod layouts;
mod panzerfaust;
mod tank_coax;

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

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        }
        .checkpoint_backed("dev:scout_car_snaking_corridor")
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

        DevScenarioSetup {
            game,
            player_id,
            units: vec![unit_id],
            goal,
            issue_after_ticks: 0,
        }
        .checkpoint_backed("dev:direct_reverse_order")
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

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        }
        .checkpoint_backed("dev:scout_car_wall_chokepoint")
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

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        }
        .checkpoint_backed("dev:vehicle_corner_wall")
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

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: 0,
        }
        .checkpoint_backed("dev:vehicle_small_block_baseline")
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

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: config::TICK_HZ / 2,
        }
        .checkpoint_backed("dev:factory_zero_gap_perpendicular")
    }

    pub fn new_command_car_corner_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::CommandCar {
            return Err(format!("unsupported building-corner unit {unit}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported building-corner unit count {unit_count}"
            ));
        }

        let (map, start_tile, buildings, unit_start, unit_facing, goal) =
            command_car_building_corner_map();
        let mut entities = EntityStore::new();
        for (kind, x, y) in buildings {
            entities
                .spawn_building(1, kind, x, y, true)
                .ok_or_else(|| format!("failed to spawn {kind}"))?;
        }
        let unit_id = entities
            .spawn_unit(1, unit, unit_start.0, unit_start.1)
            .ok_or_else(|| "failed to spawn command car".to_string())?;
        if let Some(entity) = entities.get_mut(unit_id) {
            entity.set_facing(unit_facing);
        }

        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:command_car_building_corner",
        );

        DevScenarioSetup {
            game,
            player_id,
            units: vec![unit_id],
            goal,
            issue_after_ticks: config::TICK_HZ,
        }
        .checkpoint_backed("dev:command_car_building_corner")
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
            .spawn_building(
                1,
                EntityKind::TrainingCentre,
                training_pos.0,
                training_pos.1,
                true,
            )
            .ok_or_else(|| "failed to spawn Training Centre".to_string())?;
        spawn_tank_trap_line_workers(&mut entities, worker_starts)?;
        let units = spawn_tank_trap_line_test_units(&mut entities, vehicle, unit_starts)?;
        let player_id = 1;
        let mut game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            layout.scenario_id(),
        );
        if let Some(player) = game.state.players.iter_mut().find(|p| p.id == player_id) {
            player.refund_resources(1_000, 0);
            let _ = player.spend_resources(0, 9_000);
        }
        if let Some(loadout) = game
            .state
            .starting_loadouts
            .iter_mut()
            .find(|loadout| loadout.player_id == player_id)
        {
            loadout.starting_steel = 1_000;
            loadout.starting_oil = 1_000;
        }

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: config::TICK_HZ * 30,
        }
        .checkpoint_backed(layout.scenario_id())
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

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: config::TICK_HZ,
        }
        .checkpoint_backed(&format!(
            "dev:tank_trap_pathing_matrix:{}",
            layout.scenario_case()
        ))
    }

    pub fn new_entrenchment_inspection_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Rifleman {
            return Err(format!("unsupported entrenchment inspection unit {unit}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported entrenchment inspection unit count {unit_count}"
            ));
        }

        let mut map = flat_dev_map(2);
        let center = (map.size / 2, map.size / 2);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = (center.0 - 8, center.1 + 8);
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = (center.0 + 12, center.1 - 8);
        }

        let ts = config::TILE_SIZE as f32;
        let preseeded_trench = map.tile_center(center.0, center.1);
        let connected_trench = (preseeded_trench.0 + ts * 1.25, preseeded_trench.1);
        let fog_reference_trench = map.tile_center(center.0 + 16, center.1 - 10);
        let dig_start = map.tile_center(center.0 - 7, center.1 - 4);
        let reuse_start = (preseeded_trench.0 - ts * 0.42, preseeded_trench.1);
        let crowd_start = (
            connected_trench.0 + ts * 0.64,
            connected_trench.1 + ts * 0.1,
        );
        let enemy_reuse_start = (
            preseeded_trench.0 + ts * 0.35,
            preseeded_trench.1 + ts * 0.25,
        );
        let training_pos = services::occupancy::footprint_center(
            &map,
            EntityKind::TrainingCentre,
            center.0 - 12,
            center.1 + 7,
        );

        let mut entities = EntityStore::new();
        entities
            .spawn_building(
                1,
                EntityKind::TrainingCentre,
                training_pos.0,
                training_pos.1,
                true,
            )
            .ok_or_else(|| "failed to spawn entrenchment Training Centre".to_string())?;
        let digger = entities
            .spawn_unit(1, EntityKind::Rifleman, dig_start.0, dig_start.1)
            .ok_or_else(|| "failed to spawn entrenchment digger".to_string())?;
        let reuse_rifleman = entities
            .spawn_unit(1, EntityKind::Rifleman, reuse_start.0, reuse_start.1)
            .ok_or_else(|| "failed to spawn entrenchment reuse rifleman".to_string())?;
        let crowded_machine_gunner = entities
            .spawn_unit(1, EntityKind::MachineGunner, crowd_start.0, crowd_start.1)
            .ok_or_else(|| "failed to spawn entrenchment crowded machine gunner".to_string())?;
        let enemy_reuser = entities
            .spawn_unit(
                2,
                EntityKind::Rifleman,
                enemy_reuse_start.0,
                enemy_reuse_start.1,
            )
            .ok_or_else(|| "failed to spawn enemy trench reuser".to_string())?;

        let player_id = 1;
        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            player_id,
            (center.0 - 8, center.1 + 8),
            seed,
            "dev:entrenchment_inspection",
        );
        if let Some(player) = game.state.players.iter_mut().find(|p| p.id == player_id) {
            player.upgrades.insert(upgrade::UpgradeKind::Entrenchment);
            player.refund_resources(1_000, 1_000);
        }
        if let Some(loadout) = game
            .state
            .starting_loadouts
            .iter_mut()
            .find(|loadout| loadout.player_id == player_id)
        {
            loadout.starting_steel = 1_000;
            loadout.starting_oil = 1_000;
        }
        for (x, y) in [preseeded_trench, connected_trench, fog_reference_trench] {
            game.state
                .trenches
                .create(&game.state.map, x, y)
                .ok_or_else(|| "failed to seed entrenchment trench".to_string())?;
        }
        let player_ids = game.state.player_ids();
        game.refresh_trench_memory(&player_ids);

        DevScenarioSetup {
            game,
            player_id,
            units: vec![digger, reuse_rifleman, crowded_machine_gunner, enemy_reuser],
            goal: dig_start,
            issue_after_ticks: u32::MAX,
        }
        .checkpoint_backed("dev:entrenchment_inspection")
    }
}

pub struct DevScenarioSetup {
    pub game: Game,
    pub player_id: u32,
    pub units: Vec<u32>,
    pub goal: (f32, f32),
    pub issue_after_ticks: u32,
}

impl DevScenarioSetup {
    fn checkpoint_backed(mut self, label: &str) -> Result<Self, String> {
        self.game = Game::checkpoint_backed_start_from_direct_for_setup(self.game, label)
            .map_err(|err| format!("failed to build checkpoint-backed {label} start: {err}"))?;
        Ok(self)
    }
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
    let mut game = Game::new_without_ai_controllers(&players, seed);
    game.state.map = map;
    game.state.entities = entities;
    game.state.fog = Fog::new(game.state.map.size);
    game.state.pending.clear();
    game.state.command_log.clear();
    game.state.tick = 0;
    game.reset_derived_state();
    game.state.lingering_sight.clear();
    game.state.smokes = SmokeCloudStore::new();
    game.state.starting_loadouts = players
        .iter()
        .map(|player| PlayerStartingLoadout {
            player_id: player.id,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            loadout_id: "dev_scenario".to_string(),
            starting_steel: 0,
            starting_oil: 0,
        })
        .collect();
    game.state.map_metadata = super::dev_map_metadata(metadata_name);
    game.state.active_construction_sites.clear();
    game.state.starting_loadout = StartingLoadout::Standard;
    game.state.rng = TrackedRng::seed_from_match_seed(seed);
    if let Some(player) = game
        .state
        .players
        .iter_mut()
        .find(|player| player.id == player_id)
    {
        player.reset_for_dev_scenario(start_tile);
    }
    let ids = game.state.player_ids();
    game.state.fog = Fog::new(game.state.map.size);
    game.state.fog.recompute_with_smoke(
        &ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );
    game.refresh_trench_memory(&ids);
    game
}

/// Spawn the steel and oil clusters for a base site. The clusters point inward toward the map
/// center so the layout is the same regardless of whether a player occupies the site.
#[cfg(test)]
mod tests;
