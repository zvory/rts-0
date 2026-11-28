use super::*;
use crate::sdk::actions::AiActionRequest;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{Snapshot, StartPayload};

fn fixture() -> (StartPayload, Snapshot, Point, Point) {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("P{id}"),
        color: "#ffffff".into(),
        is_ai: true,
    });
    let game = Game::new_without_ai_controllers(&players, 7);
    let start = game.start_payload();
    let mut snapshot = game.snapshot_for(1);
    let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    let positions = frame
        .players()
        .iter()
        .map(|p| {
            (
                (p.start_tile.0 as f32 + 0.5) * frame.map().tile_size as f32,
                (p.start_tile.1 as f32 + 0.5) * frame.map().tile_size as f32,
            )
        })
        .collect::<Vec<_>>();
    let midpoint = (
        (positions[0].0 + positions[1].0) * 0.5,
        (positions[0].1 + positions[1].1) * 0.5,
    );
    let length = distance(positions[0], positions[1]);
    let direction = (
        (positions[1].0 - positions[0].0) / length,
        (positions[1].1 - positions[0].1) / length,
    );
    let rifle = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    snapshot
        .entities
        .retain(|e| e.owner == 1 && e.kind != "rifleman");
    for id in 1000..1010 {
        let mut own = rifle.clone();
        own.id = id;
        own.x = midpoint.0;
        own.y = midpoint.1;
        snapshot.entities.push(own);
    }
    let mut enemy = rifle;
    enemy.id = 999;
    enemy.owner = 2;
    enemy.x = midpoint.0;
    enemy.y = midpoint.1;
    enemy.target_id = None;
    snapshot.entities.push(enemy);
    (start, snapshot, midpoint, direction)
}
fn area(anchor: Point, ids: impl IntoIterator<Item = u32>) -> DefendedArea {
    DefendedArea {
        anchor,
        radius_tiles: None,
        members: ids.into_iter().collect(),
    }
}
fn enemy_at(snapshot: &mut Snapshot, anchor: Point, axis: Point, tiles: f32, size: f32) {
    let enemy = snapshot.entities.iter_mut().find(|e| e.id == 999).unwrap();
    enemy.x = anchor.0 + axis.0 * tiles * size;
    enemy.y = anchor.1 + axis.1 * tiles * size;
}
fn run(
    response: &mut PressureResponse,
    start: &StartPayload,
    snapshot: &Snapshot,
    areas: &[DefendedArea],
    excluded: &BTreeSet<u32>,
) -> (ResponsePlan, Vec<AiActionRequest>) {
    let frame = AiFrame::from_host(start, snapshot, 1, [], Some(&[1, 2])).unwrap();
    let eligible = frame
        .owned()
        .iter()
        .filter(|e| !excluded.contains(&e.id))
        .map(|e| e.id)
        .collect();
    let mut actions = AiActions::for_frame(&frame);
    let result = response.step_defended(&frame, &eligible, areas, &mut actions);
    (result, actions.into_requests())
}
#[test]
fn weapon_reach_admits_cross_midline_shooters_and_keeps_relevant_crossings() {
    let (start, mut snapshot, midpoint, axis) = fixture();
    let size = start.map.tile_size as f32;
    let areas = [area(midpoint, [1000])];
    enemy_at(&mut snapshot, midpoint, axis, -2., size);
    let mut response = PressureResponse::with_machine_gunners();
    let (first, _) = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new());
    assert!(!first.active.is_empty());
    snapshot.tick += 9;
    enemy_at(&mut snapshot, midpoint, axis, 2., size);
    let (crossed, _) = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new());
    assert_eq!(
        first.active, crossed.active,
        "midline crossing cannot release a relevant threat"
    );
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .kind = "artillery".into();
    enemy_at(&mut snapshot, midpoint, axis, 12., size);
    let mut sniper_response = PressureResponse::with_machine_gunners();
    let (sniper, orders) = run(
        &mut sniper_response,
        &start,
        &snapshot,
        &areas,
        &BTreeSet::new(),
    );
    assert!(
        !sniper.active.is_empty(),
        "use the shooter's reach rather than a short fixed bubble"
    );
    assert!(!orders.is_empty());
    let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    let mut pressure = PressureResponse::default();
    let mut actions = AiActions::for_frame(&frame);
    let eligible = frame.owned().iter().map(|e| e.id).collect();
    assert!(
        pressure
            .step(&frame, &eligible, &mut actions)
            .active
            .is_empty(),
        "Pressure's default still ignores enemy-side shooters"
    );
}
#[test]
fn target_membership_distinguishes_defenders_buildings_and_outbound_attackers() {
    let (start, mut snapshot, midpoint, axis) = fixture();
    let size = start.map.tile_size as f32;
    let areas = [area(midpoint, [1000])];
    enemy_at(&mut snapshot, midpoint, axis, 18., size);
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1009);
    let mut response = PressureResponse::with_machine_gunners();
    assert!(
        run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty(),
        "an outbound unit's fight is not a defensive incident"
    );
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1000);
    assert!(
        !run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty(),
        "targeting an assigned guard admits a distant visible shooter"
    );
    let building = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .id;
    let building_areas = [area(midpoint, [building])];
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(building);
    let mut response = PressureResponse::with_machine_gunners();
    assert!(!run(
        &mut response,
        &start,
        &snapshot,
        &building_areas,
        &BTreeSet::new()
    )
    .0
    .active
    .is_empty());
    assert!(
        run(&mut response, &start, &snapshot, &[], &BTreeSet::new())
            .0
            .active
            .is_empty(),
        "release must use the same current protected-role relevance as admission"
    );
}
#[test]
fn original_post_leash_releases_retreat_and_prevents_next_tick_readmission() {
    let (start, mut snapshot, midpoint, axis) = fixture();
    let size = start.map.tile_size as f32;
    let areas = [area(midpoint, [1000])];
    enemy_at(&mut snapshot, midpoint, axis, 4., size);
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1000);
    let mut response = PressureResponse::with_machine_gunners();
    let first = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new()).0;
    assert!(!first.active.is_empty());
    for tiles in [10., 12., 14.] {
        snapshot.tick += 9;
        enemy_at(&mut snapshot, midpoint, axis, tiles, size);
        let (retreated, orders) = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new());
        assert!(retreated.active.is_empty());
        assert!(
            orders.is_empty(),
            "continued targeting cannot extend the original post's leash"
        );
    }
    snapshot.tick += 9;
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = None;
    assert!(
        run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty()
    );
    snapshot.tick += 9;
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1000);
    assert!(
        run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty(),
        "visible target reacquisition must not reset the old chase leash"
    );
    let other_guard_areas = [
        area(midpoint, [1000]),
        area(
            (
                midpoint.0 + axis.0 * 4. * size,
                midpoint.1 + axis.1 * 4. * size,
            ),
            [1001],
        ),
    ];
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1001);
    snapshot.tick += 9;
    assert!(
        run(
            &mut response,
            &start,
            &snapshot,
            &other_guard_areas,
            &BTreeSet::new()
        )
        .0
        .active
        .is_empty(),
        "switching to another distant guard target cannot hop pursuit leashes"
    );
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1000);
    let moved_areas = [area(
        (
            midpoint.0 + axis.0 * 14. * size,
            midpoint.1 + axis.1 * 14. * size,
        ),
        [1000],
    )];
    snapshot.tick += 9;
    assert!(
        run(
            &mut response,
            &start,
            &snapshot,
            &moved_areas,
            &BTreeSet::new()
        )
        .0
        .active
        .is_empty(),
        "moving the defended role cannot drag an existing incident's original anchor"
    );
    snapshot.tick += 9;
    enemy_at(&mut snapshot, midpoint, axis, 4., size);
    assert!(
        !run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty(),
        "returning to the defended post is a valid renewed threat"
    );
}
#[test]
fn fog_memory_is_bounded_and_wounded_eligibility_keeps_command_ownership() {
    let (start, mut snapshot, midpoint, _) = fixture();
    let areas = [area(midpoint, [1000])];
    let mut response = PressureResponse::with_machine_gunners();
    let first = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new()).0;
    let wounded = *first.active.first().unwrap();
    snapshot.tick += 9;
    let (replaced, orders) = run(
        &mut response,
        &start,
        &snapshot,
        &areas,
        &BTreeSet::from([wounded]),
    );
    assert!(
        replaced.active.iter().any(|id| !first.active.contains(id)),
        "replace unavailable purchase value"
    );
    assert!(!orders.iter().any(|order| matches!(order, AiActionRequest::AttackMove { units, .. } if units.contains(&wounded))));
    let last_seen = snapshot.tick;
    snapshot.entities.retain(|e| e.id != 999);
    snapshot.tick = last_seen + 899;
    let remembered = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new()).0;
    assert!(
        !remembered.active.is_empty(),
        "unknown ground preserves last observed threat"
    );
    snapshot.tick = last_seen + 900;
    let expired = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new()).0;
    assert!(expired.active.is_empty());
    assert_eq!(expired.released, remembered.active);
}

