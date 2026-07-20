use super::*;
use crate::game::entity::WeaponSetup;
use crate::rules::combat::WeaponKind;

const ISSUE_AFTER_TICKS: u32 = config::TICK_HZ * 10;
const TANK_WEAPON_DELAY_TICKS: u32 = config::TICK_HZ * 120;
const INSPECTION_TANK_HP: u32 = 2_000;

impl Game {
    pub fn new_tank_under_fire_retreat_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        validate_tank_launch("Tank under-fire retreat", unit, unit_count, 1)?;

        let mut map = flat_dev_map(2);
        let center_tile = (map.size / 2, map.size / 2);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = center_tile;
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = (center_tile.0 + 8, center_tile.1);
        }

        let tile_size = config::TILE_SIZE as f32;
        let tank_pos = map.tile_center(center_tile.0, center_tile.1);
        let tank_facing = 0.0;
        let gun_pos = (tank_pos.0 + tile_size * 7.0, tank_pos.1);
        let goal = (tank_pos.0 - tile_size * 15.0, tank_pos.1);

        let mut entities = EntityStore::new();
        let tank = spawn_inspection_tank(&mut entities, tank_pos, tank_facing)?;
        spawn_front_at_gun(&mut entities, gun_pos, std::f32::consts::PI)?;

        let game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            1,
            center_tile,
            seed,
            "dev:tank_under_fire_retreat",
        );

        DevScenarioSetup {
            game,
            player_id: 1,
            units: vec![tank],
            goal,
            issue_after_ticks: ISSUE_AFTER_TICKS,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:tank_under_fire_retreat")
    }

    pub fn new_tank_reverse_traffic_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        validate_tank_launch("Tank reverse-traffic", unit, unit_count, 3)?;

        let mut map = flat_dev_map(2);
        let center_tile = (map.size / 2, map.size / 2);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = center_tile;
        }
        if let Some(slot) = map.starts.get_mut(1) {
            *slot = (center_tile.0 + 12, center_tile.1);
        }

        let center = map.tile_center(center_tile.0, center_tile.1);
        let tile_size = config::TILE_SIZE as f32;
        let tank_x = center.0 + tile_size * 4.0;
        let vertical_gap = tile_size * 1.5;
        let fan_angle = 10.0_f32.to_radians();
        let gun_offset = tile_size * 7.0;
        let retreat_distance = tile_size * 12.0;
        let convergence = (tank_x - vertical_gap / fan_angle.tan(), center.1);
        let mut entities = EntityStore::new();
        spawn_enemy_scout_spotter(
            &mut entities,
            (center.0 + tile_size * 2.0, center.1 - tile_size * 6.0),
        )?;
        let mut tanks = Vec::with_capacity(3);
        let mut goals = Vec::with_capacity(3);

        for (y_offset, facing) in [
            (-vertical_gap, -fan_angle),
            (0.0, 0.0),
            (vertical_gap, fan_angle),
        ] {
            let forward = (facing.cos(), facing.sin());
            let tank_pos = (tank_x, center.1 + y_offset);
            let gun_pos = (
                tank_pos.0 + forward.0 * gun_offset,
                tank_pos.1 + forward.1 * gun_offset,
            );
            let goal = (
                tank_pos.0 - forward.0 * retreat_distance,
                tank_pos.1 - forward.1 * retreat_distance,
            );
            let tank = spawn_inspection_tank(&mut entities, tank_pos, facing)?;
            spawn_front_at_gun(
                &mut entities,
                gun_pos,
                normalize_dev_angle(facing + std::f32::consts::PI),
            )?;
            tanks.push(tank);
            goals.push((tank, goal));
        }

        let game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            1,
            center_tile,
            seed,
            "dev:tank_reverse_traffic",
        );

        DevScenarioSetup {
            game,
            player_id: 1,
            units: tanks,
            goal: convergence,
            issue_after_ticks: ISSUE_AFTER_TICKS,
            order: DevScenarioOrder::IndividualMoves(goals),
        }
        .checkpoint_backed("dev:tank_reverse_traffic")
    }
}

fn validate_tank_launch(
    label: &str,
    unit: EntityKind,
    unit_count: usize,
    expected_count: usize,
) -> Result<(), String> {
    if unit != EntityKind::Tank || unit_count != expected_count {
        return Err(format!(
            "unsupported {label} launch {unit} x{unit_count}; expected tank x{expected_count}"
        ));
    }
    Ok(())
}

