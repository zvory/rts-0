use super::*;
use crate::ai_core::observation::{AiEconomy, AiResourceSummary};
use crate::ai_core::profiles::JEFFS_AI;
use rts_sim::game::command::SimCommand;

const TS: f32 = config::TILE_SIZE as f32;

fn entity(id: u32, owner: u32, kind: EntityKind, tile: (f32, f32)) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner,
        kind,
        x: tile.0 * TS,
        y: tile.1 * TS,
        hp: 100,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn enemy_base() -> EnemyBaseFact {
    EnemyBaseFact {
        player_id: 2,
        start_tile: (56, 56),
        x: 56.5 * TS,
        y: 56.5 * TS,
    }
}

/// A 64x64 map with Jeff's HQ at (8, 8), its steel just in front of it and the enemy at (56, 56).
fn home_observation() -> AiObservation {
    let steel = (0..6)
        .map(|index| AiResourceSummary {
            id: 100 + index,
            kind: EntityKind::Steel,
            x: (12.5 + index as f32 * 0.5) * TS,
            y: (9.5 + index as f32 * 0.5) * TS,
            remaining: 500,
        })
        .collect();
    AiObservation {
        player_id: 1,
        tick: 1000,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (8, 8),
        players: Vec::new(),
        owned: vec![entity(1, 1, EntityKind::ResourceDepot, (8.5, 8.5))],
        resources: steel,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn main_line(observation: &AiObservation) -> (f32, f32) {
    let steel = defense::main_steel_cluster_center(observation).unwrap();
    defense::main_steel_line_center(observation, steel, enemy_base(), 8.0).unwrap()
}

fn post_of(observation: &AiObservation, memory: &mut AiDecisionMemory) -> HomePost {
    update_home_post(observation, memory, None, Some(enemy_base()), 8.0, true);
    memory.home_post.unwrap()
}

fn near(left: (f32, f32), right: (f32, f32)) -> bool {
    dist2(left.0, left.1, right.0, right.1) <= squared(1.0)
}

#[test]
fn the_home_post_is_the_mains_line_until_the_natural_stands_close_enough() {
    let mut observation = home_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let line = main_line(&observation);
    let post = post_of(&observation, &mut memory);
    assert!(post.on_main_line);
    assert!(near(post.center(), line));

    // A finished natural within reach: the post moves halfway toward it.
    let natural = (14.5, 26.5);
    observation
        .owned
        .push(entity(2, 1, EntityKind::ResourceDepot, natural));
    let post = post_of(&observation, &mut memory);
    assert!(!post.on_main_line);
    let midpoint = (
        (line.0 + natural.0 * TS) * 0.5,
        (line.1 + natural.1 * TS) * 0.5,
    );
    assert!(
        near(post.center(), midpoint),
        "{:?} vs {midpoint:?}",
        post.center()
    );

    // One still being built does not count yet.
    observation.owned[1].is_complete = false;
    assert!(post_of(&observation, &mut memory).on_main_line);

    // A natural too far away for one post to cover both: the post stays on the main's line.
    observation.owned[1] = entity(2, 1, EntityKind::ResourceDepot, (8.5, 50.5));
    let post = post_of(&observation, &mut memory);
    assert!(post.on_main_line);
    assert!(near(post.center(), line));
}

#[test]
fn the_home_post_outlives_the_mains_steel() {
    let mut observation = home_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let line = main_line(&observation);
    assert!(post_of(&observation, &mut memory).on_main_line);

    // Mined out: the line used to vanish and the home Tanks got no more orders.
    observation.resources.clear();
    assert!(defense::main_steel_cluster_center(&observation).is_none());
    let post = post_of(&observation, &mut memory);
    assert!(
        !post.on_main_line,
        "no live steel: the post's own line is used"
    );
    assert!(near(post.center(), line));
}

#[test]
fn resting_tanks_with_nothing_to_do_gather_on_the_home_post() {
    let mut observation = home_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    observation
        .owned
        .push(entity(2, 1, EntityKind::ResourceDepot, (40.5, 40.5)));
    let post = post_of(&observation, &mut memory);
    let at_post = (post.center().0 / TS, post.center().1 / TS);
    observation.owned.extend([
        // Resting in a corner of the main.
        entity(10, 1, EntityKind::Tank, (6.0, 6.0)),
        // Covering the other base.
        entity(11, 1, EntityKind::Tank, (42.0, 40.0)),
        // Left out on the map.
        entity(12, 1, EntityKind::Tank, (30.0, 52.0)),
        // Already home.
        entity(13, 1, EntityKind::Tank, (at_post.0 + 1.0, at_post.1)),
        // Has another job.
        entity(14, 1, EntityKind::Tank, (5.0, 10.0)),
        // Still on the way somewhere.
        entity(15, 1, EntityKind::Tank, (4.0, 12.0)),
    ]);
    let excluded = BTreeSet::from([14]);
    let facts = AiFacts::from_observation(&observation);
    let gather = |observation: &AiObservation, memory: &mut AiDecisionMemory| {
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let sent = gather_resting_tanks(
            &mut actions,
            observation,
            memory,
            &excluded,
            &BTreeSet::new(),
            &[],
        );
        (sent, actions.into_commands())
    };

    // The first sighting gives no evidence that anything stands still.
    let (sent, _) = gather(&observation, &mut memory);
    assert!(sent.is_empty());

    observation
        .owned
        .iter_mut()
        .find(|unit| unit.id == 15)
        .unwrap()
        .x += 1.0 * TS;
    let (mut sent, commands) = gather(&observation, &mut memory);
    sent.sort_unstable();
    assert_eq!(sent, vec![10, 12]);
    assert!(commands.iter().any(|command| matches!(
        command,
        SimCommand::AttackMove { units, .. } if units.contains(&10)
    )));

    // Sent once; not again straight away. The moving Tank is still left alone.
    observation
        .owned
        .iter_mut()
        .find(|unit| unit.id == 15)
        .unwrap()
        .x += 1.0 * TS;
    let (sent, _) = gather(&observation, &mut memory);
    assert!(sent.is_empty(), "{sent:?}");
}

fn siege_observation() -> AiObservation {
    let mut observation = home_observation();
    observation.owned.extend([
        entity(2, 1, EntityKind::ResourceDepot, (8.5, 28.5)),
        entity(20, 1, EntityKind::Tank, (10.0, 20.0)),
        entity(21, 1, EntityKind::Tank, (11.0, 20.0)),
        entity(30, 1, EntityKind::Rifleman, (9.0, 12.0)),
    ]);
    // Ten tiles east of the natural: out of the 6-tile zone, well inside a Tank's reach.
    observation
        .visible_enemies
        .push(entity(700, 2, EntityKind::Tank, (18.5, 28.5)));
    observation
}

#[test]
fn a_tank_shelling_a_base_from_outside_the_zone_is_an_attacker() {
    let mut observation = siege_observation();
    assert!(defense::local_defense_contact(&observation).is_none());
    let siege = defense::tank_siege_contact(&observation).unwrap();
    assert_eq!(siege.target_ids, vec![700]);

    // Out of reach of every building: not a siege.
    observation.visible_enemies[0] = entity(700, 2, EntityKind::Tank, (30.0, 50.0));
    assert!(defense::tank_siege_contact(&observation).is_none());
}

#[test]
fn home_tanks_answer_a_siege_they_can_match_and_hold_against_one_they_cannot() {
    let mut observation = siege_observation();
    let defenders = [20, 21, 30];
    let facts = AiFacts::from_observation(&observation);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let answered = defense::respond_to_local_incident(
        &mut actions,
        &observation,
        &mut memory,
        &defenders,
        None,
        true,
        true,
    )
    .unwrap();
    assert!(answered.contains(&20) && answered.contains(&21));

    // Three Tanks against two: nobody goes in, and nobody searches the damaged building either.
    observation.visible_enemies.extend([
        entity(701, 2, EntityKind::Tank, (19.5, 27.5)),
        entity(702, 2, EntityKind::Tank, (19.5, 29.5)),
    ]);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let facts = AiFacts::from_observation(&observation);
    for natural_hp in [100, 80] {
        observation.tick += 9;
        observation
            .owned
            .iter_mut()
            .find(|unit| unit.id == 2)
            .unwrap()
            .hp = natural_hp;
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        assert!(defense::respond_to_local_incident(
            &mut actions,
            &observation,
            &mut memory,
            &defenders,
            None,
            true,
            true,
        )
        .is_none());
        assert!(actions.into_commands().is_empty());
    }
    let siege = defense::tank_siege(&observation, &defenders).unwrap();
    assert!(!siege.matched());
}

#[test]
fn a_small_push_comes_home_and_a_large_one_carries_on() {
    let mut observation = home_observation();
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    post_of(&observation, &mut memory);
    for id in 900..905 {
        observation
            .owned
            .push(entity(id, 1, EntityKind::Tank, (40.0, 40.0)));
    }
    let facts = AiFacts::from_observation(&observation);
    memory.containment.wave_launched = true;
    memory.containment.repush_count = 1;

    memory.containment.active_tanks = (900..905).collect();
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    assert!(frontal::recall_small_push_home(&mut actions, &observation, &mut memory).is_none());
    assert!(!memory.containment.recovery_active);

    memory.containment.active_tanks = (900..903).collect();
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
    let recalled =
        frontal::recall_small_push_home(&mut actions, &observation, &mut memory).unwrap();
    assert_eq!(recalled, vec![900, 901, 902]);
    assert!(memory.containment.recovery_active);
    assert!(memory.containment.active_tanks.is_empty());
    assert_eq!(
        memory.containment.repush_count, 1,
        "a recall is not a failed push"
    );
    let post = memory.home_post.unwrap().center();
    assert!(actions.into_commands().iter().any(|command| matches!(
        command,
        SimCommand::Move { x, y, .. } if near((*x, *y), post)
    )));
}

/// A 64x64 grass map whose analysis marks `forest` tiles as forest (concealment).
fn analysis_with_forest(forest: &[(u32, u32)]) -> AiMapAnalysis {
    use rts_sim::protocol::{terrain, MapInfo, MapTile, PlayerStart, StartPayload};
    let size = 64;
    let tiles = (size * size) as usize;
    let start = StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        match_run_id: None,
        capabilities: Default::default(),
        diagnostics: Default::default(),
        replay: None,
        lab: None,
        observer_view: None,
        tick: 0,
        map: MapInfo {
            width: size,
            height: size,
            tile_size: config::TILE_SIZE,
            elevation: vec![0; tiles],
            sun: None,
            terrain: vec![terrain::GRASS; tiles],
            resources: Vec::new(),
            doodads: Vec::new(),
            concealment_tiles: forest.iter().map(|&(x, y)| MapTile { x, y }).collect(),
            no_vehicle_tiles: Vec::new(),
            no_building_tiles: Vec::new(),
            no_entrenchment_tiles: Vec::new(),
            damage_reduction_tiles: Vec::new(),
            slow_movement_tiles: Vec::new(),
        },
        players: [(1, (8, 8)), (2, (56, 56))]
            .into_iter()
            .map(|(id, (x, y))| PlayerStart {
                id,
                team_id: id,
                faction_id: "kriegsia".to_string(),
                name: format!("P{id}"),
                color: "#111111".to_string(),
                is_ai: true,
                start_tile_x: x,
                start_tile_y: y,
            })
            .collect(),
    };
    AiMapAnalysis::analyze(&start)
}

#[test]
fn the_live_jeffs_home_post_keeps_out_of_forest() {
    let observation = home_observation();
    let line = main_line(&observation);
    let (tx, ty) = ((line.0 / TS) as u32, (line.1 / TS) as u32);
    // Forest over the main's line: tree trunks stop Tanks, which then drive to the nearest spot
    // they can reach, on Crossroads across a water wall from the base.
    let forest: Vec<(u32, u32)> = (tx - 2..=tx + 2)
        .flat_map(|x| (ty - 2..=ty + 2).map(move |y| (x, y)))
        .collect();
    let analysis = analysis_with_forest(&forest);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

    update_home_post(
        &observation,
        &mut memory,
        Some(&analysis),
        Some(enemy_base()),
        8.0,
        false,
    );
    assert!(
        near(memory.home_post.unwrap().center(), line),
        "frozen profiles keep the old post"
    );

    update_home_post(
        &observation,
        &mut memory,
        Some(&analysis),
        Some(enemy_base()),
        8.0,
        true,
    );
    let post = memory.home_post.unwrap().center();
    let (px, py) = ((post.0 / TS) as i32, (post.1 / TS) as i32);
    assert!(
        forest
            .iter()
            .all(|&(x, y)| (x as i32 - px).abs().max((y as i32 - py).abs()) > 1),
        "post at ({px},{py}) stays a tile clear of the forest"
    );
    assert!(dist2(post.0, post.1, line.0, line.1) <= squared(6.0 * TS));
}