#[test]
fn losing_one_incident_immediately_retargets_remaining_defense_during_retry_cooldown() {
    let (start, mut snapshot, midpoint, axis) = fixture();
    let size = start.map.tile_size as f32;
    let areas = [area(midpoint, [1000])];
    enemy_at(&mut snapshot, midpoint, axis, 4., size);
    let mut second = snapshot
        .entities
        .iter()
        .find(|e| e.id == 999)
        .unwrap()
        .clone();
    second.id = 998;
    second.x = midpoint.0 - axis.0 * 4. * size;
    second.y = midpoint.1 - axis.1 * 4. * size;
    snapshot.entities.push(second.clone());
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1000);
    let mut response = PressureResponse::with_machine_gunners();
    assert!(
        !run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty()
    );
    snapshot.tick += 9;
    enemy_at(&mut snapshot, midpoint, axis, 10., size);
    let (remaining, orders) = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new());
    assert!(!remaining.active.is_empty());
    assert!(
        !orders.is_empty(),
        "do not wait ninety ticks while following a released attacker"
    );
    assert!(orders.iter().all(|order| matches!(order,
        AiActionRequest::AttackMove { x,y,.. } if (*x,*y) == (second.x,second.y))));
}

#[test]
fn observable_empty_ground_releases_without_waiting_for_memory_timeout() {
    let (mut start, mut snapshot, midpoint, axis) = fixture();
    start.map.concealment_tiles.clear();
    let areas = [area(midpoint, [1000])];
    let mut response = PressureResponse::with_machine_gunners();
    let first = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new()).0;
    assert!(!first.active.is_empty());
    let mut enemy = snapshot
        .entities
        .iter()
        .find(|e| e.id == 999)
        .unwrap()
        .clone();
    snapshot.entities.retain(|e| e.id != 999);
    snapshot.tick += 9;
    snapshot.visible_tiles = vec![1; start.map.width as usize * start.map.height as usize];
    let cleared = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new()).0;
    assert!(cleared.active.is_empty());
    assert_eq!(cleared.released, first.active);
    let size = start.map.tile_size as f32;
    enemy.x = midpoint.0 + axis.0 * 20. * size;
    enemy.y = midpoint.1 + axis.1 * 20. * size;
    enemy.target_id = Some(1000);
    snapshot.entities.push(enemy);
    snapshot.tick += 9;
    assert!(
        run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty(),
        "a far reappearance after cleared ground cannot reset pursuit memory"
    );
}

