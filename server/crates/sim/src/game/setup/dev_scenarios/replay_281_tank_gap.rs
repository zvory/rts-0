use super::*;
use crate::game::entity::WeaponSetup;
use crate::rules::combat::WeaponKind;

const CASE_LIVING_COMMAND_CAR: &str = "living_command_car";
const CASE_ENEMY_SCREEN: &str = "enemy_screen";
const CASE_COMMAND_CAR_DEATH: &str = "command_car_death";

const ISSUE_AFTER_TICKS: u32 = config::TICK_HZ * 10;
const REPLAY_SEED: u32 = 162_367_300;
const REPLAY_TANK_RELOAD_REMAINING: u32 = 45;
const GOAL: (f32, f32) = (1091.3671, 933.82385);
const START_TILE: (u32, u32) = (47, 30);

const TANK_SPECS: [(f32, f32, u32, f32); 5] = [
    (1389.6304, 975.1114, 282, -2.538_128_4),
    (1641.5762, 1047.2806, 279, -2.658_286_8),
    (1750.4877, 1046.1906, 292, -2.660_228),
    (1464.993, 982.82104, 288, -2.838_678_1),
    (1496.448, 935.6931, 283, -2.393_564),
];

const MACHINE_GUNNER_SPECS: [(f32, f32, u32, f32, u32); 3] = [
    (1232.0, 816.0, 55, 1.038_368_6, 1),
    (1232.0, 848.0, 48, 0.934_669_8, 2),
    (1200.0, 816.0, 55, 0.901_311_34, 3),
];

impl Game {
    pub fn new_replay_281_tank_gap_scenario(
        scenario_case: Option<&str>,
        unit: EntityKind,
        unit_count: usize,
        _seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Tank || unit_count != 5 {
            return Err(format!(
                "replay-281 tank gap requires five Tanks, got {unit_count} {unit}"
            ));
        }
        let scenario_case = scenario_case.ok_or_else(|| {
            "missing replay-281 tank gap case; expected living_command_car, enemy_screen, or command_car_death".to_string()
        })?;
        if !matches!(
            scenario_case,
            CASE_LIVING_COMMAND_CAR | CASE_ENEMY_SCREEN | CASE_COMMAND_CAR_DEATH
        ) {
            return Err(format!(
                "unsupported replay-281 tank gap case {scenario_case}"
            ));
        }

        let mut map = Map::load("Schone Tage", 2, REPLAY_SEED)
            .map_err(|error| format!("failed to load replay-281 map: {error}"))?;
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = START_TILE;
        }

        let mut entities = EntityStore::new();
        let tanks = spawn_replay_tanks(&mut entities, scenario_case == CASE_COMMAND_CAR_DEATH)?;
        let include_enemy_screen =
            matches!(scenario_case, CASE_ENEMY_SCREEN | CASE_COMMAND_CAR_DEATH);

        let command_car = spawn_command_car(
            &mut entities,
            scenario_case == CASE_COMMAND_CAR_DEATH,
            scenario_case == CASE_ENEMY_SCREEN,
        )?;
        let scout_car = spawn_scout_car(&mut entities)?;
        if include_enemy_screen {
            spawn_machine_gunner_screen(
                &mut entities,
                (scenario_case == CASE_COMMAND_CAR_DEATH).then_some(command_car),
            )?;
        }

        // Preserve the replay's tick-13,537 command ordering so formation slot assignment is the
        // same in the full case: Command Car, Scout Car, then Tanks 251/250/269/271/284.
        let mut ordered_units = Vec::with_capacity(7);
        ordered_units.push(command_car);
        ordered_units.push(scout_car);
        ordered_units.extend(tanks);

        let game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            1,
            START_TILE,
            REPLAY_SEED,
            "dev:replay_281_tank_gap",
        );

        DevScenarioSetup {
            game,
            player_id: 1,
            units: ordered_units,
            goal: GOAL,
            issue_after_ticks: ISSUE_AFTER_TICKS,
            order: DevScenarioOrder::AttackMove,
        }
        .checkpoint_backed("dev:replay_281_tank_gap")
    }
}

fn spawn_replay_tanks(entities: &mut EntityStore, delay_fire: bool) -> Result<Vec<u32>, String> {
    TANK_SPECS
        .into_iter()
        .map(|(x, y, hp, facing)| {
            let id = spawn_replay_unit(entities, 1, EntityKind::Tank, x, y, hp, facing)?;
            if delay_fire {
                let entity = entities
                    .get_mut(id)
                    .ok_or_else(|| "spawned replay-281 Tank is missing".to_string())?;
                // The replay's MG 226 kill depended on the tanks still being inside their
                // carried-over reload window instead of immediately deleting the MG screen.
                let cooldown = ISSUE_AFTER_TICKS + REPLAY_TANK_RELOAD_REMAINING;
                entity.set_weapon_cooldown(WeaponKind::TankCannon, cooldown);
                entity.set_weapon_cooldown(WeaponKind::TankCoax, cooldown);
            }
            Ok(id)
        })
        .collect()
}

fn spawn_command_car(
    entities: &mut EntityStore,
    vulnerable: bool,
    invulnerable: bool,
) -> Result<u32, String> {
    let hp = if vulnerable { 46 } else { 150 };
    let id = spawn_replay_unit(
        entities,
        1,
        EntityKind::CommandCar,
        1325.2281,
        974.233,
        hp,
        -2.502_215_1,
    )?;
    if let Some(entity) = entities.get_mut(id) {
        entity.set_invulnerable(invulnerable);
    }
    Ok(id)
}

