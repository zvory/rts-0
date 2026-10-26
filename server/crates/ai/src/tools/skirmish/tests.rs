use std::collections::BTreeSet;

use super::fight::{run_fight, ControllerSpec, FightOutcome, FightSpec, ReplayPolicy};
use super::scenario::{place_squads, PlacementInput, Suite};
use super::*;
use crate::selfplay::is_safe_artifact_name;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|arg| arg.to_string()).collect()
}

#[test]
fn defaults_pit_the_micro_planner_against_ai_2_1_on_train() {
    let config = parse_args(args(&[])).unwrap().unwrap();
    assert_eq!(
        config.candidate,
        ControllerSpec::Micro(RifleSquadParams::micro())
    );
    assert_eq!(config.opponent, ControllerSpec::Profile("ai_2_1"));
    assert_eq!(config.suite, Suite::Train);
    assert_eq!(config.replays, ReplayPolicy::Losses);
    assert_eq!(config.squad_size, DEFAULT_SQUAD_SIZE);
    assert!(config.out_dir.ends_with("ai-skirmish/latest"));
}

#[test]
fn controller_specs_cover_presets_overrides_and_profiles() {
    assert_eq!(
        ControllerSpec::parse("naive").unwrap(),
        ControllerSpec::Micro(RifleSquadParams::naive())
    );
    let ControllerSpec::Micro(params) =
        ControllerSpec::parse("micro:hold=1,retreat_hp=10").unwrap()
    else {
        panic!("expected micro");
    };
    assert!(params.hold);
    assert_eq!(params.retreat_hp, 10);
    assert_eq!(
        ControllerSpec::parse("profile:jeffs_ai").unwrap(),
        ControllerSpec::Profile("jeffs_ai")
    );
    assert!(ControllerSpec::parse("micro:warp=1").is_err());
    assert!(ControllerSpec::parse("skynet").is_err());
}

#[test]
fn bad_arguments_are_rejected() {
    for bad in [
        &["--seeds", "0"][..],
        &["--squad-size", "0"],
        &["--squad-size", "999"],
        &["--tag", "../escape"],
        &["--suite", "everything"],
        &["--replays", "some"],
        &["--ticks"],
        &["--frobnicate"],
    ] {
        assert!(parse_args(args(bad)).is_err(), "{bad:?} should be rejected");
    }
    assert_eq!(
        parse_args(args(&["--sweep", "4"]))
            .unwrap()
            .unwrap()
            .replays,
        ReplayPolicy::None,
        "sweeps skip per-trial replays unless asked"
    );
}

#[test]
fn mutation_changes_at_most_three_settings_of_the_base() {
    let base = RifleSquadParams::micro();
    let mut rng = SmallRng::seed_from_u64(9);
    for _ in 0..64 {
        let mutated = mutated_params(base, &mut rng);
        let changed = RifleSquadParams::KEYS
            .iter()
            .filter(|key| {
                let mut probe = base;
                probe.copy_key_from(key, &mutated);
                probe != base
            })
            .count();
        assert!(changed <= 3, "{mutated}");
    }
    assert!(parse_args(args(&["--sweep", "2", "--mutate", "--candidate", "ai_2_1"])).is_err());
}

#[test]
fn suites_have_stable_unique_artifact_safe_ids() {
    assert_eq!(Suite::Smoke.scenarios().len(), 1);
    assert_eq!(Suite::Train.scenarios().len(), 24);
    assert_eq!(Suite::Holdout.scenarios().len(), 12);
    let all = Suite::All.scenarios();
    assert_eq!(all.len(), 36);
    let ids: BTreeSet<&str> = all.iter().map(|scenario| scenario.id.as_str()).collect();
    assert_eq!(ids.len(), all.len(), "scenario ids must be unique");
    for id in ids {
        assert!(
            is_safe_artifact_name(&format!("skirmish_latest_{id}_p1_s0")),
            "{id}"
        );
    }
    let train: BTreeSet<String> = Suite::Train.scenarios().into_iter().map(|s| s.id).collect();
    assert!(Suite::Holdout
        .scenarios()
        .iter()
        .all(|scenario| !train.contains(&scenario.id)));
}

