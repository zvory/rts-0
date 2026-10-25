use super::*;
use crate::sdk::actions::AiActionRequest;
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
fn opening_attacks_immediately_then_reinforcements_require_five_arrivals() {
    let (start, mut snapshot) = fixture();
    let initial_frame = frame(&start, &snapshot);
    let enemy = initial_frame.players().iter().find(|p| p.id == 2).unwrap();
    let size = initial_frame.map().tile_size as f32;
    let target = (
        (enemy.start_tile.0 as f32 + 0.5) * size,
        (enemy.start_tile.1 as f32 + 0.5) * size,
    );
    let mut waves = RushWaves::default();
    let rally = waves.rally(&initial_frame, 2, target).unwrap();
    assert!((distance(rally, target) / size - 25.0).abs() < 0.001);
    let specimen = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    let opening = initial_frame
        .owned()
        .iter()
        .filter(|e| e.kind == EntityKind::Rifleman)
        .map(|e| e.id)
        .collect::<Vec<_>>();
    assert_eq!(opening.len(), 4);
    assert!(snapshot.upgrades.is_empty());
    let initial = step(&mut waves, &start, &snapshot, target);
    assert!(
        matches!(initial.as_slice(), [AiActionRequest::AttackMove { units, x, y, .. }] if units == &opening && (*x, *y) == target)
    );
    assert!(
        waves.slots.is_empty(),
        "opening Riflemen bypass staging entirely"
    );
    assert!(waves.held.is_empty());
    snapshot.tick += 9;
    assert!(
        step(&mut waves, &start, &snapshot, target).is_empty(),
        "opening must not be reissued every decision"
    );

    for id in 1001..=1004 {
        add_rifle(&mut snapshot, &specimen, id);
    }
    snapshot.tick += 9;
    let staging = step(&mut waves, &start, &snapshot, target);
    assert_eq!(
        staging
            .iter()
            .filter(|a| matches!(a, AiActionRequest::Move { .. }))
            .count(),
        4
    );
    let line = waves.plan.unwrap().1;
    for slot in waves.slots.values() {
        let along =
            (slot.0 - line.center.0) * line.forward.0 + (slot.1 - line.center.1) * line.forward.1;
        assert!(
            along.abs() < 0.01,
            "reinforcements retain the perpendicular staging line"
        );
    }
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    assert!(!step(&mut waves, &start, &snapshot, target)
        .iter()
        .any(|a| matches!(a, AiActionRequest::AttackMove { .. })));
    add_rifle(&mut snapshot, &specimen, 1005);
    snapshot.tick += 9;
    assert!(
        !step(&mut waves, &start, &snapshot, target)
            .iter()
            .any(|a| matches!(a, AiActionRequest::AttackMove { .. })),
        "an en-route fifth unit does not count"
    );
    assemble(&mut snapshot, &waves);
    snapshot.tick += 9;
    let launch = step(&mut waves, &start, &snapshot, target);
    assert!(
        matches!(launch.as_slice(), [AiActionRequest::AttackMove { units, .. }] if units == &vec![1001,1002,1003,1004,1005])
    );
    assert!(opening.iter().all(|id| waves.launched.contains(id)));
    add_rifle(&mut snapshot, &specimen, 1006);
    snapshot.tick += 9;
    assert!(
        matches!(step(&mut waves, &start, &snapshot, target).as_slice(), [AiActionRequest::Move { units, .. }] if units==&vec![1006])
    );
}

#[test]
fn opening_dispatch_does_not_depend_on_a_staging_route() {
    let (start, snapshot) = fixture();
    let mut waves = RushWaves::default();
    assert!(waves.plan.is_none());
    assert!(
        matches!(step(&mut waves, &start, &snapshot, (1000.0,1000.0)).as_slice(), [AiActionRequest::AttackMove { units, .. }] if units.len()==4)
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
    assert!((distance(line.center, enemy) / 32.0 - 25.0).abs() < 0.001);
    assert!(
        line.center.1 > 25.0 * 32.0,
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
    for id in 1101..=1104 {
        add_rifle(&mut snapshot, &specimen, id);
    }
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
