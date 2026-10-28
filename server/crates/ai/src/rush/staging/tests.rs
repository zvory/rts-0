use super::*;
use crate::sdk::actions::AiActionRequest;
use crate::sdk::AiStrategy;
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{EntityView, Snapshot, StartPayload};

fn fixture() -> (StartPayload, Snapshot) {
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("Player {id}"),
        color: "#4cc9f0".into(),
        is_ai: true,
    });
    let game = Game::new_without_ai_controllers(&players, 7);
    (game.start_payload(), game.snapshot_for(1))
}

fn frame(start: &StartPayload, snapshot: &Snapshot) -> AiFrame {
    AiFrame::from_host(start, snapshot, 1, [], Some(&[1, 2])).unwrap()
}

fn step(
    waves: &mut RushWaves,
    start: &StartPayload,
    snapshot: &Snapshot,
    target: Point,
) -> Vec<AiActionRequest> {
    let frame = frame(start, snapshot);
    let mut actions = AiActions::for_frame(&frame);
    waves.step(&frame, target, &mut actions);
    actions.into_requests()
}

fn assemble(snapshot: &mut Snapshot, waves: &RushWaves) {
    for entity in &mut snapshot.entities {
        if let Some(slot) = waves.slots.get(&entity.id) {
            entity.x = slot.0;
            entity.y = slot.1;
            entity.state = "idle".into();
        }
    }
}

fn add_rifle(snapshot: &mut Snapshot, specimen: &EntityView, id: u32) {
    let mut rifle = specimen.clone();
    rifle.id = id;
    snapshot.entities.push(rifle);
}

