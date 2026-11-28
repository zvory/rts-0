use super::*;
use crate::sdk::actions::AiActionRequest;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Snapshot, StartPayload};
fn fixture(infantry: usize, tanks: usize, enemy_tanks: usize) -> (StartPayload, Snapshot) {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("P{id}"),
        color: "#ffffff".into(),
        is_ai: true,
    });
    let game = Game::new_without_ai_controllers(&players, 7);
    let mut start = game.start_payload();
    let mut s = game.snapshot_for(1);
    start.map.terrain.fill(0);
    start.map.unwalkable_tiles.clear();
    start.map.no_vehicle_tiles.clear();
    start.map.doodads.clear();
    start.map.resources.clear();
    let template = s
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    s.entities.clear();
    for (index, kind) in (0..infantry)
        .map(|i| (i, "rifleman"))
        .chain((0..tanks).map(|i| (100 + i, "tank")))
        .chain(
            (0..enemy_tanks.max(1))
                .map(|i| (900 + i, if enemy_tanks > 0 { "tank" } else { "rifleman" })),
        )
    {
        let mut e = template.clone();
        e.id = index as u32;
        e.owner = if index >= 900 { 2 } else { 1 };
        e.kind = kind.into();
        e.x = if e.owner == 2 { 70.5 * 32. } else { 66.5 * 32. };
        e.y = 60.5 * 32.;
        e.hp = 100;
        e.max_hp = 100;
        e.state = "idle".into();
        e.target_id = None;
        s.entities.push(e);
    }
    (start, s)
}
fn run(
    response: &mut PressureResponse,
    start: &StartPayload,
    s: &Snapshot,
) -> Vec<AiActionRequest> {
    let f = AiFrame::from_host(start, s, 1, [], Some(&[1, 2])).unwrap();
    let eligible = f.owned().iter().map(|e| e.id).collect();
    let area = DefendedArea {
        anchor: (66.5 * 32., 60.5 * 32.),
        radius_tiles: None,
        members: BTreeSet::from([0, 100]),
    };
    let mut a = AiActions::for_frame(&f);
    response.step_defended(&f, &eligible, &[area], &mut a);
    a.into_requests()
}
#[test]
fn distant_reinforcements_do_not_authorize_feeding_and_arrival_releases_attack() {
    let (start, mut s) = fixture(3, 0, 0);
    for e in s.entities.iter_mut().filter(|e| e.owner == 1 && e.id != 0) {
        e.x = 30. * 32.;
    }
    s.entities
        .iter_mut()
        .find(|e| e.owner == 1 && e.id == 0)
        .unwrap()
        .hp = 89;
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    assert!(!orders.iter().any(|a|matches!(a,AiActionRequest::AttackMove{units,..} if units.contains(&0))),"the89%-strength isolated guard still retreats even when a distant equal-strength unit may advance");
    let local = orders
        .iter()
        .find_map(|a| match a {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&0) => Some((*x, *y)),
            _ => None,
        })
        .expect("outnumbered local guard must retreat");
    let f = AiFrame::from_host(&start, &s, 1, [], Some(&[1, 2])).unwrap();
    assert!(
        distance(local, (70.5 * 32., 60.5 * 32.))
            >= weapon_reach(&f, &f.visible_enemies()[0]) + 2. * 32.
    );
    for e in s.entities.iter_mut().filter(|e| e.owner == 1) {
        e.x = 66.5 * 32.;
    }
    s.tick += 9;
    assert!(
        run(&mut response, &start, &s)
            .iter()
            .any(|a| matches!(a,AiActionRequest::AttackMove{units,..} if units.contains(&0))),
        "reinforcement arrival must replace retreat immediately"
    );
}
#[test]
fn tank_response_recruits_two_per_enemy_and_keeps_one_withdrawal_order() {
    let (start, mut s) = fixture(0, 8, 3);
    for e in s
        .entities
        .iter_mut()
        .filter(|e| e.owner == 1 && e.kind == "tank" && e.id != 100)
    {
        e.x = 30. * 32.;
    }
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    assert_eq!(
        response
            .responders
            .iter()
            .filter(|id| (100..108).contains(*id))
            .count(),
        6
    );
    assert!(!orders.iter().any(|a|matches!(a,AiActionRequest::AttackMove{units,..}if units.iter().any(|id|(100..108).contains(id)))));
    let posts = orders
        .iter()
        .filter_map(|a| match a {
            AiActionRequest::Move { units, x, y, .. } => Some((units[0], (*x, *y))),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    for e in &mut s.entities {
        if let Some(p) = posts.get(&e.id) {
            e.x = p.0;
            e.y = p.1;
        }
    }
    s.tick += 9;
    let orders = run(&mut response, &start, &s);
    assert_eq!(orders.iter().filter(|a|matches!(a,AiActionRequest::AttackMove{units,..}if units.iter().any(|id|(100..108).contains(id)))).count(),6);
}

#[test]
fn infantry_raids_leave_tanks_in_reserve() {
    let (start, s) = fixture(10, 8, 0);
    let mut response = PressureResponse::with_cohesive_reserve();
    run(&mut response, &start, &s);
    assert!(response
        .responders
        .iter()
        .all(|id| !(100..108).contains(id)));
}
#[test]
fn low_health_local_value_does_not_count_as_full_strength() {
    let (start, mut s) = fixture(3, 0, 0);
    for e in s.entities.iter_mut().filter(|e| e.owner == 1) {
        e.hp = 10;
    }
    let mut response = PressureResponse::with_cohesive_reserve();
    assert!(!run(&mut response, &start, &s)
        .iter()
        .any(|a| matches!(a, AiActionRequest::AttackMove { .. })));
}

#[test]
fn infantry_and_armor_combine_value_without_a_separate_tank_count_veto() {
    let (start, s) = fixture(18, 1, 2);
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    assert!(
        orders
            .iter()
            .any(|a| matches!(a, AiActionRequest::AttackMove { .. })),
        "combined infantry and armor may fight at90% value even with fewer Tanks"
    );
}
#[test]
fn assembled_force_can_advance_from_beyond_the_contact_radius() {
    let (start, mut s) = fixture(4, 0, 0);
    for e in s.entities.iter_mut().filter(|e| e.owner == 1) {
        e.x = 40. * 32.;
    }
    let mut response = PressureResponse::with_cohesive_reserve();
    assert!(
        run(&mut response, &start, &s)
            .iter()
            .any(|a| matches!(a, AiActionRequest::AttackMove { .. })),
        "a locally ready force must be able to advance from outside fifteen tiles of the target"
    );
}

#[test]
fn outnumbered_defenders_stage_outside_stationary_tank_reach() {
    let (start, s) = fixture(1, 1, 2);
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    let positions = orders
        .iter()
        .filter_map(|a| match a {
            AiActionRequest::Move { units, x, y, .. } => Some((units[0], (*x, *y))),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert!(!positions.is_empty());
    assert!(positions
        .values()
        .all(|p| distance(*p, (70.5 * 32., 60.5 * 32.)) >= 16. * 32.));
}

#[test]
fn armor_regroups_at_separate_two_tile_grid_slots() {
    let (start, s) = fixture(0, 4, 5);
    let mut response = PressureResponse::with_cohesive_reserve();
    let destinations = run(&mut response, &start, &s)
        .into_iter()
        .filter_map(|a| match a {
            AiActionRequest::Move { units, x, y, .. }
                if units.iter().any(|id| (100..104).contains(id)) =>
            {
                Some((units[0], (x, y)))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let points = destinations.values().copied().collect::<Vec<_>>();
    assert_eq!(points.len(), 4);
    for (i, p) in points.iter().enumerate() {
        for q in points.iter().skip(i + 1) {
            assert!(
                distance(*p, *q) >= 2. * 32. - 0.01,
                "regrouping armor needs separate spaced destinations"
            );
        }
    }
}

#[test]
fn infantry_commits_at_ninety_percent_and_retreats_below_it() {
    for (hp, should_fight) in [(89, false), (90, true), (100, true)] {
        let (start, mut s) = fixture(1, 0, 0);
        s.entities.iter_mut().find(|e| e.owner == 1).unwrap().hp = hp;
        let mut response = PressureResponse::with_cohesive_reserve();
        let orders = run(&mut response, &start, &s);
        assert_eq!(
            orders
                .iter()
                .any(|a| matches!(a, AiActionRequest::AttackMove { .. })),
            should_fight,
            "local infantry HP parity{hp}%"
        );
    }
}
#[test]
fn tank_commitment_uses_ninety_percent_parity_instead_of_the_recruitment_target() {
    for (ours, theirs, should_fight) in [(1, 1, true), (9, 10, true), (8, 10, false)] {
        let (start, s) = fixture(0, ours, theirs);
        let mut response = PressureResponse::with_cohesive_reserve();
        let orders = run(&mut response, &start, &s);
        assert_eq!(
            orders
                .iter()
                .any(|a| matches!(a, AiActionRequest::AttackMove { .. })),
            should_fight,
            "local armor{ours}v{theirs}"
        );
    }
}

#[test]
fn infantry_still_recruits_a_fifty_percent_overmatch() {
    let (start, s) = fixture(5, 0, 0);
    let mut response = PressureResponse::with_cohesive_reserve();
    run(&mut response, &start, &s);
    assert_eq!(
        response.responders.len(),
        2,
        "one equal-value enemy still recruits two infantry even though one may now fight"
    );
}

#[test]
fn ten_seen_tanks_do_not_become_one_or_zero_threats_in_fog() {
    let (start, mut s) = fixture(0, 6, 10);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    run(&mut response, &start, &s);
    assert_eq!(response.threat_memory.len(), 10);
    s.entities.retain(|e| e.owner == 1 || e.id == 900);
    s.tick += 9;
    s.entities.iter_mut().find(|e| e.id == 900).unwrap().hp = 50;
    let orders = run(&mut response, &start, &s);
    assert_eq!(
        response.threat_memory.len(),
        10,
        "visible sightings update, rather than duplicate, remembered units"
    );
    assert_eq!(response.threat_memory[&900].1.health.current, 50);
    assert!(!orders
        .iter()
        .any(|r| matches!(r,AiActionRequest::AttackMove{units,..}if units.contains(&100))));
    assert_eq!(response.fighting.get(&100), Some(&false));
    s.entities.retain(|e| e.owner == 1);
    s.tick += 90;
    let orders = run(&mut response, &start, &s);
    assert_eq!(response.threat_memory.len(), 10);
    assert_eq!(response.fighting.get(&100), Some(&false));
    let target = (70.5 * 32., 60.5 * 32.);
    let point = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&100) => Some((*x, *y)),
            _ => None,
        })
        .last()
        .unwrap();
    assert!(
        distance(point, target) >= 16. * 32.,
        "unseen stationary Tanks retain their full firing envelope"
    );
    s.tick += MEMORY_TICKS;
    run(&mut response, &start, &s);
    assert!(response.threat_memory.is_empty());
}
#[test]
fn recon_clears_a_remembered_threat_when_its_whole_firing_area_is_observable() {
    let (start, mut s) = fixture(0, 1, 2);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    run(&mut response, &start, &s);
    assert_eq!(response.threat_memory.len(), 2);
    s.entities.retain(|e| e.owner == 1);
    s.tick += 9;
    run(&mut response, &start, &s);
    assert_eq!(response.threat_memory.len(), 2);
    s.visible_tiles.fill(1);
    s.tick += 9;
    run(&mut response, &start, &s);
    assert!(response.threat_memory.is_empty());
}

#[test]
fn retreat_survives_guard_relevance_loss_and_reentry_waits_for_regrouping() {
    let (start, mut s) = fixture(0, 2, 4);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    let first = run(&mut response, &start, &s);
    let posts = first
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } => Some((units[0], (*x, *y))),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(posts.len(), 2);
    let no_guard = |response: &mut PressureResponse, s: &Snapshot| {
        let f = AiFrame::from_host(&start, s, 1, [], Some(&[1, 2])).unwrap();
        let eligible = f.owned().iter().map(|e| e.id).collect();
        let mut a = AiActions::for_frame(&f);
        let plan = response.step_defended(&f, &eligible, &[], &mut a);
        (plan, a.into_requests())
    };
    s.tick += 9;
    let (plan, orders) = no_guard(&mut response, &s);
    assert_eq!(
        plan.active,
        BTreeSet::from([100, 101]),
        "guard relevance changes must not release a retreating group to macro"
    );
    assert!(!orders
        .iter()
        .any(|r| matches!(r, AiActionRequest::AttackMove { .. })));
    // Visible witnessed destruction changes the balance, but does not cancel regrouping.
    for e in s.entities.iter_mut().filter(|e| (900..903).contains(&e.id)) {
        e.hp = 0;
    }
    let p = posts[&100];
    let e = s.entities.iter_mut().find(|e| e.id == 100).unwrap();
    e.x = p.0;
    e.y = p.1;
    s.tick += 9;
    let (_, orders) = no_guard(&mut response, &s);
    assert!(
        !orders
            .iter()
            .any(|r| matches!(r, AiActionRequest::AttackMove { .. })),
        "one of two regrouped is not90%"
    );
    let p = posts[&101];
    let e = s.entities.iter_mut().find(|e| e.id == 101).unwrap();
    e.x = p.0;
    e.y = p.1;
    s.tick += 9;
    let (_, orders) = no_guard(&mut response, &s);
    assert_eq!(
        orders
            .iter()
            .filter(|r| matches!(r, AiActionRequest::AttackMove { .. }))
            .count(),
        2
    );
    // Fresh overwhelming reinforcements can abort an attack immediately.
    let enemy = s.entities.iter().find(|e| e.id == 903).unwrap().clone();
    for id in 950..955 {
        let mut e = enemy.clone();
        e.id = id;
        s.entities.push(e);
    }
    s.tick += 9;
    let (_, orders) = no_guard(&mut response, &s);
    assert!(!orders
        .iter()
        .any(|r| matches!(r, AiActionRequest::AttackMove { .. })));
    assert!(response.fighting.values().all(|ready| !*ready));
}
#[test]
fn nine_of_ten_regrouped_tanks_can_reenter_without_a_forward_straggler_flipping_them_back() {
    let (start, mut s) = fixture(0, 10, 10);
    s.visible_tiles.fill(0);
    for e in s.entities.iter_mut().filter(|e| e.owner == 1) {
        e.hp = 80;
    }
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    let posts = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } => Some((units[0], (*x, *y))),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(posts.len(), 10);
    for e in s.entities.iter_mut().filter(|e| e.owner == 1) {
        e.hp = 100;
        if e.id != 109 {
            let p = posts[&e.id];
            e.x = p.0;
            e.y = p.1;
        }
    }
    s.tick += 9;
    let orders = run(&mut response, &start, &s);
    assert_eq!(
        orders
            .iter()
            .filter(|r| matches!(r, AiActionRequest::AttackMove { .. }))
            .count(),
        10
    );
    s.tick += 9;
    run(&mut response, &start, &s);
    assert!(response.fighting.values().all(|ready| *ready));
}

#[test]
fn wounded_micro_can_borrow_the_whole_group_without_forgetting_its_retreat() {
    let (start, mut s) = fixture(0, 2, 4);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    run(&mut response, &start, &s);
    s.tick += 9;
    let f = AiFrame::from_host(&start, &s, 1, [], Some(&[1, 2])).unwrap();
    let mut a = AiActions::for_frame(&f);
    let plan = response.step_defended(&f, &BTreeSet::new(), &[], &mut a);
    assert!(plan.active.is_empty());
    assert!(a.is_empty());
    assert!(response.armor_encounter.is_some());
    s.tick += 9;
    let f = AiFrame::from_host(&start, &s, 1, [], Some(&[1, 2])).unwrap();
    let eligible = f.owned().iter().map(|e| e.id).collect();
    let mut a = AiActions::for_frame(&f);
    let plan = response.step_defended(&f, &eligible, &[], &mut a);
    assert_eq!(plan.active, BTreeSet::from([100, 101]));
    assert!(response.fighting.values().all(|ready| !*ready));
}

#[test]
fn threat_expiry_does_not_cancel_an_unfinished_withdrawal() {
    let (start, mut s) = fixture(0, 2, 4);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    let posts = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } => Some((units[0], (*x, *y))),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    s.entities.retain(|e| e.owner == 1);
    s.tick += MEMORY_TICKS;
    run(&mut response, &start, &s);
    assert!(response.threat_memory.is_empty());
    assert!(response.has_withdrawal());
    assert_eq!(response.responders, BTreeSet::from([100, 101]));
    for e in &mut s.entities {
        let p = posts[&e.id];
        e.x = p.0;
        e.y = p.1;
    }
    s.tick += 9;
    run(&mut response, &start, &s);
    assert!(!response.has_withdrawal());
    assert!(response.responders.is_empty());
}

#[test]
fn withdrawal_replans_a_post_blocked_by_new_construction() {
    let (start, mut s) = fixture(0, 2, 4);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    let old = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&100) => Some((*x, *y)),
            _ => None,
        })
        .last()
        .unwrap();
    let mut building = s.entities.iter().find(|e| e.id == 100).unwrap().clone();
    building.id = 2000;
    building.kind = "barracks".into();
    building.x = old.0;
    building.y = old.1;
    s.entities.push(building);
    s.tick += 9;
    let orders = run(&mut response, &start, &s);
    let new = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&100) => Some((*x, *y)),
            _ => None,
        })
        .last()
        .unwrap();
    assert_ne!(new, old);
    let f = AiFrame::from_host(&start, &s, 1, [], Some(&[1, 2])).unwrap();
    let q = WorldQueries::new(&f);
    let tank = f.owned().iter().find(|e| e.id == 100).unwrap();
    assert!(q
        .ground_route(
            q.world_point(tank.position.0, tank.position.1).unwrap(),
            q.world_point(new.0, new.1).unwrap(),
            EntityKind::Tank
        )
        .is_some());
}
#[test]
fn withdrawal_revalidates_a_route_when_the_endpoint_is_still_safe() {
    let (start, mut s) = fixture(0, 2, 4);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    let old = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&100) => Some((*x, *y)),
            _ => None,
        })
        .last()
        .unwrap();
    // Both endpoints are outside the new Rifleman's range, but the old straight route
    // crosses its firing envelope after our Tanks have already reached safe ground.
    for e in s.entities.iter_mut().filter(|e| e.owner == 1) {
        e.x = old.0 - 512.;
        e.y = old.1;
    }
    let mut enemy = s.entities.iter().find(|e| e.id == 900).unwrap().clone();
    enemy.id = 950;
    enemy.kind = "rifleman".into();
    let reach = (rts_rules::combat::attack_profile(EntityKind::Rifleman).range_tiles + 2.) * 32.;
    enemy.x = old.0 - 256.;
    enemy.y = old.1 + reach - 16.;
    s.entities.push(enemy);
    let e = s.entities.iter().find(|e| e.id == 950).unwrap();
    assert!(distance(old, (e.x, e.y)) >= reach);
    s.tick += 9;
    let orders = run(&mut response, &start, &s);
    let new = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&100) => Some((*x, *y)),
            _ => None,
        })
        .last()
        .unwrap();
    assert_ne!(
        new, old,
        "a safe endpoint cannot justify retaining an unsafe route"
    );
}