#[test]
fn placement_puts_each_squad_on_its_own_side_of_the_gap() {
    let world = (126.0 * 32.0, 126.0 * 32.0);
    for scenario in Suite::All.scenarios() {
        let input = PlacementInput {
            squad_size: 4,
            candidate_base: (816.0, 816.0),
            opponent_base: (3216.0, 3216.0),
            tile_px: 32.0,
            world_px: world,
            seed: 3,
        };
        let placement = place_squads(&scenario, &input);
        assert_eq!(placement.candidate.len(), 4);
        assert_eq!(placement.opponent.len(), 4);
        let centre = |points: &[(f32, f32)]| {
            let n = points.len() as f32;
            (
                points.iter().map(|p| p.0).sum::<f32>() / n,
                points.iter().map(|p| p.1).sum::<f32>() / n,
            )
        };
        let (ours, theirs) = (centre(&placement.candidate), centre(&placement.opponent));
        let to_base = |p: (f32, f32)| (p.0 - 816.0).hypot(p.1 - 816.0);
        assert!(to_base(ours) < to_base(theirs), "{}", scenario.id);
        let gap = (ours.0 - theirs.0).hypot(ours.1 - theirs.1) / 32.0;
        let expected = scenario.gap_tiles.hypot(scenario.lateral_tiles);
        assert!(
            (gap - expected).abs() < 1.5,
            "{}: tile snapping keeps the gap near nominal, {gap} vs {expected}",
            scenario.id
        );
        let tiles: BTreeSet<(i32, i32)> = placement
            .candidate
            .iter()
            .chain(&placement.opponent)
            .map(|&(x, y)| ((x / 32.0).floor() as i32, (y / 32.0).floor() as i32))
            .collect();
        assert_eq!(tiles.len(), 8, "{}: one Rifleman per tile", scenario.id);
        for &(x, y) in placement.candidate.iter().chain(&placement.opponent) {
            assert!(x > 0.0 && y > 0.0 && x < world.0 && y < world.1);
        }
        assert_eq!(place_squads(&scenario, &input), placement, "deterministic");
        let reseeded = place_squads(&scenario, &PlacementInput { seed: 4, ..input });
        assert_ne!(reseeded, placement, "seeds jitter the spawn points");
    }
}

#[test]
fn ai_2_1_engages_the_micro_squad_and_the_fight_writes_a_replay() {
    let scenario = Suite::Smoke.scenarios().remove(0);
    let candidate = ControllerSpec::Micro(RifleSquadParams::micro());
    let opponent = ControllerSpec::Profile("ai_2_1");
    let replay_dir = std::env::temp_dir().join(format!("rts-ai-skirmish-test-{}", process::id()));
    let result = run_fight(&FightSpec {
        scenario: &scenario,
        candidate: &candidate,
        opponent: &opponent,
        candidate_player: 2,
        seed: 0,
        max_ticks: DEFAULT_TICKS,
        map_name: DEFAULT_MAP,
        squad_size: 4,
        replay_policy: ReplayPolicy::All,
        replay_dir: &replay_dir,
        replay_tag: "test",
    })
    .expect("skirmish runs");
    assert!(
        result.first_damage_tick.is_some(),
        "AI 2.1 must fight its four starting-wave Riflemen for this testbed to mean anything"
    );
    assert!(matches!(
        result.outcome,
        FightOutcome::Win | FightOutcome::Loss
    ));
    assert!(result.candidate_survivors <= 4 && result.opponent_survivors <= 4);
    assert!((-1.0..=1.0).contains(&result.hp_margin));
    assert!(
        !result.reinforced,
        "no side should reinforce inside the tick cap"
    );
    assert!(result.candidate_shots > 0 && result.opponent_shots > 0);
    let name = result.replay_artifact.expect("replay requested");
    let replay: serde_json::Value = serde_json::from_slice(
        &fs::read(replay_dir.join(&name).join("replay.json")).expect("replay written"),
    )
    .expect("replay is JSON");
    assert!(replay.get("artifactSchemaVersion").is_some());
    assert!(replay
        .get("commandLog")
        .and_then(|log| log.as_array())
        .is_some_and(|log| !log.is_empty()));
    let _ = fs::remove_dir_all(&replay_dir);
}
