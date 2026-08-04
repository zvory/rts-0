use super::*;
use crate::rules::combat::{self, WeaponKind};

impl Game {
    pub fn new_move_reload_acquisition_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Tank || unit_count != 1 {
            return Err(format!(
                "unsupported move reload acquisition launch {unit} x{unit_count}"
            ));
        }

        let mut map = flat_dev_map(2);
        let center = (map.width / 2, map.height / 2);
        let start_tile = (center.0 - 8, center.1);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = (center.0 + 8, center.1);
        }

        let tile_size = config::TILE_SIZE as f32;
        let attacker_pos = map.tile_center(start_tile.0, start_tile.1);
        let goal = (attacker_pos.0 + tile_size * 12.0, attacker_pos.1);
        // The gun begins outside the moving Tank's five-tile cannon range, then falls well inside
        // it after roughly one second of eastward travel. Its lateral offset makes the missing
        // turret response unmistakable while the Tank continues its plain Move order.
        let target_pos = (
            attacker_pos.0 + tile_size * 5.75,
            attacker_pos.1 - tile_size * 3.0,
        );

        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::Tank, attacker_pos.0, attacker_pos.1)
            .ok_or_else(|| "failed to spawn move-reload Tank".to_string())?;
        let target = entities
            .spawn_unit(2, EntityKind::AntiTankGun, target_pos.0, target_pos.1)
            .ok_or_else(|| "failed to spawn move-reload Anti-Tank Gun".to_string())?;
        let cannon = combat::weapon_profile(WeaponKind::TankCannon)
            .ok_or_else(|| "missing Tank cannon profile".to_string())?;
        let issue_after_ticks = config::TICK_HZ * 10;
        if let Some(entity) = entities.get_mut(attacker) {
            entity.set_facing(0.0);
            entity.set_weapon_facing(0.0);
            entity.set_weapon_cooldown(
                WeaponKind::TankCannon,
                issue_after_ticks.saturating_add(cannon.cooldown),
            );
            entity.set_weapon_cooldown(
                WeaponKind::TankCoax,
                issue_after_ticks.saturating_add(cannon.cooldown),
            );
        }
        if let Some(entity) = entities.get_mut(target) {
            entity.hold_position();
            entity.set_facing(std::f32::consts::FRAC_PI_2);
            entity.set_weapon_facing(std::f32::consts::FRAC_PI_2);
            for weapon in WeaponKind::ALL {
                entity.set_weapon_cooldown(weapon, config::TICK_HZ * 120);
            }
        }

        let player_id = 1;
        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            player_id,
            start_tile,
            seed,
            "dev:move_reload_acquisition",
        );
        game.state.lab_god_mode_players.insert(2);
        game.sync_lab_god_mode_flags();

        DevScenarioSetup {
            game,
            player_id,
            units: vec![attacker],
            goal,
            issue_after_ticks,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:move_reload_acquisition")
    }
}
