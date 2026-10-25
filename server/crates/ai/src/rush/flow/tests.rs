use super::*;
use crate::sdk::actions::AiActionRequest;
use crate::sdk::{AiFrame, AiTerrain};
use rts_sim::game::{Game, PlayerInit};
use rts_sim::protocol::{EntityView, Snapshot, StartPayload};

fn grid(width: usize, height: usize, threats: Vec<Threat>) -> FlowGrid {
    FlowGrid::new(GridInput {
        width,
        height,
        size: 32.0,
        target: ((width as f32 - 3.5) * 32.0, height as f32 * 16.0),
        blocked: vec![false; width * height],
        threats,
    })
    .unwrap()
}

fn route(grid: &FlowGrid, start: Point, safe: bool) -> Vec<usize> {
    let mut cell = grid.input.cell(start).unwrap();
    let mut path = vec![cell];
    let costs = if safe { &grid.safe } else { &grid.soft };
    for _ in 0..grid.danger.len() {
        if costs[cell] == (0, 0) {
            return path;
        }
        cell = grid.next(cell, safe).expect("route must make progress");
        assert!(!path.contains(&cell), "flow must not cycle");
        path.push(cell);
    }
    panic!("flow did not reach the objective")
}

#[test]
fn open_flank_flows_around_the_whole_firing_circle() {
    let grid = grid(
        48,
        32,
        vec![Threat {
            position: (24.5 * 32.0, 16.5 * 32.0),
            radius: 6.0 * 32.0,
            dps: 2000,
        }],
    );
    let start = (7.5 * 32.0, 16.5 * 32.0);
    let path = route(&grid, start, true);
    assert!(path.iter().all(|&cell| grid.danger[cell] == 0));
    assert!(path.iter().any(|&cell| (cell / 48).abs_diff(16) >= 7));
    let mut position = start;
    for _ in 0..150 {
        let (point, breach) = grid.waypoint(position).unwrap();
        if breach {
            assert!(distance(position, grid.input.target) <= GOAL_RADIUS * 32.0);
            return;
        }
        assert!(grid.segment_clear(position, point, true));
        assert!(distance(position, point) > 1.0);
        position = point;
    }
    panic!("waypoints failed to navigate the safe flank")
}

#[test]
fn sealed_wall_attacks_the_lower_dps_end_and_sums_overlapping_fire() {
    let grid = grid(
        48,
        26,
        vec![
            Threat {
                position: (24.5 * 32.0, 4.5 * 32.0),
                radius: 5.0 * 32.0,
                dps: 300,
            },
            Threat {
                position: (24.5 * 32.0, 13.5 * 32.0),
                radius: 5.0 * 32.0,
                dps: 3000,
            },
            Threat {
                position: (24.5 * 32.0, 22.5 * 32.0),
                radius: 5.0 * 32.0,
                dps: 3000,
            },
        ],
    );
    let start = (7.5 * 32.0, 13.5 * 32.0);
    assert_eq!(grid.safe[grid.input.cell(start).unwrap()], UNREACHABLE);
    let path = route(&grid, start, false);
    let crossing = path.iter().find(|&&cell| cell % 48 == 24).unwrap();
    assert!(crossing / 48 < 5, "choose the weak upper end: {crossing}");
    let overlap = grid.input.cell((24.5 * 32.0, 9.5 * 32.0)).unwrap();
    assert_eq!(grid.danger[overlap], 3300);
    let mut position = start;
    for _ in 0..100 {
        let (point, breach) = grid.waypoint(position).unwrap();
        if breach {
            assert!(
                point.1 < 6.0 * 32.0,
                "attack the weak approach, not the central wall"
            );
            return;
        }
        assert!(distance(position, point) > 1.0);
        position = point;
    }
    panic!("sealed-wall route never attacks")
}

#[test]
fn terrain_wall_changes_the_available_flank_without_corner_cutting() {
    let mut grid = grid(
        48,
        32,
        vec![Threat {
            position: (24.5 * 32.0, 16.5 * 32.0),
            radius: 6.0 * 32.0,
            dps: 1000,
        }],
    );
    // Close the north flank using terrain; leave a lower route around the trench.
    for y in 0..16 {
        grid.input.blocked[y * 48 + 24] = true;
    }
    let grid = FlowGrid::new(grid.input).unwrap();
    let path = route(&grid, (7.5 * 32.0, 16.5 * 32.0), true);
    assert!(path
        .iter()
        .all(|&cell| !grid.input.blocked[cell] && grid.danger[cell] == 0));
    assert!(path.iter().any(|&cell| cell % 48 == 24 && cell / 48 >= 24));
}

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
    let mut start = game.start_payload();
    start.map.width = 48;
    start.map.height = 32;
    start.map.terrain = vec![0; 48 * 32];
    let mut snapshot = game.snapshot_for(1);
    snapshot.entities = vec![
        EntityView::new(
            100,
            1,
            "rifleman",
            7.5 * 32.0,
            16.5 * 32.0,
            100,
            100,
            "move",
        ),
        EntityView::new(
            200,
            2,
            "rifleman",
            24.5 * 32.0,
            16.5 * 32.0,
            100,
            100,
            "idle",
        ),
    ];
    snapshot.entities[1].occupied_trench_id = Some(300);
    (start, snapshot)
}