#[test]
fn an_advancing_encounter_tracks_new_sightings_without_extending_its_pursuit_bound() {
    let (start, mut s) = fixture(0, 2, 1);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    run(&mut response, &start, &s);
    let e = s.entities.iter_mut().find(|e| e.id == 900).unwrap();
    e.x += 3. * 32.;
    let current = (e.x, e.y);
    s.tick += 9;
    let orders = run(&mut response, &start, &s);
    assert_eq!(
        orders
            .iter()
            .filter(|r| matches!(r,AiActionRequest::AttackMove{x,y,..}if (*x,*y)==current))
            .count(),
        2,
        "fresh positions must replace the old attack destination"
    );
    // A moving target does not keep extending an encounter indefinitely.
    s.entities.iter_mut().find(|e| e.id == 900).unwrap().x += 20. * 32.;
    s.tick += 9;
    let orders = run(&mut response, &start, &s);
    assert!(!orders
        .iter()
        .any(|r| matches!(r, AiActionRequest::AttackMove { .. })));
    assert!(response.responders.is_empty());
}

#[test]
fn simulator_executes_the_checked_withdrawal_around_a_wall_and_known_firing_area() {
    use rts_sim::game::command::SimCommand;
    use rts_sim::game::lab::{LabOp, LabOpOutcome, LabSpawnEntity};
    use rts_sim::protocol::Command;
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("P{id}"),
        color: "#ffffff".into(),
        is_ai: true,
    });
    let mut map =
        rts_sim::game::map::Map::load_for_players("Classic", &[(1, 1), (2, 2)], 7).unwrap();
    map.terrain.fill(0);
    map.elevation.fill(0);
    map.doodads.clear();
    map.concealment_tiles.clear();
    map.no_vehicle_tiles.clear();
    map.no_building_tiles.clear();
    map.no_entrenchment_tiles.clear();
    map.damage_reduction_tiles.clear();
    map.slow_movement_tiles.clear();
    map.unwalkable_tiles.clear();
    map.base_sites.clear();
    for y in 58..63 {
        map.unwalkable_tiles.push((45, y));
    }
    let metadata = rts_sim::game::map::Map::metadata_for_name("Classic").unwrap();
    let mut game = Game::new_lab(&players, 7, map, metadata);
    let initial = game
        .snapshot_full_for(1)
        .entities
        .iter()
        .map(|e| e.id)
        .collect();
    game.apply_lab_op(LabOp::DeleteEntities(initial)).unwrap();
    let mut own = BTreeSet::new();
    let rifle_reach = rts_rules::combat::attack_profile(EntityKind::Rifleman).range_tiles * 32.;
    let rifle = (1456., 1936. + rifle_reach + 64. - 16.);
    for (owner, kind, p) in [
        (1, EntityKind::Tank, (1200., 1936.)),
        (1, EntityKind::Tank, (1200., 1872.)),
        (2, EntityKind::Tank, (2256., 1936.)),
        (2, EntityKind::Tank, (2320., 1936.)),
        (2, EntityKind::Tank, (2256., 2000.)),
        (2, EntityKind::Tank, (2320., 2000.)),
        (2, EntityKind::Rifleman, rifle),
    ] {
        let result = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner,
                kind,
                x: p.0,
                y: p.1,
                completed: true,
                dao: None,
                dao_master: false,
                faction_id: None,
            }))
            .unwrap();
        if let LabOpOutcome::Spawned { entity_id } = result {
            if owner == 1 {
                own.insert(entity_id);
            }
        }
    }
    assert_eq!(own.len(), 2);
    let start = game.start_payload();
    let mut response = PressureResponse::with_cohesive_reserve();
    let mut destinations = BTreeMap::new();
    let mut queued = false;
    let initial_hp = game
        .snapshot_full_for(1)
        .entities
        .iter()
        .filter(|e| own.contains(&e.id))
        .map(|e| (e.id, e.hp))
        .collect::<BTreeMap<_, _>>();
    for _ in 0..1200 {
        if game.current_tick() % 9 == 0 {
            // Deliberately disclose this tiny scene: this test verifies executed movement,
            // while separate observation tests verify fog filtering and memory.
            let mut s = game.snapshot_full_for(1);
            s.visible_tiles.fill(1);
            let f = AiFrame::from_host(&start, &s, 1, [], Some(&[1, 2])).unwrap();
            let area = DefendedArea {
                anchor: (2128., 1936.),
                radius_tiles: None,
                members: own.clone(),
            };
            let mut a = AiActions::for_frame(&f);
            response.step_defended(&f, &own, &[area], &mut a);
            for action in a.into_requests() {
                match action {
                    AiActionRequest::Move {
                        units,
                        x,
                        y,
                        queued: is_queued,
                    } => {
                        queued |= is_queued;
                        for id in &units {
                            destinations.insert(*id, (x, y));
                        }
                        game.enqueue(
                            1,
                            SimCommand::from_protocol(Command::Move {
                                units,
                                x,
                                y,
                                queued: is_queued,
                            }),
                        );
                    }
                    AiActionRequest::HoldPosition { units, queued } => {
                        game.enqueue(
                            1,
                            SimCommand::from_protocol(Command::HoldPosition { units, queued }),
                        );
                    }
                    AiActionRequest::AttackMove { .. } => {
                        panic!("the two-versus-four group must keep withdrawing")
                    }
                    _ => {}
                }
            }
        }
        game.tick();
        let s = game.snapshot_full_for(1);
        let tanks = s
            .entities
            .iter()
            .filter(|e| own.contains(&e.id))
            .collect::<Vec<_>>();
        assert_eq!(tanks.len(), 2);
        for e in &tanks {
            assert_eq!(
                e.hp, initial_hp[&e.id],
                "executed retreat entered enemy fire at tick{} ({},{})",
                s.tick, e.x, e.y
            );
            assert!(
                distance((e.x, e.y), rifle) > rifle_reach,
                "executed path crossed the Rifleman's firing area"
            );
        }
        if destinations.len() == 2
            && tanks
                .iter()
                .all(|e| distance((e.x, e.y), destinations[&e.id]) <= 64.)
        {
            assert!(
                queued,
                "the checked detour must be preserved as queued waypoints"
            );
            assert!(
                s.tick > 10,
                "arrival must be produced by native movement, not manual relocation"
            );
            return;
        }
    }
    panic!("native Tanks did not finish the queued safe withdrawal");
}

