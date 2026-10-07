use super::*;

const TS: f32 = 32.0;

fn tile(x: f32, y: f32) -> (i32, i32) {
    ((x * TS) as i32, (y * TS) as i32)
}

/// A straight route along y = 20 from x = 0 (Jeff) to x = 100 (the enemy).
fn route() -> Vec<(f32, f32)> {
    (0..100)
        .map(|x| ((x as f32 + 0.5) * TS, 20.5 * TS))
        .collect()
}

#[test]
fn a_line_across_the_route_is_found_whole_and_lines_come_nearest_to_jeff_first() {
    let mut traps = BTreeMap::new();
    // A diagonal line crossing the route at x = 60, two tiles apart like Schone Tage's arms.
    for (i, y) in [14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0]
        .iter()
        .enumerate()
    {
        traps.insert(10 + i as u32, tile(56.0 + i as f32, *y));
    }
    // A second line across the route at x = 30.
    traps.insert(30, tile(30.0, 19.0));
    traps.insert(31, tile(30.0, 21.0));
    // A wall off the route stays.
    traps.insert(50, tile(40.0, 60.0));
    traps.insert(51, tile(42.0, 60.0));
    let lines = blocking_lines(&traps, &route(), TS);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0], vec![30, 31], "the line nearest Jeff comes first");
    let mut far = lines[1].clone();
    far.sort_unstable();
    assert_eq!(
        far,
        (10..17).collect::<Vec<_>>(),
        "the whole line, not only the traps on the route"
    );
}

#[test]
fn a_post_counts_as_jeffs_side_only_if_the_way_back_passes_no_standing_trap() {
    let route = route();
    // A wall across the route at x = 60, two tiles apart.
    let wall: Vec<(f32, f32)> = (0..7)
        .map(|i| (60.0 * TS, (14.0 + 2.0 * i as f32) * TS))
        .collect();
    let trap = wall[3];
    let back = jeff_side_point(&route, trap, TS);
    assert!(
        back.0 < trap.0 - 8.0 * TS,
        "the reference point lies back toward Jeff: {back:?}"
    );
    let behind = (53.0 * TS, 18.0 * TS);
    let beyond = (67.0 * TS, 18.0 * TS);
    assert!(
        !crosses_traps(behind, back, &wall, TS),
        "a post behind the wall is reachable"
    );
    assert!(
        crosses_traps(beyond, back, &wall, TS),
        "a post beyond the wall is not"
    );
    for radius in POST_RINGS {
        assert!(radius <= TANK_SIGHT_TILES, "posts see their trap");
    }
}

#[test]
fn nothing_is_reserved_unless_the_breach_is_clearing_or_holding() {
    let mut breach = TrapBreach::default();
    breach.tanks.insert(5);
    assert_eq!(breach.reserved().count(), 0);
    breach.phase = Phase::Clear;
    assert_eq!(breach.reserved().collect::<Vec<_>>(), vec![5]);
    breach.paused = true;
    assert_eq!(breach.reserved().count(), 0);
}
