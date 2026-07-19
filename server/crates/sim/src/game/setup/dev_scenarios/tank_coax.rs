use super::*;
use crate::rules::combat::WeaponKind;

impl Game {
    pub fn new_tank_coax_inspection_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Tank || unit_count != 1 {
            return Err(format!(
                "unsupported Tank coax inspection launch {unit} x{unit_count}"
            ));
        }

        let mut map = flat_dev_map(2);
        let center = (map.size / 2, map.size / 2);
        let start_tile = (center.0 - 8, center.1);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = (center.0 + 8, center.1);
        }

        let ts = config::TILE_SIZE as f32;
        let tank_pos = map.tile_center(start_tile.0, start_tile.1);
        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
            .ok_or_else(|| "failed to spawn Tank coax inspection Tank".to_string())?;
        if let Some(tank) = entities.get_mut(tank) {
            tank.hold_position();
            tank.set_facing(0.0);
            tank.set_weapon_facing(0.0);
            tank.set_weapon_cooldown(WeaponKind::TankCannon, config::TICK_HZ * 4);
        }

        let mut units = vec![tank];
        for (kind, dx, dy) in TARGET_SPECS {
            let target = entities
                .spawn_unit(2, kind, tank_pos.0 + ts * dx, tank_pos.1 + ts * dy)
                .ok_or_else(|| format!("failed to spawn Tank coax inspection {kind}"))?;
            make_static_inspection_target(&mut entities, target);
            units.push(target);
        }
        spawn_static_targets(&mut entities, tank_pos, ts)?;

        let player_id = 1;
        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            player_id,
            start_tile,
            seed,
            "dev:tank_coax_inspection",
        );
        game.state
            .smokes
            .spawn(
                tank_pos.0 + ts * 5.3,
                tank_pos.1 + ts * 2.55,
                config::SMOKE_CLOUD_RADIUS_TILES,
                config::TICK_HZ * 120,
                0,
            )
            .ok_or_else(|| "failed to spawn Tank coax inspection smoke cloud".to_string())?;
        refresh_projection_after_smoke(&mut game);

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal: tank_pos,
            issue_after_ticks: u32::MAX,
            attack_move: false,
        }
        .checkpoint_backed("dev:tank_coax_inspection")
    }
}

const TARGET_SPECS: [(EntityKind, f32, f32); 10] = [
    (EntityKind::Tank, 3.4, 0.00),
    (EntityKind::Rifleman, 4.16, -0.58),
    (EntityKind::Worker, 4.86, 0.60),
    (EntityKind::MachineGunner, 5.50, -0.19),
    (EntityKind::MortarTeam, 6.14, 0.86),
    (EntityKind::ScoutCar, 4.57, 1.48),
    (EntityKind::Golem, 4.95, -1.61),
    (EntityKind::Ekat, 5.55, 1.70),
    (EntityKind::AntiTankGun, 5.86, -1.68),
    (EntityKind::Artillery, 6.59, -0.35),
];

const INSPECTION_TARGET_WEAPON_DELAY_TICKS: u32 = config::TICK_HZ * 120;

fn make_static_inspection_target(entities: &mut EntityStore, id: u32) {
    let Some(target) = entities.get_mut(id) else {
        return;
    };
    target.hold_position();
    target.set_facing(std::f32::consts::PI);
    target.set_weapon_facing(std::f32::consts::PI);
    for weapon in WeaponKind::ALL {
        target.set_weapon_cooldown(weapon, INSPECTION_TARGET_WEAPON_DELAY_TICKS);
    }
}

fn spawn_static_targets(
    entities: &mut EntityStore,
    tank_pos: (f32, f32),
    ts: f32,
) -> Result<(), String> {
    let depot_pos = (tank_pos.0 + ts * 4.70, tank_pos.1 - ts * 3.60);
    entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .ok_or_else(|| "failed to spawn Tank coax inspection Depot".to_string())?;
    let trap_pos = (tank_pos.0 + ts * 3.60, tank_pos.1 - ts * 1.65);
    entities
        .spawn_building(2, EntityKind::TankTrap, trap_pos.0, trap_pos.1, true)
        .ok_or_else(|| "failed to spawn Tank coax inspection Tank Trap".to_string())?;
    entities
        .spawn_node(
            EntityKind::Steel,
            tank_pos.0 + ts * 4.00,
            tank_pos.1 + ts * 2.50,
        )
        .ok_or_else(|| "failed to spawn Tank coax inspection Steel node".to_string())?;
    entities
        .spawn_node(
            EntityKind::Oil,
            tank_pos.0 + ts * 6.10,
            tank_pos.1 + ts * 2.70,
        )
        .ok_or_else(|| "failed to spawn Tank coax inspection Oil node".to_string())?;
    Ok(())
}

fn refresh_projection_after_smoke(game: &mut Game) {
    let player_ids = game.state.player_ids();
    game.state.fog = Fog::new(game.state.map.size);
    game.state.fog.recompute_with_smoke(
        &player_ids,
        &game.state.entities,
        &game.state.map,
        &game.state.smokes,
    );
    game.refresh_building_memory(&player_ids);
    game.refresh_trench_memory(&player_ids);
}
