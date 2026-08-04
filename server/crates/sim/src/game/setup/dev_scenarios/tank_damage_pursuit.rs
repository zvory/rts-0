use super::*;
use crate::rules::combat::WeaponKind;

const DISABLED_WEAPON_TICKS: u32 = config::TICK_HZ * 120;

impl Game {
    pub fn new_tank_damage_pursuit_pivot_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Tank || unit_count != 1 {
            return Err(format!(
                "unsupported Tank damage-pursuit pivot launch {unit} x{unit_count}; expected tank x1"
            ));
        }

        let mut map = flat_dev_map(2);
        let center_tile = (map.width / 2, map.height / 2);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = center_tile;
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = (center_tile.0 - 4, center_tile.1);
        }

        let tile_size = config::TILE_SIZE as f32;
        let tank_pos = map.tile_center(center_tile.0, center_tile.1);
        let panzerfaust_pos = (tank_pos.0 - tile_size, tank_pos.1);
        let target_pos = (tank_pos.0 + tile_size * 12.0, tank_pos.1);
        let spotter_pos = (tank_pos.0 + tile_size * 9.0, tank_pos.1 + tile_size * 2.0);

        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
            .ok_or_else(|| "failed to spawn damage-pursuit Tank".to_string())?;
        let panzerfaust = entities
            .spawn_unit(
                2,
                EntityKind::Rifleman,
                panzerfaust_pos.0,
                panzerfaust_pos.1,
            )
            .ok_or_else(|| "failed to spawn rear Panzerfaust".to_string())?;
        let target = entities
            .spawn_unit(2, EntityKind::AntiTankGun, target_pos.0, target_pos.1)
            .ok_or_else(|| "failed to spawn pursuit target Anti-Tank Gun".to_string())?;
        let spotter = entities
            .spawn_unit(1, EntityKind::ScoutCar, spotter_pos.0, spotter_pos.1)
            .ok_or_else(|| "failed to spawn pursuit target spotter".to_string())?;

        if let Some(entity) = entities.get_mut(tank) {
            entity.set_spawn_health(2_000);
            entity.set_facing(0.0);
            entity.set_weapon_facing(0.0);
            for weapon in WeaponKind::ALL {
                entity.set_weapon_cooldown(weapon, DISABLED_WEAPON_TICKS);
            }
        }
        if let Some(entity) = entities.get_mut(panzerfaust) {
            entity.set_facing(0.0);
            entity.set_weapon_facing(0.0);
            entity.hold_position();
            for weapon in WeaponKind::ALL {
                entity.set_weapon_cooldown(weapon, DISABLED_WEAPON_TICKS);
            }
            if let Some(combat) = entity.combat.as_mut() {
                combat.panzerfaust = None;
            }
        }
        if let Some(entity) = entities.get_mut(target) {
            // Packed and held: it is a visible direct-attack destination, not another source of AP fire.
            entity.hold_position();
            entity.set_facing(std::f32::consts::PI);
            entity.set_weapon_facing(std::f32::consts::PI);
            for weapon in WeaponKind::ALL {
                entity.set_weapon_cooldown(weapon, DISABLED_WEAPON_TICKS);
            }
        }
        if let Some(entity) = entities.get_mut(spotter) {
            entity.hold_position();
            for weapon in WeaponKind::ALL {
                entity.set_weapon_cooldown(weapon, DISABLED_WEAPON_TICKS);
            }
        }

        let player_id = 1;
        let game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            player_id,
            center_tile,
            seed,
            "dev:tank_damage_pursuit_pivot",
        );
        let mut setup = DevScenarioSetup {
            game,
            player_id,
            units: vec![tank],
            goal: target_pos,
            issue_after_ticks: config::TICK_HZ * 5,
            order: DevScenarioOrder::MoveWithPanzerfaustWindup {
                attacker: panzerfaust,
                victim: tank,
                windup_ticks: (config::TICK_HZ * 2) as u16,
            },
        }
        .checkpoint_backed("dev:tank_damage_pursuit_pivot")?;
        for id in [target, spotter] {
            if let Some(entity) = setup.game.state.entities.get_mut(id) {
                for weapon in WeaponKind::ALL {
                    entity.set_weapon_cooldown(weapon, DISABLED_WEAPON_TICKS);
                }
            }
        }
        if let Some(entity) = setup.game.state.entities.get_mut(panzerfaust) {
            if let Some(combat) = entity.combat.as_mut() {
                combat.panzerfaust = None;
            }
        }
        Ok(setup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_panzerfaust_windup_rejects_a_stale_target_without_mutating_the_attacker() {
        let mut setup =
            Game::new_tank_damage_pursuit_pivot_scenario(EntityKind::Tank, 1, 0x5150_0724)
                .expect("damage-pursuit scenario should build");
        let (attacker, _, windup_ticks) = setup
            .panzerfaust_windup()
            .expect("scenario should author one delayed Panzerfaust windup");

        assert!(!setup.game.start_dev_scenario_panzerfaust_windup(
            attacker,
            u32::MAX,
            windup_ticks,
        ));
        assert!(setup
            .game
            .state
            .entities
            .get(attacker)
            .and_then(|entity| entity.combat.as_ref())
            .is_some_and(|combat| combat.panzerfaust.is_none()));
    }

    #[test]
    fn scenario_keeps_damage_preference_from_forcing_a_large_pursuit_pivot() {
        let mut setup = Game::new_dev_scenario(
            "tank_damage_pursuit_pivot",
            EntityKind::Tank,
            1,
            None,
            None,
            0x5150_0724,
        )
        .expect("damage-pursuit scenario should build through the dispatcher");
        let tank_id = setup.units[0];
        let start_x = setup
            .game
            .state
            .entities
            .get(tank_id)
            .expect("scenario Tank should exist")
            .pos_x;
        assert!(matches!(setup.command(), SimCommand::Move { .. }));

        for _ in 0..setup.issue_after_ticks {
            setup.game.tick();
        }
        let (attacker, victim, windup_ticks) = setup
            .panzerfaust_windup()
            .expect("scenario should author one delayed Panzerfaust windup");
        assert!(setup
            .game
            .start_dev_scenario_panzerfaust_windup(attacker, victim, windup_ticks));
        setup.game.enqueue(setup.player_id, setup.command());
        let mut hit_tick = None;
        let mut max_turn_after_hit = 0.0_f32;
        let mut max_x = start_x;
        for elapsed in 0..config::TICK_HZ * 4 {
            setup.game.tick();
            let tank = setup
                .game
                .state
                .entities
                .get(tank_id)
                .unwrap_or_else(|| panic!("scenario Tank should survive tick {elapsed}"));
            if hit_tick.is_none() && tank.last_damage_tick().is_some() {
                hit_tick = tank.last_damage_tick();
            }
            if hit_tick.is_some() {
                max_turn_after_hit = max_turn_after_hit
                    .max(crate::game::services::movement::angle_delta(0.0, tank.facing()).abs());
            }
            max_x = max_x.max(tank.pos_x);
        }

        assert!(setup.game.state.entities.get(tank_id).is_some());
        assert!(
            hit_tick.is_some(),
            "the Panzerfaust should hit the moving Tank"
        );
        assert!(
            max_x > start_x + 1.0,
            "the Tank should begin pursuing the out-of-range Anti-Tank Gun"
        );
        assert!(
            max_turn_after_hit <= 80.0_f32.to_radians() + 0.001,
            "damage-facing preference must not pivot the pursuing Tank more than 80 degrees"
        );
    }
}
