use super::*;
use crate::ai_core::profiles::JEFFS_AI;

#[test]
fn failed_pushes_go_straight_then_a_random_side_then_the_other_side_then_straight_again() {
    let mut roundabout = Roundabout::default();
    assert_eq!(roundabout.approach(), Approach::Direct);
    let mut first_sides = BTreeSet::new();
    for loop_index in 0..40_u32 {
        let tick = 1_000 + loop_index * 997;
        roundabout.note_failed_push(1, tick);
        let side = roundabout.approach();
        assert_ne!(side, Approach::Direct);
        first_sides.insert(format!("{side:?}"));
        roundabout.note_failed_push(1, tick + 300);
        assert_eq!(roundabout.approach(), side.opposite());
        roundabout.note_failed_push(1, tick + 600);
        assert_eq!(roundabout.approach(), Approach::Direct);
    }
    // The first side of a loop is not always the same one.
    assert_eq!(first_sides.len(), 2);
}

#[test]
fn the_random_side_is_the_same_for_the_same_game_facts() {
    for tick in [0, 1, 4_567, 12_345, 24_999] {
        for player in [1, 2] {
            assert_eq!(random_side(player, tick, 1), random_side(player, tick, 1));
        }
    }
}

#[test]
fn a_push_that_starts_again_swings_out_again() {
    let mut roundabout = Roundabout {
        swing_reached: true,
        swing_started_tick: Some(10),
        ..Roundabout::default()
    };
    roundabout.start_push();
    assert!(!roundabout.swing_reached);
    assert_eq!(roundabout.swing_started_tick, None);
}

#[test]
fn a_push_already_on_the_lanes_side_closes_in_without_swinging_out_again() {
    let ts = 32.0;
    let at = |x: f32, y: f32| (x * ts, y * ts);
    // Target at (60, 60); the direct approach comes from the south-west; the lane's side is east.
    let lane = FlankLane {
        swing: stored(at(83.5, 60.0)),
        attack: stored(at(73.5, 60.0)),
        approach_from: stored(at(93.5, 60.0)),
    };
    let objective = at(60.0, 60.0);
    // At the old natural, east of the new target and inside the swing distance: on the side.
    assert!(on_lane_side(at(80.0, 64.0), objective, lane, ts));
    // Back home down the direct approach: not on the side.
    assert!(!on_lane_side(at(45.0, 45.0), objective, lane, ts));
    // East but far beyond the swing point: still has to come in.
    assert!(!on_lane_side(at(110.0, 60.0), objective, lane, ts));
    // Close, but round on the other side of the target.
    assert!(!on_lane_side(at(50.0, 60.0), objective, lane, ts));
}

fn map_observation(name: &str, player: u32, seed: u32) -> (AiObservation, AiMapAnalysis) {
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};
    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    let map = Map::load_for_players(name, &[(1, 1), (2, 2)], seed).unwrap();
    let game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        seed,
        map,
        Map::metadata_for_name(name).unwrap(),
    );
    let start = game.start_payload();
    let observation = AiObservation::from_snapshot_with_alive(
        &start,
        &game.snapshot_for(player),
        player,
        [],
        None,
    )
    .unwrap();
    (observation, AiMapAnalysis::analyze(&start))
}

/// The enemy natural, the direct attack point on it, and the left and right lanes to it.
type NaturalLanes = ((f32, f32), (f32, f32), [Option<FlankLane>; 2]);

/// The lanes to the enemy natural from both sides, as a push setting out from the regroup point
/// would see them.
fn lanes(observation: &AiObservation, analysis: &AiMapAnalysis) -> NaturalLanes {
    let policy = JEFFS_AI.expansion_containment.unwrap();
    let facts = AiFacts::from_observation(observation);
    let enemy_base = facts.nearest_public_enemy_base.unwrap();
    let objective = enemy_natural_edge(observation, enemy_base).unwrap();
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let rally = containment_regroup_point(own_base, enemy_base, observation.map).unwrap();
    let (direct_point, _) =
        containment_points(own_base, objective, observation.map, policy).unwrap();
    let lane = |approach| {
        flank_lane(
            analysis,
            observation,
            rally,
            (enemy_base.x, enemy_base.y),
            objective,
            direct_point,
            policy.tank_standoff_tiles,
            approach,
        )
    };
    (
        objective,
        direct_point,
        [lane(Approach::Left), lane(Approach::Right)],
    )
}

#[test]
fn side_lanes_come_at_the_enemy_natural_well_off_the_direct_approach() {
    let ts = 32.0;
    let mut sides_found = 0;
    for (name, seeds) in [
        ("Classic", vec![0x1234_5678_u32]),
        ("The River", vec![0x1234_5678]),
        ("Schone Tage", vec![0x1234_5678]),
        ("Crossroads", vec![0x1234_5678, 1, 2, 3]),
    ] {
        for seed in seeds {
            for player in [1, 2] {
                let (observation, analysis) = map_observation(name, player, seed);
                let (objective, direct_point, sides) = lanes(&observation, &analysis);
                let direct_dir = normalized_direction(objective, direct_point).unwrap();
                let tile = |p: (f32, f32)| ((p.0 / ts) as i32, (p.1 / ts) as i32);
                println!(
                    "{name} seed {seed} p{player} start {:?} target {:?} direct {:?}",
                    observation.own_start_tile,
                    tile(objective),
                    tile(direct_point),
                );
                for (label, side) in ["left", "right"].iter().zip(sides) {
                    let Some(side) = side else {
                        println!("  {label}: none");
                        continue;
                    };
                    sides_found += 1;
                    let (swing, attack) = (world(side.swing), world(side.attack));
                    let from_target =
                        |p: (f32, f32)| dist2(p.0, p.1, objective.0, objective.1).sqrt() / ts;
                    let dir = normalized_direction(objective, attack).unwrap();
                    let degrees = (dir.0 * direct_dir.0 + dir.1 * direct_dir.1)
                        .clamp(-1.0, 1.0)
                        .acos()
                        .to_degrees();
                    println!(
                        "  {label}: swing {:?} ({:.1}t) attack {:?} ({:.1}t) {:.0} deg off direct",
                        tile(swing),
                        from_target(swing),
                        tile(attack),
                        from_target(attack),
                        degrees,
                    );
                    assert!(from_target(swing) > from_target(attack), "{name} p{player}");
                    assert!(degrees >= 35.0, "{name} p{player} {label}: {degrees}");
                }
            }
        }
    }
    assert!(sides_found > 0);
}
