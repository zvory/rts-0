use super::*;
use crate::ai_core::actions::{AiActionContext, SpendBudget};
use crate::ai_core::facts::AiFacts;
use crate::ai_core::observation::{AiEconomy, AiMapSummary, AiPlayerSummary};
use crate::ai_core::profiles::{JEFFS_AI, JEFFS_AI_PRE_THIRD_PRESSURE_ID};
use rts_sim::game::command::SimCommand as Command;

fn ts() -> f32 {
    config::TILE_SIZE as f32
}

fn unit(id: u32, owner: u32, kind: EntityKind, tiles: (f32, f32)) -> AiEntitySummary {
    let hp = config::unit_stats(kind)
        .map(|stats| stats.hp)
        .or_else(|| config::building_stats(kind).map(|stats| stats.hp))
        .unwrap_or(100);
    AiEntitySummary {
        id,
        owner,
        kind,
        x: tiles.0 * ts(),
        y: tiles.1 * ts(),
        hp,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn steel_cluster(first_id: u32, center: (f32, f32)) -> Vec<AiResourceSummary> {
    [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5)]
        .iter()
        .enumerate()
        .map(|(i, (dx, dy))| AiResourceSummary {
            id: first_id + i as u32,
            kind: EntityKind::Steel,
            x: (center.0 + dx) * ts(),
            y: (center.1 + dy) * ts(),
            remaining: 1_000,
        })
        .collect()
}

/// Jeff at (10, 10) and the enemy at (110, 110) on a 128-tile map. Bases besides the starts: the
/// enemy's natural at (92, 92), its third at (112, 62), and one on Jeff's side at (28, 28). Jeff
/// has four Tanks and six Riflemen at home and a Scout Car.
fn game(tick: u32) -> AiObservation {
    let mut owned = vec![unit(1, 1, EntityKind::ResourceDepot, (10.5, 10.5))];
    owned.extend((10..14).map(|id| unit(id, 1, EntityKind::Tank, (13.0 + (id - 10) as f32, 13.0))));
    owned.extend(
        (20..26).map(|id| unit(id, 1, EntityKind::Rifleman, (16.0, 8.0 + (id - 20) as f32))),
    );
    owned.push(unit(30, 1, EntityKind::ScoutCar, (14.0, 16.0)));
    let mut resources = steel_cluster(100, (92.0, 92.0));
    resources.extend(steel_cluster(110, (112.0, 62.0)));
    resources.extend(steel_cluster(120, (28.0, 28.0)));
    resources.extend(steel_cluster(130, (13.0, 6.0)));
    resources.extend(steel_cluster(140, (113.0, 106.0)));
    AiObservation {
        player_id: 1,
        tick,
        map: AiMapSummary {
            width: 128,
            height: 128,
            tile_size: config::TILE_SIZE,
        },
        economy: AiEconomy {
            steel: 500,
            oil: 200,
            supply_used: 40,
            supply_cap: 100,
        },
        own_start_tile: (10, 10),
        players: vec![
            AiPlayerSummary {
                id: 1,
                team_id: 1,
                start_tile: (10, 10),
                is_ai: true,
                is_alive: true,
            },
            AiPlayerSummary {
                id: 2,
                team_id: 2,
                start_tile: (110, 110),
                is_ai: true,
                is_alive: true,
            },
        ],
        owned,
        resources,
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

fn step(
    profile_id: &str,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) -> Vec<Command> {
    let facts = AiFacts::from_observation(observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    let mut intents = Vec::new();
    plan_live(
        profile_id,
        memory,
        (&mut actions, observation, None, &mut intents),
    );
    actions.into_commands()
}

#[test]
fn the_target_is_the_enemy_third_not_its_natural_or_a_base_on_our_side() {
    let observation = game(10_000);
    let target = target_site(&observation, &ThirdPressure::default()).expect("a target");
    let third = (112.0 * ts(), 62.0 * ts());
    assert!(
        distance(target.center, third) < 2.0 * ts(),
        "{:?}",
        (target.center.0 / ts(), target.center.1 / ts())
    );
    assert_eq!(target.patches.len(), 4);
    assert_eq!(target.depot, None);
    // A Depot seen mining it, even outside its ring of patches, is taken exactly.
    let mut state = ThirdPressure::default();
    state.seen_depots.push(rounded((121.0 * ts(), 58.0 * ts())));
    let target = target_site(&observation, &state).unwrap();
    assert_eq!(
        target.depot.map(rounded),
        Some(rounded((121.0 * ts(), 58.0 * ts())))
    );
    // Once found empty, it is skipped for the next base...
    let mut empty = ThirdPressure::default();
    empty.checked.push(rounded(target.center));
    let next = target_site(&observation, &empty);
    assert!(next.is_none(), "{next:?}");
    // ...unless a Depot has been seen there since.
    state.checked.push(rounded(target.center));
    assert!(target_site(&observation, &state).is_some());
}

#[test]
fn the_party_leaves_from_half_past_five_with_army_tanks_riflemen_and_the_scout() {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    assert!(step(JEFFS_AI_ID, &game(9_000), &mut memory).is_empty());
    assert_eq!(memory.third_pressure.phase, Phase::Idle);

    let commands = step(JEFFS_AI_ID, &game(10_000), &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::March);
    assert_eq!(state.tanks.len(), 2);
    assert_eq!(state.riflemen.len(), 5);
    assert_eq!(state.scout, Some(30));
    assert!(commands.iter().any(
        |command| matches!(command, Command::Move { units, .. } if units.contains(&state.tanks[0]))
    ));
    // The rest of Jeff leaves the party alone.
    let reserved = memory.squad_reserved();
    assert!(state.reserved().all(|id| reserved.contains(&id)));
}

#[test]
fn the_party_never_takes_the_home_reserve() {
    // Three Tanks: two would leave the main one short of its two-Tank reserve.
    let mut observation = game(10_000);
    observation.owned.retain(|unit| unit.id != 13);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
    // With a third base of its own Jeff stays home too.
    let mut observation = game(10_000);
    observation.owned.extend([
        unit(2, 1, EntityKind::ResourceDepot, (30.5, 30.5)),
        unit(3, 1, EntityKind::ResourceDepot, (60.5, 20.5)),
    ]);
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
    // The freeze never presses.
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_PRE_THIRD_PRESSURE_ID, &game(10_000), &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
}

#[test]
fn siege_posts_stay_outside_the_defense_zone_within_reach_of_the_depot() {
    let mut observation = game(12_000);
    let site = (112.0 * ts(), 62.0 * ts());
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (112.0, 62.0)));
    for (i, (dx, dy)) in [(-4.0, 0.0), (4.0, 0.0), (0.0, -4.0), (0.0, 5.0)]
        .iter()
        .enumerate()
    {
        observation.visible_enemies.push(unit(
            510 + i as u32,
            2,
            EntityKind::SteelMine,
            (112.0 + dx, 62.0 + dy),
        ));
    }
    let keep = keep_out(&observation, site, true);
    let mut taken = Vec::new();
    for _ in 0..2 {
        let post = post_near(
            &observation,
            None,
            site,
            &TANK_POST_TILES,
            (-1.0, 0.0),
            &keep,
            &taken,
        )
        .expect("a Tank post");
        let from_depot = distance(post, site) / ts();
        assert!((13.0..=15.0).contains(&from_depot), "{from_depot}");
        // On the party's side: west of the Depot, the way it came in.
        assert!(post.0 < site.0 - 5.0 * ts(), "{post:?}");
        assert!(keep.iter().all(|(at, r)| distance(post, *at) >= *r));
        assert!(observation
            .visible_enemies
            .iter()
            .all(|enemy| distance(post, (enemy.x, enemy.y)) > 6.0 * ts()));
        taken.push(post);
    }
    assert!(distance(taken[0], taken[1]) >= POST_SEPARATION_TILES * ts());
}

#[test]
fn parked_tanks_shoot_the_builder_then_the_depot() {
    let mut observation = game(12_000);
    let site = (112.0 * ts(), 62.0 * ts());
    observation.visible_enemies.extend([
        unit(500, 2, EntityKind::ResourceDepot, (112.0, 62.0)),
        unit(510, 2, EntityKind::SteelMine, (108.0, 62.0)),
        unit(520, 2, EntityKind::Worker, (104.0, 64.0)),
    ]);
    let tank = unit(10, 1, EntityKind::Tank, (100.0, 62.0));
    assert_eq!(fire_target(&observation, &tank, site), Some(520));
    observation.visible_enemies.retain(|enemy| enemy.id != 520);
    assert_eq!(fire_target(&observation, &tank, site), Some(500));
    // Out of reach: nothing.
    let far = unit(11, 1, EntityKind::Tank, (95.0, 62.0));
    assert_eq!(fire_target(&observation, &far, site), Some(510));
    let farther = unit(12, 1, EntityKind::Tank, (90.0, 62.0));
    assert_eq!(fire_target(&observation, &farther, site), None);
}

#[test]
fn a_replacement_scout_car_is_queued_while_the_party_has_the_old_one() {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let mut priorities = vec![EntityKind::Tank, EntityKind::Rifleman, EntityKind::ScoutCar];
    let mut caps = vec![(EntityKind::ScoutCar, 1)];
    memory
        .third_pressure
        .queue_scout(&mut priorities, &mut caps);
    assert_eq!(priorities[0], EntityKind::Tank);
    assert_eq!(caps, vec![(EntityKind::ScoutCar, 1)]);
    memory.third_pressure.scout = Some(30);
    memory
        .third_pressure
        .queue_scout(&mut priorities, &mut caps);
    assert_eq!(
        priorities,
        vec![EntityKind::ScoutCar, EntityKind::Tank, EntityKind::Rifleman]
    );
    assert_eq!(caps, vec![(EntityKind::ScoutCar, 2)]);
}

/// The party's members moved to `at` in tiles.
fn party_at(observation: &mut AiObservation, memory: &AiDecisionMemory, at: (f32, f32)) {
    let members: Vec<u32> = memory.third_pressure.reserved().collect();
    for (i, unit) in observation
        .owned
        .iter_mut()
        .filter(|unit| members.contains(&unit.id))
        .enumerate()
    {
        unit.x = (at.0 + i as f32 * 0.8) * ts();
        unit.y = at.1 * ts();
    }
}

#[test]
fn the_party_turns_home_against_as_many_enemy_tanks() {
    // Still beside home it disbands, back in the hands of home defense.
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
    let mut observation = game(10_090);
    observation.visible_enemies.extend([
        unit(600, 2, EntityKind::Tank, (20.0, 13.0)),
        unit(601, 2, EntityKind::Tank, (20.0, 15.0)),
    ]);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
    assert!(memory.squad_reserved().is_empty());
    // Far out it walks home, and hands each member back on arrival.
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    let mut observation = game(11_000);
    party_at(&mut observation, &memory, (70.0, 40.0));
    observation.visible_enemies.extend([
        unit(600, 2, EntityKind::Tank, (76.0, 40.0)),
        unit(601, 2, EntityKind::Tank, (76.0, 42.0)),
    ]);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Return);
    let observation = game(11_090);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
    assert!(memory.squad_reserved().is_empty());
}

#[test]
fn the_natural_forward_tank_stays_out_of_the_party() {
    // Tank 10 is the natural's forward Tank: restaged every decision, any order sharing it is
    // dropped by the live adapter.
    let mut observation = game(10_000);
    observation
        .owned
        .push(unit(14, 1, EntityKind::Tank, (17.0, 13.0)));
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    let forward = expansion_security::surplus_tank_for_forward_base(&observation, &memory);
    assert_eq!(forward, Some(10));
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
    assert!(!memory.third_pressure.tanks.contains(&10));
    // Nor does the forward pick take a party Tank afterwards.
    let forward = expansion_security::surplus_tank_for_forward_base(&observation, &memory);
    assert!(forward.is_some_and(|id| !memory.third_pressure.is_reserved(id)));
}

#[test]
fn a_parked_tank_keeps_its_post_while_a_worker_walks_up() {
    let mut observation = game(12_000);
    let site = (112.0 * ts(), 62.0 * ts());
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (112.0, 62.0)));
    let mut state = ThirdPressure {
        tanks: vec![10, 11],
        riflemen: vec![20, 21],
        target: Some(rounded(site)),
        route: vec![rounded((90.0 * ts(), 62.0 * ts()))],
        ..ThirdPressure::default()
    };
    let posts = assign_posts(&observation, None, &mut state, site).expect("posts");
    let first = posts.iter().find(|(id, _)| *id == 10).unwrap().1;
    observation.visible_enemies.push(unit(
        520,
        2,
        EntityKind::Worker,
        (first.0 / ts() + 2.0, first.1 / ts()),
    ));
    let again = assign_posts(&observation, None, &mut state, site).expect("posts");
    assert_eq!(
        rounded(again.iter().find(|(id, _)| *id == 10).unwrap().1),
        rounded(first)
    );
    assert_eq!(again.len(), 4);
}

