use super::*;
use crate::game::entity::Entity;
use crate::game::state::TrackedRng;
use crate::game::trench::{Trench, TrenchStore};

const CASE_TICK_PERFECT: &str = "tick_perfect";
const CASE_MINIMAL_ATTACK_MOVE: &str = "minimal_attack_move";
const CASE_MINIMAL_MOVE: &str = "minimal_move";
const CASE_MINIMAL_NO_ENEMY: &str = "minimal_no_enemy";

const REPLAY_SEED: u32 = 162_367_300;
const REPLAY_PRE_COMMAND_TICK: u32 = 13_536;
const REPLAY_NEXT_ENTITY_ID: u32 = 335;
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
#[cfg(test)]
const GOLDEN_ACTORS: [u32; 5] = [174, 177, COMMAND_CAR, LEAD_TANK, REAR_TANK];
#[cfg(test)]
const GOLDEN_LAST_TICK: u32 = 13_620;

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
        restore_replay_clock_and_trenches(&mut game);

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
    let mut replay_entities: Vec<Entity> =
        serde_json::from_str(include_str!("fixtures/replay_281_tick_13536_entities.json"))
            .map_err(|error| format!("invalid replay-281 entity fixture: {error}"))?;
    replay_entities.retain(|entity| keep.contains(&entity.id));
    replay_entities.sort_by_key(|entity| entity.id);
    if replay_entities
        .windows(2)
        .any(|pair| pair[0].id == pair[1].id)
    {
        return Err("replay-281 entity fixture contains duplicate ids".to_string());
    }
    let entities = EntityStore::from_checkpoint_entities(REPLAY_NEXT_ENTITY_ID, replay_entities);
    if let Some(missing) = keep.iter().find(|id| !entities.contains(**id)) {
        return Err(format!(
            "replay-281 entity fixture is missing required actor {missing}"
        ));
    }
    Ok(entities)
}

fn restore_replay_clock_and_trenches(game: &mut Game) {
    game.state.tick = REPLAY_PRE_COMMAND_TICK;
    game.state.rng = TrackedRng::seed_from_match_seed(REPLAY_SEED);
    game.state.ground_decals.begin_tick(REPLAY_PRE_COMMAND_TICK);
    let radius_tiles = config::ENTRENCHMENT_TRENCH_RADIUS_TILES;
    game.state.trenches = TrenchStore::from_scenario_trenches(
        39,
        vec![
            Trench {
                id: 6,
                x: 1232.0,
                y: 816.0,
                radius_tiles,
            },
            Trench {
                id: 8,
                x: 1232.0,
                y: 848.0,
                radius_tiles,
            },
            Trench {
                id: 38,
                x: 1232.0,
                y: 912.0,
                radius_tiles,
            },
        ],
    );
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
    }

    #[test]
    fn terrain_vehicle_case_repeats_every_tick_deterministically() {
        let mut setup = scenario(CASE_TICK_PERFECT);
        let mut second = scenario(CASE_TICK_PERFECT);
        assert_eq!(setup.game.tick_count(), REPLAY_PRE_COMMAND_TICK);
        assert_eq!(setup.issue_after_ticks, REPLAY_PRE_COMMAND_TICK);
        assert_eq!(
            setup.game.state.entities.next_id_for_test(),
            REPLAY_NEXT_ENTITY_ID
        );
        assert_eq!(
            setup
                .game
                .state
                .trenches
                .all()
                .iter()
                .map(|trench| trench.id)
                .collect::<Vec<_>>(),
            vec![6, 8, 38]
        );
        setup.game.enqueue(setup.player_id, setup.command());
        second.game.enqueue(second.player_id, second.command());
        let golden: Vec<GoldenTick> =
            serde_json::from_str(include_str!("fixtures/replay_281_ticks_13537_13620.json"))
                .expect("valid replay-281 golden trace");
        assert_eq!(
            golden.len(),
            (GOLDEN_LAST_TICK - REPLAY_PRE_COMMAND_TICK) as usize,
            "golden trace must cover every tick through {GOLDEN_LAST_TICK}"
        );

        for (offset, expected_tick) in golden.into_iter().enumerate() {
            let expected_tick_number = REPLAY_PRE_COMMAND_TICK + offset as u32 + 1;
            assert_eq!(
                expected_tick.tick, expected_tick_number,
                "golden trace ticks must be consecutive"
            );
            let mut expected_ids = expected_tick
                .entities
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>();
            expected_ids.sort_unstable();
            assert_eq!(
                expected_ids, GOLDEN_ACTORS,
                "golden tick {} must cover every surviving replay actor",
                expected_tick.tick
            );
            setup.game.tick();
            second.game.tick();
            let snapshot = setup.game.snapshot_full_for_with_options(
                SOUPMAN,
                SnapshotOptions {
                    include_movement_paths: true,
                    movement_paths_for_all_projected: true,
                },
            );
            assert_eq!(snapshot.tick, expected_tick.tick);
            let second_snapshot = second.game.snapshot_full_for_with_options(
                SOUPMAN,
                SnapshotOptions {
                    include_movement_paths: true,
                    movement_paths_for_all_projected: true,
                },
            );
            for expected in expected_tick.entities {
                let actual = snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.id == expected.id)
                    .unwrap_or_else(|| panic!("missing replay actor {}", expected.id));
                let repeated = second_snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.id == expected.id)
                    .unwrap_or_else(|| panic!("missing repeated replay actor {}", expected.id));
                assert_eq!(
                    actual.x.to_bits(),
                    repeated.x.to_bits(),
                    "tick {} entity {} x",
                    snapshot.tick,
                    expected.id
                );
                assert_eq!(
                    actual.y.to_bits(),
                    repeated.y.to_bits(),
                    "tick {} entity {} y",
                    snapshot.tick,
                    expected.id
                );
                assert_eq!(
                    actual.hp, repeated.hp,
                    "tick {} entity {} hp",
                    snapshot.tick, expected.id
                );
                assert_eq!(
                    actual.state, repeated.state,
                    "tick {} entity {} state",
                    snapshot.tick, expected.id
                );
                assert_eq!(
                    actual.target_id, repeated.target_id,
                    "tick {} entity {} target",
                    snapshot.tick, expected.id
                );
                assert_eq!(
                    actual.debug_path.as_ref().map(|path| path.stuck_ticks),
                    repeated.debug_path.as_ref().map(|path| path.stuck_ticks),
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
        assert!(
            matches!(acquired_at, Some(13_543..=13_544)),
            "authored vehicle route should preserve the same target-acquisition episode: {acquired_at:?}"
        );
        assert!(
            rear_slowed_at.is_some_and(|tick| tick > REPLAY_PRE_COMMAND_TICK),
            "rear Tank should still exhibit the recorded slowdown episode: {rear_slowed_at:?}"
        );
        assert!(
            target_removed_at.is_some_and(|tick| acquired_at.is_some_and(|acquired| tick > acquired)),
            "target removal should follow acquisition: acquired={acquired_at:?} removed={target_removed_at:?}"
        );
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
