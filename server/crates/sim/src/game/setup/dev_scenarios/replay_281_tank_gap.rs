use super::*;
use crate::game::entity::Entity;
use crate::game::state::TrackedRng;
use crate::game::trench::TrenchStore;

const CASE_TICK_PERFECT: &str = "tick_perfect";
const CASE_MINIMAL_ATTACK_MOVE: &str = "minimal_attack_move";
const CASE_MINIMAL_MOVE: &str = "minimal_move";
const CASE_MINIMAL_NO_ENEMY: &str = "minimal_no_enemy";

const REPLAY_SEED: u32 = 162_367_300;
const REPLAY_PRE_COMMAND_TICK: u32 = 13_536;
const GOAL: (f32, f32) = (1091.3671, 933.82385);
const SOUPMAN: u32 = 15;
const ALEX: u32 = 13;
const LEAD_TANK: u32 = 251;
const REAR_TANK: u32 = 269;
const COMMAND_CAR: u32 = 229;
const TARGET_MACHINE_GUNNER: u32 = 232;
const EXACT_ACTORS: [u32; 6] = [
    174,
    177,
    COMMAND_CAR,
    TARGET_MACHINE_GUNNER,
    LEAD_TANK,
    REAR_TANK,
];
const EXACT_COMMAND_UNITS: [u32; 7] = [COMMAND_CAR, 219, LEAD_TANK, 250, REAR_TANK, 271, 284];

impl Game {
    pub fn new_replay_281_tank_gap_scenario(
        scenario_case: Option<&str>,
        unit: EntityKind,
        unit_count: usize,
        _seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Tank || unit_count != 5 {
            return Err(format!(
                "replay-281 tank gap requires the recorded five-Tank selection label, got {unit_count} {unit}"
            ));
        }
        let scenario_case = scenario_case.ok_or_else(|| {
            "missing replay-281 tank gap case; expected tick_perfect, minimal_attack_move, minimal_move, or minimal_no_enemy".to_string()
        })?;
        if !matches!(
            scenario_case,
            CASE_TICK_PERFECT
                | CASE_MINIMAL_ATTACK_MOVE
                | CASE_MINIMAL_MOVE
                | CASE_MINIMAL_NO_ENEMY
        ) {
            return Err(format!(
                "unsupported replay-281 tank gap case {scenario_case}"
            ));
        }

        let keep = match scenario_case {
            CASE_TICK_PERFECT => EXACT_ACTORS.as_slice(),
            CASE_MINIMAL_NO_ENEMY => &[LEAD_TANK, REAR_TANK],
            CASE_MINIMAL_ATTACK_MOVE | CASE_MINIMAL_MOVE => {
                &[TARGET_MACHINE_GUNNER, LEAD_TANK, REAR_TANK]
            }
            _ => unreachable!("validated replay-281 case"),
        };
        let entities = replay_entities(keep)?;
        let map = Map::load("Schone Tage", 2, REPLAY_SEED)
            .map_err(|error| format!("failed to load replay-281 map: {error}"))?;
        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(ALEX, 1), (SOUPMAN, 2)],
            SOUPMAN,
            (157, 47),
            REPLAY_SEED,
            "dev:replay_281_tank_gap",
        );
        restore_replay_clock_and_trenches(&mut game)?;

        let (units, order) = if scenario_case == CASE_TICK_PERFECT {
            (EXACT_COMMAND_UNITS.to_vec(), DevScenarioOrder::AttackMove)
        } else {
            (
                vec![LEAD_TANK, REAR_TANK],
                if scenario_case == CASE_MINIMAL_MOVE {
                    DevScenarioOrder::Move
                } else {
                    DevScenarioOrder::AttackMove
                },
            )
        };

        DevScenarioSetup {
            game,
            player_id: SOUPMAN,
            units,
            goal: GOAL,
            issue_after_ticks: REPLAY_PRE_COMMAND_TICK,
            order,
        }
        .checkpoint_backed(&format!("dev:replay_281_tank_gap:{scenario_case}"))
    }
}