#[test]
fn a_losing_party_still_fires_the_volley_that_finishes_the_depot() {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    let site = (112.0 * ts(), 62.0 * ts());
    let tanks = memory.third_pressure.tanks.clone();
    let state = &mut memory.third_pressure;
    state.phase = Phase::Siege;
    state.phase_since = 12_000;
    state.target = Some(rounded(site));
    state.route = vec![rounded((90.0 * ts(), 62.0 * ts()))];
    let mut observation = game(12_400);
    party_at(&mut observation, &memory, (98.5, 62.0));
    let mut depot = unit(500, 2, EntityKind::ResourceDepot, (112.0, 62.0));
    depot.hp = 100;
    observation.visible_enemies.push(depot);
    // Machine Gunners walking up outnumber the party.
    observation.visible_enemies.extend((0..8).map(|i| {
        unit(
            600 + i,
            2,
            EntityKind::MachineGunner,
            (104.0, 56.0 + i as f32),
        )
    }));
    for id in &tanks {
        memory.third_pressure.parked_since.insert(*id, 12_000);
    }
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Siege);
    // With the Depot sound, the same fight sends it home.
    let mut observation = observation.clone();
    observation.visible_enemies[0].hp = 225;
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Return);
}

#[test]
fn a_base_whose_patches_spread_out_is_one_site() {
    // The natural's patches in two groups six tiles apart: one site, so the target is still the
    // third and never the natural's far half.
    let mut observation = game(10_000);
    observation
        .resources
        .extend(steel_cluster(150, (97.0, 86.0)));
    let bases = enemy_bases(&observation, &ThirdPressure::default());
    assert_eq!(bases.len(), 3, "{bases:?}");
    let target = target_site(&observation, &ThirdPressure::default()).unwrap();
    assert!(distance(target.center, (112.0 * ts(), 62.0 * ts())) < 4.0 * ts());
}

