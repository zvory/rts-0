use crate::game::pathfinding::Passability;

pub(super) fn expand_vehicle_diagonal_steps_to_l_waypoints<P: Passability>(
    start: (i32, i32),
    tile_path: &[(i32, i32)],
    pass: &P,
    initial_direction: Option<u8>,
) -> Vec<(i32, i32)> {
    let mut expanded = Vec::with_capacity(tile_path.len() * 2);
    let mut prev = start;

    for &next in tile_path {
        let dx = next.0 - prev.0;
        let dy = next.1 - prev.1;
        if dx.abs() == 1 && dy.abs() == 1 {
            if let Some(elbow) = choose_vehicle_l_elbow(
                prev,
                next,
                pass,
                if prev == start {
                    initial_direction
                } else {
                    None
                },
            ) {
                if expanded.last().copied() != Some(elbow) {
                    expanded.push(elbow);
                }
            }
        }
        if expanded.last().copied() != Some(next) {
            expanded.push(next);
        }
        prev = next;
    }

    expanded
}

fn choose_vehicle_l_elbow<P: Passability>(
    prev: (i32, i32),
    next: (i32, i32),
    pass: &P,
    initial_direction: Option<u8>,
) -> Option<(i32, i32)> {
    let horizontal_first = (next.0, prev.1);
    let vertical_first = (prev.0, next.1);
    let horizontal_ok = pass.passable(horizontal_first.0, horizontal_first.1);
    let vertical_ok = pass.passable(vertical_first.0, vertical_first.1);

    match (horizontal_ok, vertical_ok) {
        (true, true) => {
            let horizontal_cost = pass.movement_cost(horizontal_first.0, horizontal_first.1, 10);
            let vertical_cost = pass.movement_cost(vertical_first.0, vertical_first.1, 10);
            let prefer_horizontal = initial_direction
                .map(|heading| {
                    let (fx, fy) = [
                        (1, 0),
                        (-1, 0),
                        (0, 1),
                        (0, -1),
                        (1, 1),
                        (1, -1),
                        (-1, 1),
                        (-1, -1),
                    ]
                    .get(heading as usize)
                    .copied()
                    .unwrap_or((0, 0));
                    fx * (next.0 - prev.0) >= fy * (next.1 - prev.1)
                })
                .unwrap_or(true);
            if horizontal_cost < vertical_cost
                || (horizontal_cost == vertical_cost && prefer_horizontal)
            {
                Some(horizontal_first)
            } else {
                Some(vertical_first)
            }
        }
        (true, false) => Some(horizontal_first),
        (false, true) => Some(vertical_first),
        (false, false) => None,
    }
}