#[test]
fn disconnected_tank_cannot_hold_a_cleared_encounter_forever() {
    let (mut start, mut s) = fixture(0, 2, 4);
    s.visible_tiles.fill(0);
    for y in 0..start.map.height {
        start
            .map
            .unwalkable_tiles
            .push(rts_sim::protocol::MapTile { x: 30, y });
    }
    s.entities.iter_mut().find(|e| e.id == 101).unwrap().x = 10.5 * 32.;
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    let post = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&100) => Some((*x, *y)),
            _ => None,
        })
        .last()
        .unwrap();
    // Losing sight alone must preserve ownership even for the disconnected member.
    s.entities.retain(|e| e.owner == 1);
    s.tick += 9;
    run(&mut response, &start, &s);
    assert_eq!(response.armor_encounter.as_ref().unwrap().members.len(), 2);
    s.tick += MEMORY_TICKS;
    run(&mut response, &start, &s);
    assert!(
        response.has_withdrawal(),
        "reachable Tank still has to finish retreating"
    );
    assert!(!response.responders.contains(&101));
    let tank = s.entities.iter_mut().find(|e| e.id == 100).unwrap();
    tank.x = post.0;
    tank.y = post.1;
    s.tick += 9;
    run(&mut response, &start, &s);
    assert!(response.armor_encounter.is_none());
    assert!(response.responders.is_empty());
}

#[test]
#[ignore = "Known defect: a blocked rally is treated as disconnected terrain; fixing AI behavior requires separate authorization"]
fn blocked_rally_must_not_release_an_unfinished_withdrawal() {
    let (start, mut s) = fixture(0, 2, 4);
    s.visible_tiles.fill(0);
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    let post = orders
        .iter()
        .filter_map(|r| match r {
            AiActionRequest::Move { units, x, y, .. } if units.contains(&100) => Some((*x, *y)),
            _ => None,
        })
        .last()
        .unwrap();
    // The force has not reached its posts. Construction invalidates the old rally,
    // but does not partition this open map or disconnect either surviving Tank.
    s.entities.retain(|e| e.owner == 1);
    let mut building = s.entities[0].clone();
    building.id = 2000;
    building.kind = "barracks".into();
    building.x = post.0;
    building.y = post.1;
    s.entities.push(building);
    s.tick += MEMORY_TICKS;
    run(&mut response, &start, &s);
    assert!(response.has_withdrawal(), "blocking the old rally must replan the unfinished withdrawal, not release every member as disconnected");
}