/// A party at the third's staging point (west of it), about to look for its Depot.
fn locating(observation: &mut AiObservation) -> AiDecisionMemory {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    party_at(observation, &memory, (92.0, 62.0));
    let state = &mut memory.third_pressure;
    state.route = vec![rounded((92.0 * ts(), 62.0 * ts()))];
    state.leg = 1;
    memory
}

#[test]
fn the_scout_car_looks_for_the_depot_while_the_rest_wait() {
    let mut observation = game(11_000);
    let mut memory = locating(&mut observation);
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Locate);
    assert_eq!(state.locator, vec![30]);
    assert!(!state.unseen.is_empty());
    // Every spot that could hold the Depot mines a patch of the third.
    let area: Vec<(f32, f32)> = state.area.iter().map(|p| unrounded(*p)).collect();
    assert!(state
        .unseen
        .iter()
        .all(|spot| mines_from(&observation, unrounded(*spot), &area)));
    let scout_moved = commands
        .iter()
        .any(|command| matches!(command, Command::Move { units, .. } if units == &vec![30]));
    assert!(scout_moved, "{commands:?}");
    assert!(!commands.iter().any(|command| {
        matches!(command, Command::Move { units, x, .. }
            if units.contains(&state.tanks[0]) && *x > 95.0 * ts())
    }));
}