fn replay_entities(keep: &[u32]) -> Result<EntityStore, String> {
    let replay_entities: Vec<Entity> =
        serde_json::from_str(include_str!("fixtures/replay_281_tick_13536_entities.json"))
            .map_err(|error| format!("invalid replay-281 entity fixture: {error}"))?;
    let mut entities = EntityStore::new();
    for id in 1..=REAR_TANK {
        if let Some(entity) = replay_entities.iter().find(|entity| entity.id == id) {
            let inserted = entities.insert(entity.clone());
            debug_assert_eq!(inserted, id);
        } else {
            let placeholder = Entity::new_unit(SOUPMAN, EntityKind::Rifleman, 5000.0, 5000.0)
                .ok_or_else(|| "failed to allocate replay-281 id placeholder".to_string())?;
            let inserted = entities.insert(placeholder);
            debug_assert_eq!(inserted, id);
            let _ = entities.remove(inserted);
        }
    }
    for id in EXACT_ACTORS {
        if !keep.contains(&id) {
            let _ = entities.remove(id);
        }
    }
    if let Some(missing) = keep.iter().find(|id| !entities.contains(**id)) {
        return Err(format!(
            "replay-281 entity fixture is missing required actor {missing}"
        ));
    }
    Ok(entities)
}

fn restore_replay_clock_and_trenches(game: &mut Game) -> Result<(), String> {
    game.state.tick = REPLAY_PRE_COMMAND_TICK;
    game.state.rng = TrackedRng::seed_from_match_seed(REPLAY_SEED);
    game.state.ground_decals.begin_tick(REPLAY_PRE_COMMAND_TICK);
    game.state.trenches = TrenchStore::new();
    for id in 1..=38 {
        let (x, y) = match id {
            6 => (1232.0, 816.0),
            8 => (1232.0, 848.0),
            38 => (1232.0, 912.0),
            _ => (16.0 + id as f32, 16.0),
        };
        let created = game
            .state
            .trenches
            .create(&game.state.map, x, y)
            .ok_or_else(|| format!("failed to restore replay-281 trench {id}"))?;
        debug_assert_eq!(created, id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GoldenTick {
        tick: u32,
        entities: Vec<GoldenEntity>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct GoldenEntity {
        id: u32,
        x: f32,
        y: f32,
        hp: u32,
        state: String,
        target_id: Option<u32>,
        stuck_ticks: Option<u16>,
    }

    #[test]
    fn tick_perfect_case_matches_every_recorded_replay_tick() {
        let mut setup = scenario(CASE_TICK_PERFECT);
        assert_eq!(setup.game.tick_count(), REPLAY_PRE_COMMAND_TICK);
        setup.game.enqueue(setup.player_id, setup.command());
        let golden: Vec<GoldenTick> =
            serde_json::from_str(include_str!("fixtures/replay_281_ticks_13537_13620.json"))
                .expect("valid replay-281 golden trace");

        for expected_tick in golden {
            setup.game.tick();
            let snapshot = setup.game.snapshot_full_for_with_options(
                SOUPMAN,
                SnapshotOptions {
                    include_movement_paths: true,
                    movement_paths_for_all_projected: true,
                },
            );
            assert_eq!(snapshot.tick, expected_tick.tick);
            for expected in expected_tick.entities {
                let actual = snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.id == expected.id)
                    .unwrap_or_else(|| panic!("missing replay actor {}", expected.id));
                assert_eq!(
                    actual.x.to_bits(),
                    expected.x.to_bits(),
                    "tick {} entity {} x",
                    snapshot.tick,
                    expected.id
                );
                assert_eq!(
                    actual.y.to_bits(),
                    expected.y.to_bits(),
                    "tick {} entity {} y",
                    snapshot.tick,
                    expected.id
                );
                assert_eq!(
                    actual.hp, expected.hp,
                    "tick {} entity {} hp",
                    snapshot.tick, expected.id
                );
                assert_eq!(
                    actual.state, expected.state,
                    "tick {} entity {} state",
                    snapshot.tick, expected.id
                );
                assert_eq!(
                    actual.target_id, expected.target_id,
                    "tick {} entity {} target",
                    snapshot.tick, expected.id
                );
                assert_eq!(
                    actual.debug_path.as_ref().map(|path| path.stuck_ticks),
                    expected.stuck_ticks,
                    "tick {} entity {} stuck ticks",
                    snapshot.tick,
                    expected.id
                );
            }
        }
    }

    #[test]
    fn minimal_case_reproduces_attack_move_jam_and_move_control_clears_it() {
        let attack = positions_after(CASE_MINIMAL_ATTACK_MOVE, 84);
        let no_enemy = positions_after(CASE_MINIMAL_NO_ENEMY, 84);
        let direct_move = positions_after(CASE_MINIMAL_MOVE, 84);

        assert!(
            attack.0 > 1380.0,
            "lead Tank should stop to attack the MG: {attack:?}"
        );
        assert!(
            attack.1 > 1390.0,
            "rear Tank should queue behind the stopped Tank: {attack:?}"
        );
        assert!(
            no_enemy.0 < 1320.0 && no_enemy.1 < 1370.0,
            "without the target both Tanks advance: {no_enemy:?}"
        );
        assert!(
            direct_move.0 < 1320.0 && direct_move.1 < 1370.0,
            "Move should ignore the target and advance: {direct_move:?}"
        );
    }

    #[test]
    fn replay_cause_is_lead_tank_target_acquisition_not_a_ghost_collider() {
        let mut setup = scenario(CASE_TICK_PERFECT);
        setup.game.enqueue(setup.player_id, setup.command());
        let mut acquired_at = None;
        let mut target_removed_at = None;
        let mut rear_slowed_at = None;
        while setup.game.tick_count() < 13_700 {
            setup.game.tick();
            let tick = setup.game.tick_count();
            let lead = setup.game.state.entities.get(LEAD_TANK).expect("lead Tank");
            if acquired_at.is_none() && lead.target_id() == Some(TARGET_MACHINE_GUNNER) {
                acquired_at = Some(tick);
            }
            if target_removed_at.is_none()
                && !setup.game.state.entities.contains(TARGET_MACHINE_GUNNER)
            {
                target_removed_at = Some(tick);
            }
            let rear = setup.game.state.entities.get(REAR_TANK).expect("rear Tank");
            if rear_slowed_at.is_none()
                && rear.movement.as_ref().is_some_and(|movement| {
                    movement.last_move_delta.0.hypot(movement.last_move_delta.1) < 1.0
                })
            {
                rear_slowed_at = Some(tick);
            }
        }
        assert_eq!(acquired_at, Some(13_543));
        assert_eq!(rear_slowed_at, Some(13_579));
        assert_eq!(target_removed_at, Some(13_599));
        assert!(setup.game.state.entities.contains(COMMAND_CAR));
        assert!(!setup.game.state.entities.contains(219));
        assert!(!setup.game.state.entities.contains(250));
    }

    fn scenario(case: &str) -> DevScenarioSetup {
        Game::new_replay_281_tank_gap_scenario(Some(case), EntityKind::Tank, 5, 0)
            .expect("replay-281 scenario")
    }

    fn positions_after(case: &str, ticks: usize) -> (f32, f32) {
        let mut setup = scenario(case);
        setup.game.enqueue(setup.player_id, setup.command());
        for _ in 0..ticks {
            setup.game.tick();
        }
        let lead = setup.game.state.entities.get(LEAD_TANK).expect("lead Tank");
        let rear = setup.game.state.entities.get(REAR_TANK).expect("rear Tank");
        (lead.pos_x, rear.pos_x)
    }
}
