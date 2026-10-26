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
    assert!((distance(rally, target) / size - 25.0).abs() < 0.001);
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
            .filter(|action| matches!(action, AiActionRequest::Move { .. }))
            .count(),
        4
    );
    assert!(!initial
        .iter()
        .any(|action| matches!(action, AiActionRequest::AttackMove { .. })));
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
            .any(|action| matches!(action, AiActionRequest::AttackMove { .. })),
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
    assert!(matches!(next.as_slice(),[AiActionRequest::Move {units,..}] if units==&vec![1006]));
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