#[test]
fn a_depot_outside_the_ring_of_patches_is_found_and_shelled_where_it_stands() {
    let mut observation = game(11_000);
    let mut memory = locating(&mut observation);
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Locate);
    // The Depot stands 9 tiles east of the patches' centre, beyond the ring.
    let mut observation = game(11_009);
    party_at(&mut observation, &memory, (92.0, 62.0));
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    step(JEFFS_AI_ID, &observation, &mut memory);
    // First the Scout Car sizes it up from outside, on the staging side, the rest still waiting.
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Locate);
    assert!(state.survey_since.is_some());
    assert_eq!(state.depot, Some(500));
    assert_eq!(state.target, Some(rounded((121.0 * ts(), 62.0 * ts()))));
    let mut observation = observation.clone();
    observation.tick = 11_018;
    let scout = observation.owned.iter_mut().find(|u| u.id == 30).unwrap();
    scout.x = 109.0 * ts();
    scout.y = 62.0 * ts();
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Siege);
    assert_eq!(state.depot, Some(500));
    assert_eq!(state.target, Some(rounded((121.0 * ts(), 62.0 * ts()))));
    let tank_post = state
        .posts
        .get(&state.tanks[0])
        .copied()
        .map(unrounded)
        .unwrap();
    let tank = unit(
        1,
        1,
        EntityKind::Tank,
        (tank_post.0 / ts(), tank_post.1 / ts()),
    );
    assert!(in_reach(
        &observation,
        &tank,
        &observation.visible_enemies[0]
    ));
}