#[test]
fn waves_wait_for_ninety_percent_then_require_five_arrivals_without_retasking_old_waves() {
    let (start, mut snapshot) = fixture();
    let frame = frame(&start, &snapshot);
    let enemy = frame.players().iter().find(|p| p.id == 2).unwrap();
    let size = frame.map().tile_size as f32;
    let target = (
        (enemy.start_tile.0 as f32 + 0.5) * size,
        (enemy.start_tile.1 as f32 + 0.5) * size,
    );
    let mut waves = RushWaves::default();
    let rally = waves.rally(&frame, 2, target).unwrap();
    assert!((distance(rally, target) / size - 20.0).abs() < 0.001);
    let specimen = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    let initial = step(&mut waves, &start, &snapshot, target);
    assert_eq!(
        initial
            .iter()
            .filter(|action| matches!(action, AiActionRequest::AttackMove { .. }))
            .count(),
        4
    );
    assert!(initial.iter().all(|action| matches!(action,
        AiActionRequest::AttackMove { units, x, y, queued: false }
        if units.len() == 1 && waves.slots.get(&units[0]) == Some(&(*x, *y))
    )));
    assert!(!initial
        .iter()
        .any(|action| matches!(action, AiActionRequest::Move { .. })));
    let line = waves.plan.unwrap().1;
    for slot in waves.slots.values() {
        let along =
            (slot.0 - line.center.0) * line.forward.0 + (slot.1 - line.center.1) * line.forward.1;
        assert!(
            along.abs() < 0.01,
            "opening slots must form a perpendicular line"
        );
    }
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    assert!(!step(&mut waves, &start, &snapshot, target)
        .iter()
        .any(|action| matches!(action, AiActionRequest::AttackMove { .. })));
    let mut centre = EntityView::new(
        999,
        1,
        "training_centre",
        specimen.x,
        specimen.y,
        120,
        120,
        "train",
    );
    centre.prod_upgrade = Some("methamphetamines".into());
    centre.prod_progress = Some(0.899);
    centre.prod_queue = Some(1);
    snapshot.entities.push(centre);
    snapshot.tick += 9;
    assert!(!step(&mut waves, &start, &snapshot, target)
        .iter()
        .any(|action| matches!(action, AiActionRequest::AttackMove { .. })));
    snapshot
        .entities
        .iter_mut()
        .find(|entity| entity.id == 999)
        .unwrap()
        .prod_progress = Some(0.9);
    snapshot.tick += 9;
    let launch = step(&mut waves, &start, &snapshot, target);
    let groups = launch
        .iter()
        .filter_map(|action| match action {
            AiActionRequest::AttackMove { units, x, y, .. } => {
                assert_eq!((*x, *y), target);
                Some(units.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].len(), 4);
    let first_wave = groups[0].clone();
    for id in 1001..=1004 {
        add_rifle(&mut snapshot, &specimen, id);
    }
    snapshot.tick += 9;
    step(&mut waves, &start, &snapshot, target);
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    assert!(!step(&mut waves, &start, &snapshot, target)
        .iter()
        .any(|action| matches!(action, AiActionRequest::AttackMove { .. })));
    add_rifle(&mut snapshot, &specimen, 1005);
    snapshot.tick += 9;
    assert!(
        !step(&mut waves, &start, &snapshot, target)
            .iter()
            .any(|action| matches!(action, AiActionRequest::AttackMove { x, y, .. } if (*x, *y) == target)),
        "an en-route fifth unit does not count"
    );
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    let launch = step(&mut waves, &start, &snapshot, target);
    assert!(
        matches!(launch.as_slice(),[AiActionRequest::AttackMove { units,.. }] if units==&vec![1001,1002,1003,1004,1005])
    );
    assert!(first_wave.iter().all(|id| waves.launched.contains(id)));
    add_rifle(&mut snapshot, &specimen, 1006);
    snapshot.tick += 9;
    let next = step(&mut waves, &start, &snapshot, target);
    assert!(
        matches!(next.as_slice(),[AiActionRequest::AttackMove {units,..}] if units==&vec![1006])
    );
}

#[test]
fn route_and_staging_follow_the_passable_approach_around_a_wall() {
    let map = AiMap {
        width: 96,
        height: 96,
        tile_size: 32,
        terrain: vec![AiTerrain::Grass; 96 * 96],
        no_building_tiles: BTreeSet::new(),
    };
    let mut blocked = map.clone();
    for y in 0..60 {
        blocked.terrain[y * 96 + 48] = AiTerrain::Water;
    }
    let home = (10.5 * 32.0, 10.5 * 32.0);
    let enemy = (75.5 * 32.0, 10.5 * 32.0);
    let path = shortest_route(&blocked, home, enemy).unwrap();
    assert!(path.iter().any(|point| point.1 >= 60.0 * 32.0));
    assert!(path.iter().all(|point| passable_world(&blocked, *point)));
    let line = StagingLine::on_route(&blocked, home, enemy).unwrap();
    assert!((distance(line.center, enemy) / 32.0 - 20.0).abs() < 0.001);
    assert!(
        line.center.1 > enemy.1 + 0.25 * 32.0,
        "staging must use the route around water, not the blocked base diagonal"
    );
    assert!(passable_world(&blocked, line.center));
}

#[test]
fn panzerfausts_stage_on_a_parallel_line_three_tiles_behind_front_infantry() {
    let (start, mut snapshot) = fixture();
    let initial = frame(&start, &snapshot);
    let enemy = initial
        .players()
        .iter()
        .find(|player| player.id == 2)
        .unwrap();
    let size = initial.map().tile_size as f32;
    let target = (
        (enemy.start_tile.0 as f32 + 0.5) * size,
        (enemy.start_tile.1 as f32 + 0.5) * size,
    );
    let mut waves = RushWaves::default();
    waves.rally(&initial, 2, target).unwrap();
    let specimen = snapshot
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == "rifleman")
        .unwrap()
        .clone();
    for id in 1001..=1003 {
        let mut panzer = specimen.clone();
        panzer.id = id;
        panzer.kind = "panzerfaust".into();
        snapshot.entities.push(panzer);
    }
    step(&mut waves, &start, &snapshot, target);
    let line = waves.plan.unwrap().1;
    for entity in snapshot
        .entities
        .iter()
        .filter(|entity| waves.slots.contains_key(&entity.id))
    {
        let point = waves.slots[&entity.id];
        let depth = -((point.0 - line.center.0) * line.forward.0
            + (point.1 - line.center.1) * line.forward.1)
            / size;
        let expected = if entity.kind == "panzerfaust" {
            3.0
        } else {
            0.0
        };
        assert!(
            (depth - expected).abs() < 0.001,
            "{} staged at depth {depth}",
            entity.kind
        );
    }
    assert!((1001..=1003).all(|id| waves.slots.contains_key(&id)));
}

#[test]
fn barracks_rally_and_new_recruits_attack_move_to_staging_before_wave_unlock() {
    let (start, mut snapshot) = fixture();
    snapshot.entities.push(EntityView::new(
        900, 1, "barracks", 432.0, 496.0, 500, 500, "idle",
    ));
    let frame = frame(&start, &snapshot);
    let mut strategy = crate::rush::RushStrategy::default();
    let mut actions = AiActions::for_frame(&frame);
    strategy.step(&frame, &mut actions);
    let requests = actions.into_requests();
    let rally = strategy.waves.plan.unwrap().1.center;
    assert!(requests.iter().any(|action| matches!(action,
        AiActionRequest::SetRally { building: 900, x, y, kind: rts_sim::game::entity::RallyKind::AttackMove, queued: false }
        if (*x, *y) == rally
    )));
    for (id, slot) in &strategy.waves.slots {
        assert!(requests.iter().any(|action| matches!(action,
            AiActionRequest::AttackMove { units, x, y, queued: false }
            if units == &vec![*id] && (*x, *y) == *slot
        )));
    }
    assert!(!strategy.waves.first_wave_launched);
}

#[test]
fn pullback_owns_only_riflemen_approaching_staging_and_resumes_the_slot() {
    let (start, mut snapshot) = fixture();
    let target = (2000.0, 2000.0);
    let mut waves = RushWaves::default();
    waves.rally(&frame(&start, &snapshot), 2, target).unwrap();
    let rifle = snapshot
        .entities
        .iter_mut()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap();
    rifle.hp = 10;
    rifle.x = 320.0;
    rifle.y = 320.0;
    let id = rifle.id;
    let mut enemy = rifle.clone();
    enemy.id = 999;
    enemy.owner = 2;
    enemy.hp = 45;
    enemy.x += 64.0;
    enemy.target_id = Some(id);
    snapshot.entities.push(enemy);
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(orders
        .iter()
        .any(|a| matches!(a, AiActionRequest::Move { units, .. } if units == &[id])));
    assert!(!orders
        .iter()
        .any(|a| matches!(a, AiActionRequest::AttackMove { units, .. } if units.contains(&id))));
    snapshot.tick += 9;
    assert!(!step(&mut waves, &start, &snapshot, target).iter().any(|a| {
        matches!(a, AiActionRequest::AttackMove { units, .. } | AiActionRequest::HoldPosition { units, .. } if units.contains(&id))
    }));
    snapshot.tick += 36;
    snapshot.entities.last_mut().unwrap().target_id = None;
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(orders.iter().any(
        |a| matches!(a, AiActionRequest::AttackMove { units, x, y, queued: false }
        if units == &[id] && (*x, *y) == waves.slots[&id])
    ));
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    let post = waves.slots[&id];
    let enemy = snapshot.entities.last_mut().unwrap();
    enemy.x = post.0 + 64.0;
    enemy.y = post.1;
    enemy.target_id = Some(id);
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(waves.staged.contains(&id));
    assert_eq!(waves.pullback, PullbackMemory::default());
    assert!(!orders
        .iter()
        .any(|a| matches!(a, AiActionRequest::Move { units, .. } if units.contains(&id))));
    snapshot.entities.retain(|entity| entity.id != 999);
    waves.first_wave_unlocked = true;
    snapshot.tick += 9;
    step(&mut waves, &start, &snapshot, target);
    assert!(waves.launched.contains(&id));
    assert_eq!(waves.pullback, PullbackMemory::default());
}

#[test]
fn handoff_is_armed_by_the_fifth_successful_wave_and_later_waves_do_not_delay_it() {
    let (start, mut snapshot) = fixture();
    snapshot.upgrades.push("methamphetamines".into());
    let target = (2000.0, 2000.0);
    let mut waves = RushWaves::default();
    waves.rally(&frame(&start, &snapshot), 2, target).unwrap();
    let specimen = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    snapshot.entities.retain(|e| e.kind != "rifleman");
    // A very late match clock cannot finish an opening that has not launched its waves.
    snapshot.tick = 20000;
    assert_eq!(waves.handoff_tick(), None);
    let mut scheduled = None;
    for wave in 1..=HANDOFF_WAVES + 1 {
        for i in 0..5 {
            add_rifle(&mut snapshot, &specimen, 1000 + wave * 5 + i);
        }
        step(&mut waves, &start, &snapshot, target);
        assemble(&mut snapshot, &waves);
        snapshot.tick += 9;
        let actions = step(&mut waves, &start, &snapshot, target);
        assert!(actions.iter().any(
            |a| matches!(a, AiActionRequest::AttackMove { units, x, y, queued: false }
            if units.len() == 5 && (*x, *y) == target)
        ));
        assert_eq!(waves.waves_launched, wave);
        if wave < HANDOFF_WAVES {
            assert_eq!(waves.handoff_tick(), None);
        } else if wave == HANDOFF_WAVES {
            scheduled = Some(snapshot.tick + HANDOFF_DELAY_TICKS);
            assert_eq!(waves.handoff_tick(), scheduled);
        } else {
            assert_eq!(waves.handoff_tick(), scheduled);
        }
        snapshot.tick += 9;
    }
}

#[test]
fn losing_the_engineer_arms_a_fixed_recovery_delay_and_suspends_new_waves() {
    let mut strategy = crate::rush::RushStrategy::default();
    strategy.observe_engineer(20_000, true);
    assert!(!strategy.handoff_due(u32::MAX));
    strategy.observe_engineer(20_010, false);
    strategy.observe_engineer(20_020, false);
    assert!(strategy.waves.launches_suspended);
    assert!(!strategy.handoff_due(20_010 + HANDOFF_DELAY_TICKS - 1));
    assert!(strategy.handoff_due(20_010 + HANDOFF_DELAY_TICKS));
}

#[test]
fn recovery_grace_keeps_a_ready_cohort_from_launching_just_before_handoff() {
    let (start, mut snapshot) = fixture();
    snapshot.upgrades.push("methamphetamines".into());
    let target = (2000.0, 2000.0);
    let mut waves = RushWaves::default();
    waves.rally(&frame(&start, &snapshot), 2, target).unwrap();
    waves.suspend_launches();
    step(&mut waves, &start, &snapshot, target);
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    step(&mut waves, &start, &snapshot, target);
    assert_eq!(waves.waves_launched, 0);
    assert!(!waves.first_wave_launched);
    assert_eq!(waves.handoff_tick(), None);
}

#[test]
fn staging_defense_supports_the_cohort_then_regroups_without_retasking_other_units() {
    let (start, mut snapshot) = fixture();
    let target = (2000.0, 2000.0);
    let mut waves = RushWaves::default();
    waves.rally(&frame(&start, &snapshot), 2, target).unwrap();
    step(&mut waves, &start, &snapshot, target);
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    step(&mut waves, &start, &snapshot, target);
    let staged: Vec<_> = waves.staged.iter().copied().collect();
    assert_eq!(staged.len(), 4);
    let specimen = snapshot
        .entities
        .iter()
        .find(|e| e.id == staged[0])
        .unwrap()
        .clone();
    add_rifle(&mut snapshot, &specimen, 1001);
    add_rifle(&mut snapshot, &specimen, 1002);
    waves.launched.insert(1002);
    let mut enemy = specimen.clone();
    enemy.id = 2001;
    enemy.owner = 2;
    enemy.x += 64.0;
    enemy.target_id = Some(staged[0]);
    let threat = (enemy.x, enemy.y);
    snapshot.entities.push(enemy.clone());
    // A visible bystander must not prolong the response after the actual attackers vanish.
    enemy.id = 2002;
    enemy.target_id = None;
    snapshot.entities.push(enemy);
    waves.first_wave_unlocked = true;
    snapshot.tick += 9;
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(orders.iter().any(|a| matches!(a,
        AiActionRequest::AttackMove { units, x, y, queued: false }
        if units == &staged && (*x, *y) == threat)));
    assert!(!orders.iter().any(|a| matches!(a,
        AiActionRequest::AttackMove { units, x, y, .. }
        if units.contains(&1002) && (*x, *y) == threat)));
    assert!(!waves.defending.contains(&1002));
    assert!(!waves.defending.contains(&1001));
    assert_eq!(waves.waves_launched, 0);
    // The threat stays visible between shots; defenders keep their cohort away from the line.
    for entity in &mut snapshot.entities {
        if staged.contains(&entity.id) {
            entity.x += 128.0;
            entity.state = "moving".into();
        }
        if entity.id == 2001 {
            entity.target_id = None;
            entity.x += 32.0;
        }
    }
    snapshot.tick += 9;
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(orders.iter().any(|a| matches!(a,
        AiActionRequest::AttackMove { units, x, y, .. }
        if units == &staged && (*x, *y) == (threat.0 + 32.0, threat.1))));
    snapshot.entities.retain(|entity| entity.id != 2001);
    snapshot.tick += 9;
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(waves.defending.is_empty());
    assert_eq!(waves.waves_launched, 0);
    for id in &staged {
        assert!(orders.iter().any(|a| matches!(a,
            AiActionRequest::AttackMove { units, x, y, queued: false }
            if units == &[*id] && (*x, *y) == waves.slots[id])));
    }
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(orders.iter().any(|a| matches!(a,
        AiActionRequest::AttackMove { units, x, y, .. }
        if units.len() == 5 && (*x, *y) == target)));
}
