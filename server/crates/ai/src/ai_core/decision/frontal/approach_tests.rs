use super::*;
use crate::ai_core::observation::AiEconomy;
use crate::ai_core::profiles::JEFFS_AI;

#[test]
fn failed_pushes_go_straight_then_a_random_side_then_the_other_side_then_straight_again() {
    let mut approach = PushApproach::default();
    assert_eq!(approach.approach(), Approach::Direct);
    let mut first_sides = BTreeSet::new();
    for loop_index in 0..40_u32 {
        let tick = 1_000 + loop_index * 997;
        approach.note_failed_push(1, tick);
        let side = approach.approach();
        assert_ne!(side, Approach::Direct);
        first_sides.insert(format!("{side:?}"));
        approach.note_failed_push(1, tick + 300);
        assert_eq!(approach.approach(), side.opposite());
        approach.note_failed_push(1, tick + 600);
        assert_eq!(approach.approach(), Approach::Direct);
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
fn a_push_that_sets_out_again_travels_again_and_may_take_its_side() {
    let mut approach = PushApproach {
        lane_abandoned: true,
        phase: PushPhase::Final,
        phase_since: Some(10),
        staging: Some((1, 1)),
        closest_to_staging: Some(5),
        last_progress_tick: Some(10),
        ..PushApproach::default()
    };
    approach.start_push();
    assert_eq!(approach.phase(), PushPhase::Travel);
    assert!(!approach.lane_abandoned);
    assert_eq!(approach.staging, None);
}

const TS: f32 = 32.0;

fn at(x: f32, y: f32) -> (f32, f32) {
    (x * TS, y * TS)
}

fn leg_observation(tick: u32) -> AiObservation {
    AiObservation {
        player_id: 1,
        tick,
        map: AiMapSummary {
            width: 126,
            height: 126,
            tile_size: TS as u32,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (9, 9),
        players: Vec::new(),
        owned: Vec::new(),
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

/// Target at (80, 80), Jeff's base at (9.5, 9.5); the direct push stages 21.5 tiles out.
fn direct_legs() -> PushLegs {
    PushLegs {
        staging: at(64.8, 64.8),
        attack: at(70.5, 70.5),
        face_from: at(9.5, 9.5),
        side: false,
    }
}

fn orders(
    memory: &mut AiDecisionMemory,
    tick: u32,
    legs: PushLegs,
    center: (f32, f32),
    contact: bool,
) -> LegOrders {
    leg_orders(
        memory,
        &leg_observation(tick),
        legs,
        Some(center),
        contact,
        at(9.5, 9.5),
        at(80.0, 80.0),
    )
}

#[test]
fn a_push_travels_to_its_staging_point_reforms_there_then_closes_in() {
    let mut memory = AiDecisionMemory::default();
    let legs = direct_legs();
    let travel = orders(&mut memory, 100, legs, at(30.0, 30.0), false);
    assert_eq!(travel.phase, PushPhase::Travel);
    assert_eq!(travel.destination, legs.staging);

    let reform = orders(&mut memory, 900, legs, at(62.0, 63.0), false);
    assert_eq!(reform.phase, PushPhase::Reform);
    assert_eq!(reform.destination, legs.staging);

    memory.approach.note_reform(920, false);
    assert_eq!(memory.approach.phase(), PushPhase::Reform);
    memory.approach.note_reform(940, true);
    let final_leg = orders(&mut memory, 950, legs, at(64.0, 64.0), false);
    assert_eq!(final_leg.phase, PushPhase::Final);
    assert_eq!(final_leg.destination, legs.attack);
}

#[test]
fn a_push_that_cannot_reform_closes_in_after_waiting() {
    let mut memory = AiDecisionMemory::default();
    let legs = direct_legs();
    orders(&mut memory, 900, legs, at(63.0, 64.0), false);
    assert_eq!(memory.approach.phase(), PushPhase::Reform);
    memory
        .approach
        .note_reform(900 + REFORM_TIMEOUT_TICKS - 1, false);
    assert_eq!(memory.approach.phase(), PushPhase::Reform);
    memory
        .approach
        .note_reform(900 + REFORM_TIMEOUT_TICKS, false);
    assert_eq!(memory.approach.phase(), PushPhase::Final);
}

#[test]
fn a_stuck_side_push_goes_straight_in_but_fighting_on_the_way_is_not_stuck() {
    let side_legs = PushLegs {
        staging: at(95.0, 60.0),
        attack: at(88.0, 71.0),
        face_from: at(105.0, 50.0),
        side: true,
    };
    // Fighting the whole minute: still on the side lane.
    let mut memory = AiDecisionMemory::default();
    for tick in (0..=NO_PROGRESS_TICKS + 200).step_by(100) {
        orders(&mut memory, tick, side_legs, at(50.0, 30.0), true);
    }
    assert!(!memory.approach.lane_abandoned);
    // Standing still, not fighting: after a minute it gives the side up.
    let mut memory = AiDecisionMemory::default();
    for tick in (0..=NO_PROGRESS_TICKS + 200).step_by(100) {
        orders(&mut memory, tick, side_legs, at(50.0, 30.0), false);
    }
    assert!(memory.approach.lane_abandoned);
    assert_eq!(memory.approach.phase(), PushPhase::Travel);
}

#[test]
fn a_push_already_inside_the_staging_distance_closes_in_from_there() {
    let mut memory = AiDecisionMemory::default();
    let legs = direct_legs();
    // Fought its way in to 14 tiles out on the direct approach.
    let final_leg = orders(&mut memory, 100, legs, at(70.0, 70.0), false);
    assert_eq!(final_leg.phase, PushPhase::Final);
    // Inside the distance but round the far side of the target: not past the staging point.
    let mut memory = AiDecisionMemory::default();
    let travel = orders(&mut memory, 100, legs, at(92.0, 92.0), false);
    assert_eq!(travel.phase, PushPhase::Travel);
}

#[test]
fn a_new_target_starts_the_legs_again() {
    let mut memory = AiDecisionMemory::default();
    let legs = direct_legs();
    orders(&mut memory, 100, legs, at(70.0, 70.0), false);
    assert_eq!(memory.approach.phase(), PushPhase::Final);
    // The natural's steel shifts a tile: same target, same leg.
    let shifted = PushLegs {
        staging: at(65.8, 64.8),
        ..legs
    };
    orders(&mut memory, 200, shifted, at(70.0, 70.0), false);
    assert_eq!(memory.approach.phase(), PushPhase::Final);
    // The natural falls and the main, far beyond, is next: travel to its staging point.
    let main = PushLegs {
        staging: at(95.0, 95.0),
        attack: at(101.0, 101.0),
        ..legs
    };
    let travel = orders(&mut memory, 300, main, at(70.0, 70.0), false);
    assert_eq!(travel.phase, PushPhase::Travel);
    assert_eq!(travel.destination, main.staging);
}

#[test]
fn a_push_in_ranks_is_as_cohesive_as_its_ranks_allow() {
    // Ten Tanks in two ranks heading east, the ranks a little more than their 2 tiles apart.
    let owned: Vec<AiEntitySummary> = (0..10)
        .map(|index| {
            let rank = (index / 6) as f32;
            let file = (index % 6) as f32;
            AiEntitySummary {
                id: index + 1,
                owner: 1,
                kind: EntityKind::Tank,
                x: (40.0 - rank * 2.3) * TS,
                y: (40.0 + file * 1.5) * TS,
                hp: 300,
                state: AiEntityState::Idle,
                is_complete: true,
                production_queue_len: None,
                production_kind: None,
                latched_node: None,
                target_id: None,
                free_for_combat: true,
            }
        })
        .collect();
    let mut observation = leg_observation(100);
    observation.owned = owned;
    let tanks: Vec<u32> = (1..=10).collect();
    assert!(!tank_group_is_cohesive(
        &observation,
        &tanks,
        (1.0, 0.0),
        MarchShape::LEGACY
    ));
    assert!(tank_group_is_cohesive(
        &observation,
        &tanks,
        (1.0, 0.0),
        MarchShape::TIGHT
    ));
    assert!(tank_group_is_cohesive(
        &observation,
        &tanks,
        (1.0, 0.0),
        MarchShape::TRAVEL
    ));
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