#[test]
fn a_base_with_no_depot_on_any_spot_sends_the_party_to_join_the_push() {
    let mut observation = game(11_000);
    let mut memory = locating(&mut observation);
    step(JEFFS_AI_ID, &observation, &mut memory);
    // The Scout Car has seen every spot but one; standing on it, it sees that one too.
    let last = *memory.third_pressure.unseen.first().unwrap();
    memory.third_pressure.unseen = vec![last];
    let mut observation = game(11_009);
    party_at(&mut observation, &memory, (92.0, 62.0));
    let scout = observation.owned.iter_mut().find(|u| u.id == 30).unwrap();
    scout.x = last.0 as f32;
    scout.y = last.1 as f32;
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Merge);
    assert!(state
        .checked
        .iter()
        .any(|site| distance(unrounded(*site), (112.0 * ts(), 62.0 * ts())) < 2.0 * ts()));
}

#[test]
fn spotters_walk_up_past_patches_with_no_extractor_on_them() {
    // On The River every spot near the Depot on the party's side lay within 7 tiles of a Steel
    // patch, so the spotters had no post, nobody saw the Depot, and the Tanks sat out of reach.
    let mut observation = game(12_000);
    let site = (112.0 * ts(), 62.0 * ts());
    for (i, at) in [(104.0, 62.0), (105.0, 58.0), (105.0, 66.0)]
        .iter()
        .enumerate()
    {
        observation.resources.push(AiResourceSummary {
            id: 190 + i as u32,
            kind: EntityKind::Steel,
            x: at.0 * ts(),
            y: at.1 * ts(),
            remaining: 1_000,
        });
    }
    let mut state = ThirdPressure {
        tanks: vec![10, 11],
        riflemen: vec![20, 21, 22],
        target: Some(rounded(site)),
        route: vec![rounded((90.0 * ts(), 62.0 * ts()))],
        ..ThirdPressure::default()
    };
    let posts = assign_posts(&observation, None, &mut state, site).expect("posts");
    for id in [20, 21, 22] {
        let post = posts.iter().find(|(member, _)| *member == id).unwrap().1;
        assert!(distance(post, site) <= 10.5 * ts(), "{id}: {post:?}");
    }
    for id in [10, 11] {
        let post = posts.iter().find(|(member, _)| *member == id).unwrap().1;
        let tank = unit(id, 1, EntityKind::Tank, (post.0 / ts(), post.1 / ts()));
        let depot = unit(500, 2, EntityKind::ResourceDepot, (112.0, 62.0));
        assert!(in_reach(&observation, &tank, &depot), "{id}: {post:?}");
    }
}

