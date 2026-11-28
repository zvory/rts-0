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
fn tank_response_recruits_two_per_enemy_but_commits_each_ready_compact_group() {
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
    assert!(
        !orders
            .iter()
            .any(|a| matches!(a,AiActionRequest::AttackMove{units,..} if units.contains(&100))),
        "the isolated Tank still waits for local support"
    );
    assert!(orders.iter().any(|a|matches!(a,AiActionRequest::AttackMove{units,..} if units.iter().any(|id|(101..108).contains(id)))),"the assembled group can advance without waiting for the distant straggler");
    for e in s
        .entities
        .iter_mut()
        .filter(|e| e.owner == 1 && e.kind == "tank")
    {
        e.x = 66.5 * 32.;
    }
    s.tick += 9;
    let orders = run(&mut response, &start, &s);
    assert!(orders
        .iter()
        .any(|a| matches!(a,AiActionRequest::AttackMove{units,..} if units.contains(&100))));
    assert_eq!(
        response
            .fighting
            .iter()
            .filter(|(id, ready)| (100..108).contains(*id) && **ready)
            .count(),
        6
    );
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
    let positions: Vec<_> = orders
        .iter()
        .filter_map(|a| match a {
            AiActionRequest::Move { x, y, .. } => Some((*x, *y)),
            _ => None,
        })
        .collect();
    assert!(!positions.is_empty());
    assert!(positions
        .iter()
        .all(|p| distance(*p, (70.5 * 32., 60.5 * 32.)) >= 16. * 32.));
}

#[test]
fn individual_tank_retreats_keep_separate_two_tile_destinations() {
    let (start, s) = fixture(0, 4, 5);
    let mut response = PressureResponse::with_cohesive_reserve();
    let points: BTreeMap<_, _> = run(&mut response, &start, &s)
        .into_iter()
        .filter_map(|a| match a {
            AiActionRequest::Move { units, x, y, .. }
                if units.iter().any(|id| (100..104).contains(id)) =>
            {
                Some((units[0], (x, y)))
            }
            _ => None,
        })
        .collect();
    assert_eq!(points.len(), 4);
    let points = points.values().copied().collect::<Vec<_>>();
    for (i, p) in points.iter().enumerate() {
        for q in points.iter().skip(i + 1) {
            assert!(
                distance(*p, *q) >= 2. * 32. - 0.01,
                "retreating armor needs separate spaced destinations"
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
fn a_scout_car_alone_does_not_mobilize_tanks() {
    let (start, mut s) = fixture(8, 10, 0);
    s.entities.iter_mut().find(|e| e.owner == 2).unwrap().kind = "scout_car".into();
    let mut response = PressureResponse::with_cohesive_reserve();
    let orders = run(&mut response, &start, &s);
    assert!(!orders.iter().any(|r|matches!(r,AiActionRequest::AttackMove{units,..}|AiActionRequest::Move{units,..} if units.iter().any(|id| (100..110).contains(id)))));
}
#[test]
fn cleared_incident_releases_tanks_without_waiting_for_regrouping() {
    let (start, mut s) = fixture(0, 4, 4);
    let mut response = PressureResponse::with_cohesive_reserve();
    run(&mut response, &start, &s);
    assert!(!response.responders.is_empty());
    s.entities.retain(|e| e.owner == 1);
    s.visible_tiles.fill(1);
    s.tick += 9;
    run(&mut response, &start, &s);
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