fn step(flow: &mut RushFlow, start: &StartPayload, snapshot: &Snapshot) -> Vec<AiActionRequest> {
    let frame = AiFrame::from_host(start, snapshot, 1, [], Some(&[1, 2])).unwrap();
    let mut actions = AiActions::for_frame(&frame);
    flow.step(
        &frame,
        (44.5 * 32.0, 16.0 * 32.0),
        &BTreeSet::from([100]),
        &mut actions,
    );
    actions.into_requests()
}

#[test]
fn only_launched_units_are_steered_and_fog_memory_expires_without_command_spam() {
    let (start, mut snapshot) = fixture();
    snapshot.entities.push(EntityView::new(
        101,
        1,
        "rifleman",
        6.5 * 32.0,
        16.5 * 32.0,
        100,
        100,
        "idle",
    ));
    let mut flow = RushFlow::default();
    let first = step(&mut flow, &start, &snapshot);
    assert!(matches!(first.as_slice(),[AiActionRequest::Move {units,..}] if units==&vec![100]));
    snapshot.tick += 9;
    assert!(step(&mut flow, &start, &snapshot).is_empty());
    snapshot.entities.retain(|e| e.id != 200);
    snapshot.tick += 9;
    assert!(step(&mut flow, &start, &snapshot).is_empty());
    assert_eq!(flow.contacts.len(), 1);
    snapshot.tick += CONTACT_MEMORY + 1;
    let resume = step(&mut flow, &start, &snapshot);
    assert!(
        matches!(resume.as_slice(),[AiActionRequest::AttackMove {units,..}] if units==&vec![100])
    );
    assert!(flow.contacts.is_empty());
    snapshot.tick += 9;
    assert!(step(&mut flow, &start, &snapshot).is_empty());
}

#[test]
fn visible_departure_clears_a_trench_and_vision_only_contacts_never_add_one() {
    let (start, mut snapshot) = fixture();
    let mut flow = RushFlow::default();
    snapshot.entities[1].vision_only = true;
    assert!(step(&mut flow, &start, &snapshot).is_empty());
    assert!(flow.contacts.is_empty());
    snapshot.entities[1].vision_only = false;
    step(&mut flow, &start, &snapshot);
    assert_eq!(flow.contacts.len(), 1);
    snapshot.entities[1].occupied_trench_id = None;
    snapshot.tick += 9;
    assert!(matches!(
        step(&mut flow, &start, &snapshot).as_slice(),
        [AiActionRequest::AttackMove { .. }]
    ));
    assert!(flow.contacts.is_empty());
}

#[test]
fn frame_maps_public_trench_occupation_and_known_buildings_block_routes() {
    let (start, mut snapshot) = fixture();
    snapshot.entities.push(EntityView::new(
        400,
        2,
        "barracks",
        12.5 * 32.0,
        16.0 * 32.0,
        100,
        100,
        "idle",
    ));
    let frame = AiFrame::from_host(&start, &snapshot, 1, [], Some(&[1, 2])).unwrap();
    assert_eq!(
        frame
            .visible_enemies()
            .iter()
            .find(|e| e.id == 200)
            .unwrap()
            .occupied_trench_id,
        Some(300)
    );
    assert_eq!(frame.owned()[0].occupied_trench_id, None);
    assert_eq!(frame.map().terrain[0], AiTerrain::Grass);
    let input = GridInput::from_frame(&frame, (44.5 * 32.0, 16.0 * 32.0), vec![]);
    // Construction centers the 3x2 Barracks on a tile boundary on its even-height axis.
    // Check the entire footprint, including the adjacent open tiles, against simulation occupancy.
    let blocked = input
        .blocked
        .iter()
        .enumerate()
        .filter_map(|(cell, &blocked)| blocked.then_some((cell % 48, cell / 48)))
        .collect::<Vec<_>>();
    assert_eq!(
        blocked,
        vec![(11, 15), (12, 15), (13, 15), (11, 16), (12, 16), (13, 16)]
    );
    let grid = FlowGrid::new(input).unwrap();
    let path = route(&grid, (7.5 * 32.0, 16.5 * 32.0), true);
    assert!(path.iter().all(|&cell| !grid.input.blocked[cell]));
}

#[test]
fn late_obstacle_replaces_a_stale_move_when_no_route_remains() {
    let (start, mut snapshot) = fixture();
    let mut flow = RushFlow::default();
    assert!(matches!(
        step(&mut flow, &start, &snapshot).as_slice(),
        [AiActionRequest::Move { .. }]
    ));
    // A visible building now occupies this unit's cell. The AI grid has no route from it,
    // but the simulation can resolve occupancy and combat through the ordinary base attack.
    snapshot.entities.push(EntityView::new(
        401,
        2,
        "barracks",
        7.5 * 32.0,
        16.0 * 32.0,
        100,
        100,
        "idle",
    ));
    snapshot.tick += 9;
    assert!(
        matches!(step(&mut flow, &start, &snapshot).as_slice(), [AiActionRequest::AttackMove { units, x, y, .. }] if units==&vec![100] && *x==44.5*32.0 && *y==16.0*32.0)
    );
}