/// The test game with a fourth enemy-side base north-west of the third, ranked below it.
fn with_fourth_base(observation: &mut AiObservation) {
    observation
        .resources
        .extend(steel_cluster(160, (100.0, 40.0)));
}

#[test]
fn a_seen_depot_beats_a_better_ranked_guess() {
    let mut observation = game(10_000);
    with_fourth_base(&mut observation);
    let guess = target_site(&observation, &ThirdPressure::default()).unwrap();
    assert!(distance(guess.center, (112.0 * ts(), 62.0 * ts())) < 2.0 * ts());
    let mut state = ThirdPressure::default();
    state.seen_depots.push(rounded((101.0 * ts(), 43.0 * ts())));
    let seen = target_site(&observation, &state).unwrap();
    assert!(distance(seen.center, (100.0 * ts(), 40.0 * ts())) < 2.0 * ts());
    assert_eq!(
        seen.depot.map(rounded),
        Some(rounded((101.0 * ts(), 43.0 * ts())))
    );
}

#[test]
fn an_empty_third_hands_the_party_to_a_depot_seen_nearby() {
    let mut observation = game(11_000);
    let mut memory = locating(&mut observation);
    step(JEFFS_AI_ID, &observation, &mut memory);
    let last = *memory.third_pressure.unseen.first().unwrap();
    memory.third_pressure.unseen = vec![last];
    memory
        .third_pressure
        .seen_depots
        .push(rounded((101.0 * ts(), 43.0 * ts())));
    let mut observation = game(11_009);
    with_fourth_base(&mut observation);
    party_at(&mut observation, &memory, (92.0, 62.0));
    let scout = observation.owned.iter_mut().find(|u| u.id == 30).unwrap();
    scout.x = last.0 as f32;
    scout.y = last.1 as f32;
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::March);
    assert_eq!(state.empty_bases, 1);
    assert_eq!(state.target, Some(rounded((101.0 * ts(), 43.0 * ts()))));
    let area: Vec<(f32, f32)> = state.area.iter().map(|p| unrounded(*p)).collect();
    assert!(mines_from(&observation, (101.0 * ts(), 43.0 * ts()), &area));
    assert!(!state.route.is_empty());
}