fn spawn_scout_car(entities: &mut EntityStore) -> Result<u32, String> {
    spawn_replay_unit(
        entities,
        1,
        EntityKind::ScoutCar,
        1448.0062,
        937.9907,
        34,
        -2.823_458,
    )
}

fn spawn_replay_unit(
    entities: &mut EntityStore,
    owner: u32,
    kind: EntityKind,
    x: f32,
    y: f32,
    hp: u32,
    facing: f32,
) -> Result<u32, String> {
    let id = entities
        .spawn_unit(owner, kind, x, y)
        .ok_or_else(|| format!("failed to spawn replay-281 {kind}"))?;
    let entity = entities
        .get_mut(id)
        .ok_or_else(|| format!("spawned replay-281 {kind} is missing"))?;
    let damage = entity.hp.saturating_sub(hp);
    entity.apply_damage(damage, None);
    entity.set_facing(facing);
    entity.set_weapon_facing(facing);
    Ok(id)
}

fn spawn_machine_gunner_screen(
    entities: &mut EntityStore,
    command_car_target: Option<u32>,
) -> Result<(), String> {
    for (index, (x, y, hp, facing, fire_offset)) in MACHINE_GUNNER_SPECS.into_iter().enumerate() {
        let id = spawn_replay_unit(entities, 2, EntityKind::MachineGunner, x, y, hp, facing)?;
        let entity = entities
            .get_mut(id)
            .ok_or_else(|| "spawned replay-281 Machine Gunner is missing".to_string())?;
        entity.set_weapon_setup(WeaponSetup::Deployed);
        entity.set_weapon_cooldown(
            WeaponKind::MachineGunnerMg,
            ISSUE_AFTER_TICKS.saturating_add(fire_offset),
        );
        if index == 2 {
            if let Some(command_car) = command_car_target {
                entity.set_order(crate::game::entity::Order::attack(command_car));
                entity.set_target_id(Some(command_car));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_case_removes_the_command_car_and_keeps_all_five_tanks_live() {
        let mut setup = Game::new_replay_281_tank_gap_scenario(
            Some(CASE_COMMAND_CAR_DEATH),
            EntityKind::Tank,
            5,
            0x0281_1357,
        )
        .expect("replay-281 full scenario");
        let command = setup.command();
        let command_car = setup.units[0];
        let tank_ids = setup.units[2..].to_vec();

        for _ in 0..setup.issue_after_ticks {
            setup.game.tick();
        }
        setup.game.enqueue(setup.player_id, command);

        let mut removed_at = None;
        for _ in 0..180 {
            setup.game.tick();
            let snapshot = setup.game.snapshot_full_for(setup.player_id);
            if !snapshot
                .entities
                .iter()
                .any(|entity| entity.id == command_car)
            {
                removed_at = Some(snapshot.tick);
                assert!(tank_ids
                    .iter()
                    .all(|tank_id| snapshot.entities.iter().any(|entity| entity.id == *tank_id)));
                break;
            }
        }

        assert!(
            removed_at.is_some(),
            "the low-health lead car should die; final={:?}; enemies={:?}",
            setup.game.state.entities.get(command_car).map(|entity| (
                entity.hp,
                entity.pos_x,
                entity.pos_y,
                entity.order(),
                entity.target_id()
            )),
            setup
                .game
                .state
                .entities
                .iter()
                .filter(|entity| entity.owner == 2)
                .map(|entity| (
                    entity.id,
                    entity.hp,
                    entity.pos_x,
                    entity.pos_y,
                    entity.order(),
                    entity.target_id(),
                    entity.weapon_cooldown(WeaponKind::MachineGunnerMg)
                ))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn living_command_car_case_advances_through_the_real_gap() {
        let mut setup = Game::new_replay_281_tank_gap_scenario(
            Some(CASE_LIVING_COMMAND_CAR),
            EntityKind::Tank,
            5,
            0x0281_1357,
        )
        .expect("replay-281 tanks-only scenario");
        let command = setup.command();
        for _ in 0..setup.issue_after_ticks {
            setup.game.tick();
        }
        setup.game.enqueue(setup.player_id, command);
        setup.game.tick();
        assert!(
            setup.units.iter().any(|id| setup
                .game
                .state
                .entities
                .get(*id)
                .is_some_and(|entity| !matches!(entity.order(), crate::game::entity::Order::Idle))),
            "attack-move should be accepted"
        );
        for _ in 1..360 {
            setup.game.tick();
        }
        let snapshot = setup.game.snapshot_full_for(setup.player_id);
        let tanks = snapshot
            .entities
            .iter()
            .filter(|entity| entity.owner == setup.player_id && entity.kind == "tank")
            .collect::<Vec<_>>();
        assert_eq!(tanks.len(), 5);
        assert!(
            tanks.iter().any(|tank| tank.x < 1248.0),
            "at least one Tank should clear the eastern edge of the seven-tile gap; positions={:?}",
            tanks
                .iter()
                .map(|tank| (tank.id, tank.x, tank.y, tank.state.as_str()))
                .collect::<Vec<_>>()
        );
    }
}