fn spawn_inspection_tank(
    entities: &mut EntityStore,
    pos: (f32, f32),
    facing: f32,
) -> Result<u32, String> {
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, pos.0, pos.1)
        .ok_or_else(|| "failed to spawn retreat inspection Tank".to_string())?;
    if let Some(entity) = entities.get_mut(tank) {
        // These scenarios run under sustained live AT fire. Extra scenario-only health keeps the
        // subject alive long enough to inspect the retreat and traffic behavior without changing
        // damage, armor-facing, targeting, or reaction-lock rules.
        entity.set_spawn_health(INSPECTION_TANK_HP);
        entity.set_facing(facing);
        entity.set_weapon_facing(facing);
        for weapon in WeaponKind::ALL {
            entity.set_weapon_cooldown(weapon, TANK_WEAPON_DELAY_TICKS);
        }
    }
    Ok(tank)
}

fn spawn_front_at_gun(
    entities: &mut EntityStore,
    pos: (f32, f32),
    facing: f32,
) -> Result<u32, String> {
    let gun = entities
        .spawn_unit(2, EntityKind::AntiTankGun, pos.0, pos.1)
        .ok_or_else(|| "failed to spawn retreat inspection Anti-Tank Gun".to_string())?;
    if let Some(entity) = entities.get_mut(gun) {
        entity.hold_position();
        entity.set_facing(facing);
        entity.set_weapon_facing(facing);
        entity.set_desired_weapon_facing(facing);
        entity.set_emplacement_facing(Some(facing));
        entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    Ok(gun)
}

fn spawn_enemy_scout_spotter(entities: &mut EntityStore, pos: (f32, f32)) -> Result<u32, String> {
    let scout = entities
        .spawn_unit(2, EntityKind::ScoutCar, pos.0, pos.1)
        .ok_or_else(|| "failed to spawn retreat inspection Scout Car".to_string())?;
    if let Some(entity) = entities.get_mut(scout) {
        entity.hold_position();
        entity.set_facing(std::f32::consts::PI);
        for weapon in WeaponKind::ALL {
            entity.set_weapon_cooldown(weapon, TANK_WEAPON_DELAY_TICKS);
        }
    }
    Ok(scout)
}

fn normalize_dev_angle(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_fire_retreat_places_one_at_gun_in_front_and_a_long_goal_behind() {
        let setup = Game::new_tank_under_fire_retreat_scenario(EntityKind::Tank, 1, 0x5150_0720)
            .expect("under-fire retreat scenario should build");
        let tank = setup
            .game
            .state
            .entities
            .get(setup.units[0])
            .expect("scenario Tank should exist");
        let gun = setup
            .game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 2 && entity.kind == EntityKind::AntiTankGun)
            .expect("scenario Anti-Tank Gun should exist");

        assert!(gun.pos_x > tank.pos_x);
        assert!((gun.pos_y - tank.pos_y).abs() <= 0.001);
        assert!(setup.goal.0 < tank.pos_x - config::TILE_SIZE as f32 * 3.0);
        assert!((setup.goal.1 - tank.pos_y).abs() <= 0.001);
        assert_eq!(setup.issue_after_ticks, config::TICK_HZ * 10);
        assert_tanks_take_front_ap_damage_before_orders(setup);
    }

    #[test]
    fn reverse_traffic_authors_a_ten_degree_fan_with_converging_paths() {
        let setup = Game::new_tank_reverse_traffic_scenario(EntityKind::Tank, 3, 0x5150_0721)
            .expect("reverse-traffic scenario should build");
        assert_eq!(setup.units.len(), 3);
        assert_eq!(
            setup
                .game
                .state
                .entities
                .iter()
                .filter(|entity| entity.owner == 2 && entity.kind == EntityKind::AntiTankGun)
                .count(),
            3
        );
        let spotter = setup
            .game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 2 && entity.kind == EntityKind::ScoutCar)
            .expect("enemy Scout Car spotter should exist");
        let leftmost_gun_x = setup
            .game
            .state
            .entities
            .iter()
            .filter(|entity| entity.owner == 2 && entity.kind == EntityKind::AntiTankGun)
            .map(|gun| gun.pos_x)
            .fold(f32::INFINITY, f32::min);
        assert!(spotter.pos_x < leftmost_gun_x);
        assert!(WeaponKind::ALL
            .into_iter()
            .all(|weapon| spotter.weapon_cooldown(weapon) == TANK_WEAPON_DELAY_TICKS));

        let mut facings = setup
            .units
            .iter()
            .filter_map(|tank_id| setup.game.state.entities.get(*tank_id))
            .map(|tank| tank.facing())
            .collect::<Vec<_>>();
        facings.sort_by(f32::total_cmp);
        let expected_gap = 10.0_f32.to_radians();
        assert!((facings[1] - facings[0] - expected_gap).abs() <= 0.001);
        assert!((facings[2] - facings[1] - expected_gap).abs() <= 0.001);

        let commands = setup.commands();
        assert_eq!(commands.len(), 3);
        for command in commands {
            let SimCommand::Move { units, x, y, .. } = command else {
                panic!("reverse-traffic scenario should author only Move commands");
            };
            assert_eq!(units.len(), 1);
            let tank = setup
                .game
                .state
                .entities
                .get(units[0])
                .expect("commanded Tank should exist");
            let to_goal = (x - tank.pos_x, y - tank.pos_y);
            let to_center = (setup.goal.0 - tank.pos_x, setup.goal.1 - tank.pos_y);
            assert!(to_goal.0 * to_center.0 + to_goal.1 * to_center.1 > 0.0);
            assert!(
                (to_goal.0 * to_center.1 - to_goal.1 * to_center.0).abs() <= 0.1,
                "each authored reverse path should pass through the shared convergence point"
            );
            assert!(
                to_goal.0 * tank.facing().cos() + to_goal.1 * tank.facing().sin() < 0.0,
                "each authored move should begin behind its Tank"
            );
            let sight = config::unit_stats(EntityKind::ScoutCar)
                .expect("Scout Car stats should exist")
                .sight_tiles as f32
                * config::TILE_SIZE as f32;
            assert!(
                (x - spotter.pos_x).hypot(y - spotter.pos_y) <= sight,
                "the spotter should see each complete retreat lane"
            );
        }
        assert_tanks_take_front_ap_damage_before_orders(setup);
    }

    #[test]
    fn reverse_traffic_remains_under_fire_through_the_merge() {
        let mut setup = Game::new_tank_reverse_traffic_scenario(EntityKind::Tank, 3, 0x5150_0722)
            .expect("reverse-traffic scenario should build");
        for _ in 0..setup.issue_after_ticks {
            setup.game.tick();
        }
        for command in setup.commands() {
            setup.game.enqueue(setup.player_id, command);
        }
        for _ in 0..config::TICK_HZ * 5 {
            setup.game.tick();
        }

        let latest_hit = setup
            .units
            .iter()
            .filter_map(|tank_id| setup.game.state.entities.get(*tank_id))
            .filter_map(|tank| tank.last_damage_tick())
            .max()
            .expect("at least one inspection Tank should take damage");
        assert!(
            latest_hit > setup.issue_after_ticks + config::TICK_HZ * 3,
            "the spotter should keep anti-tank fire active late into the merge"
        );
    }

    fn assert_tanks_take_front_ap_damage_before_orders(mut setup: DevScenarioSetup) {
        for _ in 0..setup.issue_after_ticks {
            setup.game.tick();
        }
        for tank_id in setup.units {
            let tank = setup
                .game
                .state
                .entities
                .get(tank_id)
                .expect("inspection Tank should survive the setup pause");
            let source = tank
                .last_damage_pos()
                .expect("front Anti-Tank Gun should damage every inspection Tank");
            let source_angle = (source.1 - tank.pos_y).atan2(source.0 - tank.pos_x);
            assert!(
                crate::game::services::movement::angle_delta(tank.facing(), source_angle).abs()
                    <= std::f32::consts::FRAC_PI_4,
                "incoming AP damage should originate inside the Tank's front arc"
            );
            assert!(
                tank.combat
                    .as_ref()
                    .is_some_and(|combat| combat.tank_armor_reaction_lock.is_some()),
                "front AP damage should establish the existing armor-reaction lock"
            );
        }
    }
}