#[test]
fn a_garrison_the_party_would_lose_to_sends_it_home_and_spares_the_base() {
    let mut observation = game(11_000);
    let mut memory = locating(&mut observation);
    step(JEFFS_AI_ID, &observation, &mut memory);
    let mut observation = game(11_009);
    party_at(&mut observation, &memory, (92.0, 62.0));
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    // Three Tanks and six Machine Gunners behind the Depot, out of the Scout Car's threat range.
    observation
        .visible_enemies
        .extend((0..3).map(|i| unit(600 + i, 2, EntityKind::Tank, (130.0, 58.0 + 3.0 * i as f32))));
    observation.visible_enemies.extend((0..6).map(|i| {
        unit(
            610 + i,
            2,
            EntityKind::MachineGunner,
            (128.0, 57.0 + 2.0 * i as f32),
        )
    }));
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Locate);
    let mut observation = observation.clone();
    observation.tick = 11_018;
    let scout = observation.owned.iter_mut().find(|u| u.id == 30).unwrap();
    scout.x = 109.0 * ts();
    scout.y = 62.0 * ts();
    step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Return);
    assert!(state
        .defended
        .iter()
        .any(|(site, _)| distance(unrounded(*site), (112.0 * ts(), 62.0 * ts())) < 2.0 * ts()));
    // The next launch leaves the defended third alone a while.
    let next = target_site(&observation, state);
    assert!(next.is_none_or(|base| distance(base.center, (112.0 * ts(), 62.0 * ts())) > 8.0 * ts()));
    // A garrison it beats does not stop the siege.
    let mut observation = game(11_009);
    let mut memory = locating(&mut observation);
    step(JEFFS_AI_ID, &observation, &mut memory);
    party_at(&mut observation, &memory, (92.0, 62.0));
    observation
        .visible_enemies
        .push(unit(500, 2, EntityKind::ResourceDepot, (121.0, 62.0)));
    observation
        .visible_enemies
        .push(unit(620, 2, EntityKind::Rifleman, (130.0, 62.0)));
    step(JEFFS_AI_ID, &observation, &mut memory);
    let mut observation = observation.clone();
    observation.tick = 11_018;
    let scout = observation.owned.iter_mut().find(|u| u.id == 30).unwrap();
    scout.x = 109.0 * ts();
    scout.y = 62.0 * ts();
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Siege);
}

#[test]
fn only_the_scout_car_falls_back_from_contact_at_the_base() {
    let mut observation = game(11_000);
    let mut memory = locating(&mut observation);
    step(JEFFS_AI_ID, &observation, &mut memory);
    let mut observation = game(11_009);
    party_at(&mut observation, &memory, (92.0, 62.0));
    let scout = observation.owned.iter_mut().find(|u| u.id == 30).unwrap();
    scout.x = 104.0 * ts();
    scout.y = 62.0 * ts();
    // Two Riflemen near the Scout Car only: the party would beat them.
    observation.visible_enemies.extend([
        unit(700, 2, EntityKind::Rifleman, (113.0, 66.0)),
        unit(701, 2, EntityKind::Rifleman, (113.0, 67.0)),
    ]);
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    let state = &memory.third_pressure;
    assert_eq!(state.phase, Phase::Locate);
    assert!(state.fallback_until > 11_009);
    assert!(
        commands.iter().any(|command| {
            matches!(command, Command::Move { units, x, .. }
            if units.contains(&30) && (*x - 92.0 * ts()).abs() < 1.0)
        }),
        "{commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::AttackMove { .. })),
        "{commands:?}"
    );
}

#[test]
fn a_search_out_of_time_with_most_spots_seen_found_the_base_empty() {
    for (share_unseen, empty) in [(10, true), (50, false)] {
        let mut observation = game(11_000);
        let mut memory = locating(&mut observation);
        step(JEFFS_AI_ID, &observation, &mut memory);
        let state = &mut memory.third_pressure;
        let total = state.spots_total;
        assert!(total > 20, "{total}");
        state.unseen.truncate(total * share_unseen / 100);
        let observation = {
            let mut later = game(11_000 + LOCATE_TIMEOUT_TICKS + 9);
            party_at(&mut later, &memory, (92.0, 62.0));
            later
        };
        step(JEFFS_AI_ID, &observation, &mut memory);
        let state = &memory.third_pressure;
        assert_eq!(state.phase, Phase::Merge, "{share_unseen}% unseen");
        assert_eq!(
            state.empty_bases,
            u32::from(empty),
            "{share_unseen}% unseen"
        );
    }
}

