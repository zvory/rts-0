use super::*;

fn unit(id: u32, x: f32, hp: u32, target: Option<u32>) -> PullbackUnit {
    PullbackUnit {
        id,
        position: (x, 100.0),
        hp,
        target,
    }
}

#[test]
fn wounded_target_pulls_back_once_then_releases_on_the_exact_tick() {
    let mut memory = PullbackMemory::default();
    let units = [unit(1, 100.0, 15, None), unit(2, 100.0, 45, None)];
    let enemies = [unit(10, 150.0, 45, Some(1))];
    let params = PullbackParams::default();
    let plan = memory.plan(9, (100, 100, 32), &units, &enemies, params);
    assert_eq!(
        plan.moves,
        vec![PullbackMove {
            unit: 1,
            position: (16.0, 100.0)
        }]
    );
    assert_eq!(plan.active, BTreeSet::from([1]));
    let plan = memory.plan(53, (100, 100, 32), &units, &[], params);
    assert!(plan.moves.is_empty());
    assert!(plan.active.contains(&1));
    let plan = memory.plan(54, (100, 100, 32), &units, &[], params);
    assert!(plan.active.is_empty());
    assert_eq!(plan.released, BTreeSet::from([1]));
}

#[test]
fn no_visible_target_or_no_healthy_member_means_no_retreat() {
    let mut memory = PullbackMemory::default();
    let units = [unit(1, 100.0, 15, None)];
    let enemies = [unit(10, 150.0, 45, Some(1))];
    assert!(memory
        .plan(
            9,
            (100, 100, 32),
            &units,
            &enemies,
            PullbackParams::default()
        )
        .moves
        .is_empty());
    let units = [units[0], unit(2, 100.0, 45, None)];
    assert!(memory
        .plan(9, (100, 100, 32), &units, &[], PullbackParams::default())
        .moves
        .is_empty());
}

#[test]
fn nearest_enemy_and_id_tie_break_preserve_the_legacy_direction() {
    let mut memory = PullbackMemory::default();
    let units = [unit(1, 150.0, 10, None), unit(2, 150.0, 45, None)];
    let enemies = [unit(20, 200.0, 45, Some(1)), unit(10, 100.0, 45, None)];
    let plan = memory.plan(
        0,
        (100, 100, 32),
        &units,
        &enemies,
        PullbackParams::default(),
    );
    assert_eq!(plan.moves[0].position, (246.0, 100.0));
    memory.retain(&BTreeSet::from([2]), 1);
    assert_eq!(memory, PullbackMemory::default());
}

#[test]
fn sdk_failed_emission_is_retried_and_enemy_buildings_do_not_trigger_retreat() {
    use crate::sdk::{AiEntityState, EntityKind};
    use rts_sim::game::{Game, PlayerInit};
    let players = [1, 2].map(|id| PlayerInit {
        id,
        team_id: id,
        faction_id: "kriegsia".into(),
        name: format!("Player {id}"),
        color: "#4cc9f0".into(),
        is_ai: true,
    });
    let game = Game::new_without_ai_controllers(&players, 7);
    let start = game.start_payload();
    let snapshot = game.snapshot_for(1);
    let mut frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    let rifles: Vec<_> = frame
        .owned()
        .iter()
        .filter(|e| e.kind == EntityKind::Rifleman)
        .map(|e| e.id)
        .collect();
    let wounded = rifles[0];
    let mut snapshot = snapshot;
    let specimen = snapshot
        .entities
        .iter()
        .find(|e| e.id == wounded)
        .unwrap()
        .clone();
    for entity in &mut snapshot.entities {
        if entity.id == wounded {
            entity.hp = 10;
            entity.x = 320.0;
            entity.y = 320.0;
        }
    }
    let mut enemy = specimen;
    enemy.id = 999;
    enemy.owner = 2;
    enemy.x = 400.0;
    enemy.y = 320.0;
    enemy.target_id = Some(wounded);
    snapshot.entities.push(enemy);
    frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    let mut memory = PullbackMemory::default();
    let mut actions = AiActions::for_frame(&frame);
    actions
        .hold_position(&UnitGroup::new([wounded]).unwrap(), false)
        .unwrap();
    let failed = memory.step(&frame, &rifles, PullbackParams::default(), &mut actions);
    assert!(failed.active.is_empty());
    let mut actions = AiActions::for_frame(&frame);
    let accepted = memory.step(&frame, &rifles, PullbackParams::default(), &mut actions);
    assert_eq!(accepted.moves.len(), 1);
    assert!(accepted.active.contains(&wounded));
    // If the timer expires but re-emission is blocked, callers must still restore the old route.
    snapshot.tick += PullbackParams::default().ticks;
    let expired = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    let mut blocked = AiActions::for_frame(&expired);
    blocked
        .hold_position(&UnitGroup::new([wounded]).unwrap(), false)
        .unwrap();
    let rejected = memory.step(&expired, &rifles, PullbackParams::default(), &mut blocked);
    assert!(rejected.active.is_empty());
    assert!(rejected.released.contains(&wounded));

    assert_eq!(
        frame
            .owned()
            .iter()
            .find(|e| e.id == wounded)
            .unwrap()
            .state,
        AiEntityState::Idle
    );
    snapshot.entities.last_mut().unwrap().kind = "barracks".into();
    let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    let mut memory = PullbackMemory::default();
    let mut actions = AiActions::for_frame(&frame);
    assert!(memory
        .step(&frame, &rifles, PullbackParams::default(), &mut actions)
        .moves
        .is_empty());
}

#[test]
fn disabling_or_losing_the_healthy_screen_releases_active_members() {
    for params in [
        PullbackParams {
            hp: 0,
            ..PullbackParams::default()
        },
        PullbackParams::default(),
    ] {
        let mut memory = PullbackMemory::default();
        let mut units = [unit(1, 100.0, 10, None), unit(2, 100.0, 45, None)];
        let enemies = [unit(10, 150.0, 45, Some(1))];
        memory.plan(
            0,
            (100, 100, 32),
            &units,
            &enemies,
            PullbackParams::default(),
        );
        units[1].hp = 15;
        let plan = memory.plan(9, (100, 100, 32), &units, &enemies, params);
        assert!(plan.active.is_empty());
        assert_eq!(plan.released, BTreeSet::from([1]));
    }
}
