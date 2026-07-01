use super::*;
use crate::game::entity::Order;
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
            tank.set_order(Order::HoldPosition);
            tank.set_facing(0.0);
            tank.set_weapon_facing(0.0);
            tank.set_weapon_cooldown(WeaponKind::TankCannon, config::TICK_HZ * 4);
        }

        let mut units = vec![tank];
        for (kind, dx, dy) in TARGET_SPECS {
            units.push(
                entities
                    .spawn_unit(2, kind, tank_pos.0 + ts * dx, tank_pos.1 + ts * dy)
                    .ok_or_else(|| format!("failed to spawn Tank coax inspection {kind}"))?,
            );
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
        game.smokes
            .spawn(
                tank_pos.0 + ts * 5.3,
                tank_pos.1 + ts * 2.55,
                config::SMOKE_CLOUD_RADIUS_TILES,
                config::TICK_HZ * 120,
                0,
            )
            .ok_or_else(|| "failed to spawn Tank coax inspection smoke cloud".to_string())?;
        refresh_projection_after_smoke(&mut game);

        Ok(DevScenarioSetup {
            game,
            player_id,
            units,
            goal: tank_pos,
            issue_after_ticks: u32::MAX,
        })
    }
}

const TARGET_SPECS: [(EntityKind, f32, f32); 10] = [
    (EntityKind::Tank, 4.4, -0.05),
    (EntityKind::Worker, 5.8, -0.05),
    (EntityKind::Rifleman, 5.0, -0.72),
    (EntityKind::MachineGunner, 5.3, 0.80),
    (EntityKind::ScoutCar, 5.4, 0.30),
    (EntityKind::Golem, 5.7, -0.42),
    (EntityKind::Ekat, 5.9, 0.58),
    (EntityKind::MortarTeam, 6.0, -0.82),
    (EntityKind::AntiTankGun, 4.9, 0.62),
    (EntityKind::Artillery, 6.2, -0.20),
];

fn spawn_static_targets(
    entities: &mut EntityStore,
    tank_pos: (f32, f32),
    ts: f32,
) -> Result<(), String> {
    let depot_pos = (tank_pos.0 + ts * 5.9, tank_pos.1 - ts * 0.8);
    entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .ok_or_else(|| "failed to spawn Tank coax inspection Depot".to_string())?;
    let trap_pos = (tank_pos.0 + ts * 4.8, tank_pos.1 - ts * 0.98);
    entities
        .spawn_building(2, EntityKind::TankTrap, trap_pos.0, trap_pos.1, true)
        .ok_or_else(|| "failed to spawn Tank coax inspection Tank Trap".to_string())?;
    entities
        .spawn_node(
            EntityKind::Steel,
            tank_pos.0 + ts * 5.2,
            tank_pos.1 + ts * 1.10,
        )
        .ok_or_else(|| "failed to spawn Tank coax inspection Steel node".to_string())?;
    entities
        .spawn_node(
            EntityKind::Oil,
            tank_pos.0 + ts * 6.3,
            tank_pos.1 - ts * 1.10,
        )
        .ok_or_else(|| "failed to spawn Tank coax inspection Oil node".to_string())?;
    Ok(())
}

fn refresh_projection_after_smoke(game: &mut Game) {
    let player_ids: Vec<u32> = game.players.iter().map(|player| player.id).collect();
    game.fog = Fog::new(game.map.size);
    game.fog
        .recompute_with_smoke(&player_ids, &game.entities, &game.map, &game.smokes);
    game.refresh_building_memory(&player_ids);
    game.refresh_trench_memory(&player_ids);
}