#[test]
fn retreat_can_start_new_incident_at_a_different_stationary_protected_base() {
    let (start, mut snapshot, midpoint, axis) = fixture();
    let size = start.map.tile_size as f32;
    let new_anchor = (
        midpoint.0 + axis.0 * 16. * size,
        midpoint.1 + axis.1 * 16. * size,
    );
    let building = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .id;
    let areas = [area(midpoint, [1000]), area(new_anchor, [building])];
    enemy_at(&mut snapshot, midpoint, axis, 4., size);
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(1000);
    let mut response = PressureResponse::with_machine_gunners();
    assert!(
        !run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty()
    );
    snapshot.tick += 9;
    enemy_at(&mut snapshot, midpoint, axis, 16., size);
    snapshot
        .entities
        .iter_mut()
        .find(|e| e.id == 999)
        .unwrap()
        .target_id = Some(building);
    let (new_incident, orders) = run(&mut response, &start, &snapshot, &areas, &BTreeSet::new());
    assert!(
        !new_incident.active.is_empty(),
        "a different protected base must receive its own incident"
    );
    assert!(
        !orders.is_empty(),
        "a genuinely new incident retargets immediately during retry cooldown"
    );
    let anchored = response.defended.incidents.get(&999).unwrap();
    assert_eq!(anchored.anchor, new_anchor);
    assert!(anchored.members.contains(&building));
    snapshot.tick += 9;
    enemy_at(&mut snapshot, midpoint, axis, 26., size);
    assert!(
        run(&mut response, &start, &snapshot, &areas, &BTreeSet::new())
            .0
            .active
            .is_empty(),
        "the new incident also has a fixed pursuit leash"
    );
}