#[test]
fn after_two_empty_bases_only_a_seen_depot_sends_the_party_out() {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    memory.third_pressure.empty_bases = 2;
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
    memory
        .third_pressure
        .seen_depots
        .push(rounded((114.0 * ts(), 60.0 * ts())));
    step(JEFFS_AI_ID, &game(10_009), &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
    assert_eq!(
        memory.third_pressure.target,
        Some(rounded((114.0 * ts(), 60.0 * ts())))
    );
}

/// A party that found its base empty, at the third's staging point, now on its way to the push.
fn merging(observation: &mut AiObservation) -> AiDecisionMemory {
    let mut memory = locating(observation);
    let state = &mut memory.third_pressure;
    state.phase = Phase::Merge;
    state.phase_since = observation.tick;
    memory
}

#[test]
fn the_partys_tanks_count_toward_the_push_until_it_heads_home() {
    let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
    assert_eq!(memory.third_pressure.pledged_tanks(), 0);
    step(JEFFS_AI_ID, &game(10_000), &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::March);
    assert_eq!(memory.third_pressure.pledged_tanks(), 2);
    assert_eq!(memory.third_pressure.joining_tanks(), 0);
    memory.third_pressure.phase = Phase::Merge;
    assert_eq!(memory.third_pressure.joining_tanks(), 2);
    memory.third_pressure.phase = Phase::Return;
    assert_eq!(memory.third_pressure.pledged_tanks(), 0);
}

#[test]
fn while_the_push_forms_the_party_waits_midway_on_its_way() {
    let mut observation = game(12_000);
    let mut memory = merging(&mut observation);
    let commands = step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Merge);
    // The push forms 8 tiles out of the main toward the enemy; the party heads for the point of
    // its way to the enemy main nearest it, inside the middle stretch.
    let own = (10.5 * ts(), 10.5 * ts());
    let enemy = (110.5 * ts(), 110.5 * ts());
    let tank = memory.third_pressure.tanks[0];
    let (x, y) = commands
        .iter()
        .find_map(|command| match command {
            Command::Move { units, x, y, .. } if units.contains(&tank) => Some((*x, *y)),
            _ => None,
        })
        .expect("the party moves to meet the push");
    let rally = (
        own.0 + 8.0 * ts() / 2f32.sqrt(),
        own.1 + 8.0 * ts() / 2f32.sqrt(),
    );
    let along = distance(rally, (x, y)) / distance(rally, enemy);
    assert!((0.29..=0.71).contains(&along), "{along}");
    let off_line = ((x - rally.0) - (y - rally.1)).abs() / 2f32.sqrt();
    assert!(off_line < 1.0 * ts(), "{off_line}");
}

#[test]
fn the_push_takes_the_party_in_when_it_reaches_it() {
    let mut observation = game(12_000);
    let mut memory = merging(&mut observation);
    // The launched push: two home Tanks marching past the party.
    observation.owned.extend([
        unit(40, 1, EntityKind::Tank, (90.0, 64.0)),
        unit(41, 1, EntityKind::Tank, (91.0, 60.0)),
    ]);
    memory.containment.wave_launched = true;
    memory.containment.active_tanks = [40, 41].into_iter().collect();
    let party_tanks = memory.third_pressure.tanks.clone();
    let party_riflemen = memory.third_pressure.riflemen.clone();
    step(JEFFS_AI_ID, &observation, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Idle);
    assert!(memory.squad_reserved().is_empty());
    assert!(party_tanks
        .iter()
        .all(|id| memory.containment.active_tanks.contains(id)));
    assert!(party_riflemen
        .iter()
        .all(|id| memory.containment.active_riflemen.contains(id)));
    assert_eq!(memory.containment.active_scout, Some(30));
}

#[test]
fn a_party_that_meets_no_push_goes_home() {
    let mut observation = game(12_000);
    let mut memory = merging(&mut observation);
    step(JEFFS_AI_ID, &observation, &mut memory);
    let mut later = game(12_000 + config::TICK_HZ * 151);
    party_at(&mut later, &memory, (92.0, 62.0));
    step(JEFFS_AI_ID, &later, &mut memory);
    assert_eq!(memory.third_pressure.phase, Phase::Return);
}
