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