#[test]
fn building_presence_bubble_includes_twenty_tiles_and_releases_beyond_it() {
    let (start, mut snapshot, midpoint, axis) = fixture();
    let size = start.map.tile_size as f32;
    let building = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .id;
    let mut protected = area(midpoint, [building]);
    protected.radius_tiles = Some(20.);
    let mut response = PressureResponse::with_machine_gunners();
    for (tiles, expected) in [(19., true), (20., true), (20.1, false)] {
        enemy_at(&mut snapshot, midpoint, axis, tiles, size);
        snapshot.tick += 9;
        let result = run(
            &mut response,
            &start,
            &snapshot,
            &[protected.clone()],
            &BTreeSet::new(),
        )
        .0;
        assert_eq!(
            !result.active.is_empty(),
            expected,
            "building reaction at{tiles}tiles"
        );
    }
}
#[test]
fn building_bubble_accepts_noncombatants_and_enemy_construction() {
    for kind in ["worker", "barracks"] {
        let (start, mut snapshot, midpoint, axis) = fixture();
        snapshot
            .entities
            .iter_mut()
            .find(|e| e.id == 999)
            .unwrap()
            .kind = kind.into();
        enemy_at(
            &mut snapshot,
            midpoint,
            axis,
            19.,
            start.map.tile_size as f32,
        );
        let mut protected = area(midpoint, [1]);
        protected.radius_tiles = Some(20.);
        let mut response = PressureResponse::with_machine_gunners();
        assert!(
            !run(
                &mut response,
                &start,
                &snapshot,
                &[protected],
                &BTreeSet::new()
            )
            .0
            .active
            .is_empty(),
            "react to any visible{kind}behind the line"
        );
    }
}
#[test]
fn a_short_guard_incident_hands_off_to_the_building_bubble_at_the_same_anchor() {
    let (start, mut snapshot, midpoint, axis) = fixture();
    let size = start.map.tile_size as f32;
    let building = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "resource_depot")
        .unwrap()
        .id;
    let guard = area(midpoint, [1000]);
    let mut protected = area(midpoint, [building]);
    protected.radius_tiles = Some(20.);
    let mut response = PressureResponse::with_machine_gunners();
    enemy_at(&mut snapshot, midpoint, axis, 4., size);
    assert!(!run(
        &mut response,
        &start,
        &snapshot,
        &[guard.clone(), protected.clone()],
        &BTreeSet::new()
    )
    .0
    .active
    .is_empty());
    assert!(response
        .defended
        .incidents
        .get(&999)
        .unwrap()
        .members
        .contains(&1000));
    snapshot.tick += 9;
    enemy_at(&mut snapshot, midpoint, axis, 19., size);
    assert!(
        !run(
            &mut response,
            &start,
            &snapshot,
            &[guard, protected],
            &BTreeSet::new()
        )
        .0
        .active
        .is_empty(),
        "crossing behind a staffed line remains a threat inside the nearby building bubble"
    );
    assert!(response
        .defended
        .incidents
        .get(&999)
        .unwrap()
        .members
        .contains(&building));
}

#[test]
fn wounded_enemy_recruits_less_and_memory_keeps_last_observed_health_value() {
    let (start, mut snapshot, midpoint, _) = fixture();
    let areas = [area(midpoint, [1000])];
    let mut full = PressureResponse::with_machine_gunners();
    let (full_plan, _) = run(&mut full, &start, &snapshot, &areas, &BTreeSet::new());
    let enemy = snapshot.entities.iter_mut().find(|e| e.id == 999).unwrap();
    enemy.hp = enemy.max_hp / 4;
    let mut wounded = PressureResponse::with_machine_gunners();
    let (wounded_plan, _) = run(&mut wounded, &start, &snapshot, &areas, &BTreeSet::new());
    assert!(wounded_plan.active.len() < full_plan.active.len());
    let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    let mut defended = DefendedResponse::default();
    let book = AiRulebook::for_frame(&frame).unwrap();
    let contacts = defended.update(&frame, book, &areas);
    let observed = contacts[&999].value;
    assert!(observed > 0 && observed < value(book, &frame.visible_enemies()[0]));
    snapshot.entities.retain(|e| e.id != 999);
    snapshot.tick += 9;
    let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    defended.update(&frame, book, &areas);
    assert_eq!(defended.incidents[&999].value, observed);
}
