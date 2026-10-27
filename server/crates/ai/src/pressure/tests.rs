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
    waves: &mut PressureWaves,
    start: &StartPayload,
    snapshot: &Snapshot,
    target: Point,
) -> Vec<AiActionRequest> {
    let frame = frame(start, snapshot);
    let mut actions = AiActions::for_frame(&frame);
    let eligible = frame.owned().iter().map(|e| e.id).collect();
    waves.step(&frame, target, &eligible, &mut actions);
    actions.into_requests()
}

fn assemble(snapshot: &mut Snapshot, waves: &PressureWaves) {
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
    let mut waves = PressureWaves::default();
    let rally = waves.rally(&frame, 2, target).unwrap();
    assert!((distance(rally, target) / size - 35.0).abs() < 0.001);
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
    for action in &initial {
        if let AiActionRequest::AttackMove {
            units,
            x,
            y,
            queued,
        } = action
        {
            assert_eq!(units.len(), 1);
            assert_eq!((*x, *y), waves.slots[&units[0]]);
            assert!(!queued);
        }
        assert!(!matches!(action, AiActionRequest::Move { .. }));
    }
    assert!(waves.launched.is_empty());
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
    let en_route = step(&mut waves, &start, &snapshot, target);
    assert!(en_route.iter().any(|action| matches!(action,
        AiActionRequest::AttackMove { units, x, y, queued: false }
        if units == &vec![1005] && (*x, *y) == waves.slots[&1005]
    )));
    assert!(
        (1001..=1005).all(|id| !waves.launched.contains(&id)),
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
    assert!((distance(line.center, enemy) / 32.0 - 35.0).abs() < 0.001);
    assert!(
        line.center.1 > enemy.1 + 0.25 * 32.0,
        "staging must use the route around water, not the blocked base diagonal"
    );
    assert!(passable_world(&blocked, line.center));
}

#[test]
fn panzerfausts_join_front_infantry_while_tanks_stage_three_tiles_behind() {
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
    let mut waves = PressureWaves::default();
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
        panzer.kind = "tank".into();
        snapshot.entities.push(panzer);
    }
    for id in 1004..=1006 {
        let mut unit = specimen.clone();
        unit.id = id;
        unit.kind = "panzerfaust".into();
        snapshot.entities.push(unit);
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
        let expected = if entity.kind == "tank" { 3.0 } else { 0.0 };
        assert!(
            (depth - expected).abs() < 0.001,
            "{} staged at depth {depth}",
            entity.kind
        );
    }
    assert!((1001..=1006).all(|id| waves.slots.contains_key(&id)));
}

#[test]
fn defense_can_borrow_unlaunched_units_and_they_resume_staging_when_released() {
    let (start, mut snapshot) = fixture();
    let initial = frame(&start, &snapshot);
    let enemy = initial.players().iter().find(|p| p.id == 2).unwrap();
    let size = initial.map().tile_size as f32;
    let target = (
        (enemy.start_tile.0 as f32 + 0.5) * size,
        (enemy.start_tile.1 as f32 + 0.5) * size,
    );
    let mut waves = PressureWaves::default();
    waves.rally(&initial, 2, target).unwrap();
    step(&mut waves, &start, &snapshot, target);
    let id = *waves.slots.keys().next().unwrap();
    let current = frame(&start, &snapshot);
    let eligible = BTreeSet::new();
    let mut actions = AiActions::for_frame(&current);
    waves.step(&current, target, &eligible, &mut actions);
    assert!(
        actions.into_requests().is_empty(),
        "defense owns every unit this decision"
    );
    snapshot.tick += 9;
    let released = step(&mut waves, &start, &snapshot, target);
    assert!(released.iter().any(
        |request| matches!(request, AiActionRequest::AttackMove { units, .. } if units.contains(&id))
    ));
}

fn production_step(
    production: &mut production::PressureProduction,
    start: &StartPayload,
    snapshot: &Snapshot,
) -> Vec<AiActionRequest> {
    let current = frame(start, snapshot);
    let mut actions = AiActions::for_frame(&current);
    production.step(&current, &mut actions);
    actions.into_requests()
}

#[test]
fn panzerfaust_research_waits_for_two_minutes_and_recovers_if_its_centre_dies() {
    let (start, mut snapshot) = fixture();
    let mut production = production::PressureProduction::default();
    snapshot.entities.push(EntityView::new(
        990,
        1,
        "training_centre",
        500.0,
        500.0,
        120,
        120,
        "idle",
    ));
    snapshot.steel = 0;
    snapshot.oil = 0;
    snapshot.tick = 3599;
    assert!(production_step(&mut production, &start, &snapshot).is_empty());
    snapshot.tick = 3600;
    assert!(matches!(
        production_step(&mut production, &start, &snapshot).as_slice(),
        [AiActionRequest::Research {
            building: 990,
            upgrade: UpgradeKind::Panzerfausts
        }]
    ));
    snapshot.tick += 9;
    assert!(
        production_step(&mut production, &start, &snapshot).is_empty(),
        "research must not be queued repeatedly"
    );
    snapshot.entities.retain(|e| e.id != 990);
    snapshot.entities.push(EntityView::new(
        991,
        1,
        "training_centre",
        500.0,
        500.0,
        120,
        120,
        "idle",
    ));
    assert!(matches!(
        production_step(&mut production, &start, &snapshot).as_slice(),
        [AiActionRequest::Research {
            building: 991,
            upgrade: UpgradeKind::Panzerfausts
        }]
    ));
}

#[test]
fn panzerfaust_repeat_starts_only_after_research_and_configures_replacement_barracks() {
    let (start, mut snapshot) = fixture();
    let mut production = production::PressureProduction::default();
    snapshot.tick = 3600;
    snapshot.entities.push(EntityView::new(
        990, 1, "barracks", 500.0, 500.0, 120, 120, "idle",
    ));
    assert!(
        production_step(&mut production, &start, &snapshot).is_empty(),
        "locked Panzerfausts must not be repeated"
    );
    snapshot.upgrades.push("panzerfausts".into());
    assert!(
        matches!(production_step(&mut production, &start, &snapshot).as_slice(), [
        AiActionRequest::AdjustProductionRepeat { buildings: old, unit: EntityKind::Rifleman, delta: -1 },
        AiActionRequest::AdjustProductionRepeat { buildings: new, unit: EntityKind::Panzerfaust, delta: 1 },
    ] if old == &[990] && new == &[990])
    );
    snapshot.tick += 9;
    assert!(
        production_step(&mut production, &start, &snapshot).is_empty(),
        "repeat allocations must not grow each decision"
    );
    snapshot.entities.retain(|e| e.id != 990);
    snapshot.entities.push(EntityView::new(
        991, 1, "barracks", 500.0, 500.0, 120, 120, "idle",
    ));
    let replacement = production_step(&mut production, &start, &snapshot);
    assert!(
        matches!(replacement.as_slice(), [_, AiActionRequest::AdjustProductionRepeat {
        buildings, unit: EntityKind::Panzerfaust, delta: 1,
    }] if buildings == &[991])
    );
}

fn timed_fixture(count: usize) -> (StartPayload, Snapshot, Vec<crate::sdk::AiBaseSite>, Point) {
    use crate::sdk::{AiBaseSite, AiBaseSiteId};
    let (mut start, mut snapshot) = fixture();
    start.map.terrain.fill(0);
    start.map.no_building_tiles.clear();
    let size = start.map.tile_size as f32;
    let point = |id| {
        let p = start.players.iter().find(|p| p.id == id).unwrap();
        (
            (p.start_tile_x as f32 + 0.5) * size,
            (p.start_tile_y as f32 + 0.5) * size,
        )
    };
    let home = point(1);
    let main = point(2);
    let second = (main.0 - 20.0 * size, main.1);
    let third = (main.0 - 40.0 * size, main.1 - 10.0 * size);
    let mut sites = vec![
        AiBaseSite {
            id: AiBaseSiteId::Start(1),
            position: home,
            resource_ids: vec![],
            ground_distances_from_starts: Default::default(),
        },
        AiBaseSite {
            id: AiBaseSiteId::Start(2),
            position: main,
            resource_ids: vec![],
            ground_distances_from_starts: Default::default(),
        },
        AiBaseSite {
            id: AiBaseSiteId::ResourceCluster(1),
            position: second,
            resource_ids: vec![],
            ground_distances_from_starts: Default::default(),
        },
        AiBaseSite {
            id: AiBaseSiteId::ResourceCluster(2),
            position: third,
            resource_ids: vec![],
            ground_distances_from_starts: Default::default(),
        },
    ];
    for site in &mut sites {
        site.ground_distances_from_starts = [
            (1, distance(home, site.position)),
            (2, distance(main, site.position)),
        ]
        .into_iter()
        .collect();
    }
    snapshot.entities.clear();
    snapshot
        .upgrades
        .extend(["methamphetamines".into(), "panzerfausts".into()]);
    for id in 1..=count as u32 {
        snapshot.entities.push(EntityView::new(
            id, 1, "rifleman", home.0, home.1, 45, 45, "idle",
        ));
    }
    (start, snapshot, sites, main)
}

fn timed_step(
    waves: &mut PressureWaves,
    start: &StartPayload,
    snapshot: &Snapshot,
    sites: &[crate::sdk::AiBaseSite],
    main: Point,
) -> Vec<AiActionRequest> {
    let current = frame(start, snapshot).with_base_sites(sites);
    waves.rally(&current, 2, main).unwrap();
    let eligible = current.owned().iter().map(|e| e.id).collect();
    let mut actions = AiActions::for_frame(&current);
    waves.step(&current, main, &eligible, &mut actions);
    actions.into_requests()
}

#[test]
fn pressure_retargets_at_exact_four_and_eight_minute_boundaries_on_the_main_base_axis() {
    let (start, mut snapshot, sites, main) = timed_fixture(4);
    let home = sites[0].position;
    let size = start.map.tile_size as f32;
    let mut waves = PressureWaves::default();
    for (tick, phase, target) in [
        (7199, 0, main),
        (7200, 1, sites[2].position),
        (14399, 1, sites[2].position),
        (14400, 2, sites[3].position),
    ] {
        snapshot.tick = tick;
        let current = frame(&start, &snapshot).with_base_sites(&sites);
        waves.rally(&current, 2, main).unwrap();
        let line = waves.plan.unwrap().1;
        let objective = waves.objective.unwrap();
        assert_eq!(objective.phase, phase);
        assert_eq!(objective.target(), target);
        assert_eq!(objective.minimum_wave_size(), 5 << phase);
        let radius = if phase == 0 { 35.0 } else { 25.0 };
        assert!((distance(line.center, target) / size - radius).abs() < 0.001);
        if phase > 0 {
            let axis = (
                (target.0 - home.0) / distance(home, target),
                (target.1 - home.1) / distance(home, target),
            );
            assert!(
                (line.forward.0 - axis.0).abs() < 0.0001
                    && (line.forward.1 - axis.1).abs() < 0.0001
            );
            let offset = (line.center.0 - home.0, line.center.1 - home.1);
            assert!((offset.0 * axis.1 - offset.1 * axis.0).abs() < 0.01);
        }
    }
}

#[test]
fn pressure_waits_for_ten_then_twenty_arrivals_and_queues_each_lower_base() {
    for (tick, minimum, expected) in [(7200, 10, vec![2, 1]), (14400, 20, vec![3, 2, 1])] {
        let (start, mut snapshot, sites, main) = timed_fixture(minimum - 1);
        snapshot.tick = tick;
        let mut waves = PressureWaves {
            first_wave_launched: true,
            ..Default::default()
        };
        timed_step(&mut waves, &start, &snapshot, &sites, main);
        assemble(&mut snapshot, &waves);
        let below = timed_step(&mut waves, &start, &snapshot, &sites, main);
        assert!(!below
            .iter()
            .any(|a| matches!(a, AiActionRequest::AttackMove { .. })));
        let specimen = snapshot.entities[0].clone();
        add_rifle(&mut snapshot, &specimen, minimum as u32);
        timed_step(&mut waves, &start, &snapshot, &sites, main);
        assemble(&mut snapshot, &waves);
        let launch = timed_step(&mut waves, &start, &snapshot, &sites, main);
        let route: Vec<_> = launch
            .iter()
            .filter_map(|a| match a {
                AiActionRequest::AttackMove {
                    units,
                    x,
                    y,
                    queued,
                } => Some((units, (*x, *y), *queued)),
                _ => None,
            })
            .collect();
        assert_eq!(route.len(), expected.len());
        for (i, ((units, point, queued), rank)) in route.iter().zip(expected).enumerate() {
            let target = match rank {
                1 => main,
                2 => sites[2].position,
                _ => sites[3].position,
            };
            assert_eq!(units.len(), minimum);
            assert_eq!(*point, target);
            assert_eq!(*queued, i > 0);
            assert_eq!(*units, route[0].0);
        }
        assert_eq!(waves.launched.len(), minimum);
        snapshot.tick += 9;
        assert!(timed_step(&mut waves, &start, &snapshot, &sites, main).is_empty());
    }
}

#[test]
fn pressure_retargets_only_unlaunched_units_and_reissues_their_movement() {
    let (start, mut snapshot, sites, main) = timed_fixture(4);
    snapshot.tick = 7199;
    let mut waves = PressureWaves::default();
    waves.launched.insert(1);
    timed_step(&mut waves, &start, &snapshot, &sites, main);
    let previous = waves.slots.clone();
    for tick in [7200, 14400] {
        snapshot.tick = tick;
        let requests = timed_step(&mut waves, &start, &snapshot, &sites, main);
        assert!(waves.launched.contains(&1));
        assert!(!waves.slots.contains_key(&1));
        assert!(
            requests
                .iter()
                .filter(|a| matches!(a, AiActionRequest::AttackMove { .. }))
                .count()
                >= 3
        );
        assert!(requests
            .iter()
            .all(|a| !matches!(a,AiActionRequest::AttackMove{units,..} if units.contains(&1))));
        assert_ne!(waves.slots, previous);
    }
}

#[test]
fn pressure_late_wave_still_requires_opening_research_and_handles_missing_ranked_sites() {
    let (start, mut snapshot, _, main) = timed_fixture(20);
    snapshot.tick = 14400;
    snapshot.upgrades.clear();
    let mut waves = PressureWaves::default();
    timed_step(&mut waves, &start, &snapshot, &[], main);
    assemble(&mut snapshot, &waves);
    assert!(!timed_step(&mut waves, &start, &snapshot, &[], main)
        .iter()
        .any(|a| matches!(a, AiActionRequest::AttackMove { .. })));
    snapshot.upgrades.push("methamphetamines".into());
    let requests = timed_step(&mut waves, &start, &snapshot, &[], main);
    assert_eq!(
        requests
            .iter()
            .filter(|a| matches!(a, AiActionRequest::AttackMove { .. }))
            .count(),
        1
    );
    assert!(requests
        .iter()
        .any(|a| matches!(a,AiActionRequest::AttackMove{x,y,queued:false,..}if (*x,*y)==main)));
}

#[test]
fn pressure_keeps_initial_ranked_landmarks_when_later_resources_or_site_rankings_change() {
    let (start, mut snapshot, mut sites, main) = timed_fixture(4);
    let mut waves = PressureWaves::default();
    timed_step(&mut waves, &start, &snapshot, &sites, main);
    let second = sites[2].position;
    let third = sites[3].position;
    sites.remove(2);
    snapshot.tick = 7200;
    timed_step(&mut waves, &start, &snapshot, &sites, main);
    assert_eq!(waves.objective.unwrap().target(), second);
    sites.clear();
    snapshot.tick = 14400;
    timed_step(&mut waves, &start, &snapshot, &sites, main);
    let objective = waves.objective.unwrap();
    assert_eq!(objective.target(), third);
    assert_eq!(objective.followups(), &[second, main]);
}

#[test]
fn later_axis_slots_skip_disconnected_ground_without_moving_the_line() {
    let (mut start, mut snapshot, sites, main) = timed_fixture(20);
    snapshot.tick = 14400;
    let size = start.map.tile_size as f32;
    let expected = StagingLine::on_axis(
        frame(&start, &snapshot).map(),
        sites[0].position,
        sites[3].position,
    )
    .unwrap();
    let wall_x = (expected.center.0 / size) as usize + 2;
    for y in 0..start.map.height as usize {
        start.map.terrain[y * start.map.width as usize + wall_x] =
            rts_rules::terrain::MAP_TERRAIN_WATER;
    }
    let mut waves = PressureWaves::default();
    timed_step(&mut waves, &start, &snapshot, &sites, main);
    assert_eq!(waves.plan.unwrap().1.center, expected.center);
    assert!(
        !waves.slots.is_empty(),
        "home={:?} wall={} center={:?} forward={:?} reachable={}",
        sites[0].position,
        wall_x,
        expected.center,
        expected.forward,
        waves.reachable.iter().filter(|v| **v).count()
    );
    for point in waves.slots.values() {
        let x = (point.0 / size) as usize;
        let y = (point.1 / size) as usize;
        assert!(x < wall_x, "slot must remain on the main's connected side");
        assert!(waves.reachable[y * start.map.width as usize + x]);
    }
}

#[test]
fn pressure_ground_connectivity_does_not_cut_diagonally_blocked_corners() {
    let mut map = AiMap {
        width: 3,
        height: 3,
        tile_size: 32,
        terrain: vec![AiTerrain::Grass; 9],
        no_building_tiles: BTreeSet::new(),
    };
    map.terrain[1] = AiTerrain::Water;
    map.terrain[3] = AiTerrain::Rock;
    let reachable = ground_reachable(&map, (16.0, 16.0));
    assert!(reachable[0]);
    assert!(!reachable[4]);
}

#[test]
fn response_recruits_value_escalates_tracks_fog_and_releases_at_thirty_seconds() {
    let (start, mut snapshot) = fixture();
    let rifle = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    snapshot.entities.retain(|e| e.kind != "rifleman");
    for id in 1000..1008 {
        let mut own = rifle.clone();
        own.id = id;
        own.x = rifle.x + (id - 1000) as f32 * 32.0;
        snapshot.entities.push(own);
    }
    let mut enemy = rifle.clone();
    enemy.id = 999;
    enemy.owner = 2;
    enemy.x += 64.0;
    enemy.target_id = Some(1000);
    snapshot.entities.push(enemy.clone());
    let mut response = response::PressureResponse::default();
    let run = |response: &mut response::PressureResponse, snapshot: &Snapshot| {
        let f = frame(&start, snapshot);
        let eligible = f.owned().iter().map(|e| e.id).collect();
        let mut actions = AiActions::for_frame(&f);
        let plan = response.step(&f, &eligible, &mut actions);
        (plan, actions.into_requests())
    };
    let (first, orders) = run(&mut response, &snapshot);
    assert_eq!(
        first.active.len(),
        2,
        "one enemy Rifleman needs two whole Riflemen for >=150% value"
    );
    assert!(orders.iter().all(|a| matches!(a, AiActionRequest::AttackMove { x, y, queued: false, .. } if (*x,*y) == (enemy.x,enemy.y))));
    enemy.id = 998;
    snapshot.entities.push(enemy.clone());
    snapshot.tick += 9;
    let (reinforced, _) = run(&mut response, &snapshot);
    assert_eq!(
        reinforced.active.len(),
        3,
        "two enemy Riflemen need three responders"
    );
    let last_seen = snapshot.tick;
    snapshot.entities.retain(|e| e.owner != 2);
    snapshot.tick = last_seen + 899;
    let (unseen, orders) = run(&mut response, &snapshot);
    assert_eq!(unseen.active, reinforced.active);
    assert!(orders.iter().all(
        |a| matches!(a, AiActionRequest::AttackMove { x, y, .. } if (*x,*y) == (enemy.x,enemy.y))
    ));
    snapshot.tick = last_seen + 900;
    let (expired, _) = run(&mut response, &snapshot);
    assert!(expired.active.is_empty());
    assert_eq!(expired.released, reinforced.active);
}

#[test]
fn recalled_launched_units_return_to_staging_after_contact_expires() {
    let (start, mut snapshot) = fixture();
    let target = (2000.0, 2000.0);
    let mut waves = PressureWaves::default();
    waves.rally(&frame(&start, &snapshot), 2, target).unwrap();
    step(&mut waves, &start, &snapshot, target);
    assemble(&mut snapshot, &waves);
    waves.first_wave_unlocked = true;
    step(&mut waves, &start, &snapshot, target);
    let id = *waves.launched.first().unwrap();
    // This is a home-side incident recalling a previously launched unit.
    let f = frame(&start, &snapshot);
    let home = f.players().iter().find(|p| p.id == 1).unwrap().start_tile;
    let size = f.map().tile_size as f32;
    for entity in snapshot
        .entities
        .iter_mut()
        .filter(|entity| entity.id == id)
    {
        entity.x = (home.0 as f32 + 0.5) * size;
        entity.y = (home.1 as f32 + 0.5) * size;
    }
    let mut enemy = snapshot
        .entities
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .clone();
    enemy.id = 999;
    enemy.owner = 2;
    enemy.target_id = Some(id);
    snapshot.entities.push(enemy);
    snapshot.tick += 9;
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(orders.iter().any(|a| matches!(a, AiActionRequest::AttackMove { units, queued: false, .. } if units.contains(&id))));
    // A pending pull-back resume must not take a responder back to its offensive route.
    waves.resume.insert(id);
    snapshot.tick += 9;
    let orders = step(&mut waves, &start, &snapshot, target);
    assert!(!orders.iter().any(|action| matches!(action,
        AiActionRequest::AttackMove { units, .. } if units.contains(&id))));
    // Construction clearance retains ownership even while a response is active.
    let f = frame(&start, &snapshot);
    let mut actions = AiActions::for_frame(&f);
    waves.step(&f, target, &BTreeSet::new(), &mut actions);
    assert!(!actions
        .into_requests()
        .iter()
        .any(|a| matches!(a, AiActionRequest::AttackMove { .. })));
    snapshot.entities.retain(|e| e.owner != 2);
    snapshot.tick += 900;
    step(&mut waves, &start, &snapshot, target);
    assert!(!waves.launched.contains(&id));
    assert!(!waves.wave_routes.contains_key(&id));
    assert!(waves.slots.contains_key(&id));
}

#[test]
fn response_budgets_mixed_purchase_values_and_replaces_dead_responders() {
    let (start, mut snapshot) = fixture();
    let rifle = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    snapshot.entities.retain(|e| e.kind != "rifleman");
    for id in 1000..1010 {
        let mut unit = rifle.clone();
        unit.id = id;
        unit.x += (id - 1000) as f32 * 16.0;
        if id >= 1004 {
            unit.kind = "tank".into();
        }
        snapshot.entities.push(unit);
    }
    let mut enemy = rifle.clone();
    enemy.id = 999;
    enemy.owner = 2;
    enemy.kind = "tank".into();
    enemy.target_id = Some(1000);
    snapshot.entities.push(enemy);
    let mut response = response::PressureResponse::default();
    let f = frame(&start, &snapshot);
    let book = crate::sdk::AiRulebook::for_frame(&f).unwrap();
    let tank = book.cost(EntityKind::Tank).unwrap();
    let required = (tank.steel + tank.oil) * 3 / 2;
    let eligible = f.owned().iter().map(|e| e.id).collect();
    let mut actions = AiActions::for_frame(&f);
    let plan = response.step(&f, &eligible, &mut actions);
    let committed: u32 = f
        .owned()
        .iter()
        .filter(|e| plan.active.contains(&e.id))
        .map(|e| {
            let cost = book.cost(e.kind).unwrap();
            cost.steel + cost.oil
        })
        .sum();
    assert!(committed >= required);
    assert!(
        committed - (tank.steel + tank.oil) < required,
        "stop as soon as full unit costs reach the budget"
    );
    let lost = *plan.active.last().unwrap();
    snapshot.entities.retain(|e| e.id != lost);
    snapshot.tick += 9;
    let f = frame(&start, &snapshot);
    let mut actions = AiActions::for_frame(&f);
    let eligible = f.owned().iter().map(|e| e.id).collect();
    let replacement = response.step(&f, &eligible, &mut actions);
    assert!(!replacement.active.contains(&lost));
    assert!(replacement
        .active
        .iter()
        .any(|id| !plan.active.contains(id)));
}

#[test]
fn response_ignores_forward_fights_and_never_refreshes_a_contact_across_the_midpoint() {
    let (start, mut snapshot) = fixture();
    let f = frame(&start, &snapshot);
    let size = f.map().tile_size as f32;
    let enemy_start = f.players().iter().find(|p| p.id == 2).unwrap().start_tile;
    let enemy_position = (
        (enemy_start.0 as f32 + 0.5) * size,
        (enemy_start.1 as f32 + 0.5) * size,
    );
    let rifle = snapshot
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == "rifleman")
        .unwrap()
        .clone();
    let mut enemy = rifle.clone();
    enemy.id = 999;
    enemy.owner = 2;
    enemy.x = enemy_position.0;
    enemy.y = enemy_position.1;
    enemy.target_id = Some(rifle.id);
    enemy.state = "attack".into();
    snapshot.entities.push(enemy);
    let mut response = response::PressureResponse::default();
    let run = |response: &mut response::PressureResponse, snapshot: &Snapshot| {
        let f = frame(&start, snapshot);
        let eligible = f.owned().iter().map(|e| e.id).collect();
        let mut actions = AiActions::for_frame(&f);
        let plan = response.step(&f, &eligible, &mut actions);
        (plan, actions.into_requests())
    };
    let (forward, orders) = run(&mut response, &snapshot);
    assert!(
        forward.active.is_empty(),
        "even an attacking enemy on its half cannot trigger a response"
    );
    assert!(orders.is_empty());
    snapshot.entities.last_mut().unwrap().x = rifle.x;
    snapshot.entities.last_mut().unwrap().y = rifle.y;
    snapshot.tick += 9;
    let last_home_sighting = snapshot.tick;
    let (home, _) = run(&mut response, &snapshot);
    assert!(!home.active.is_empty());
    snapshot.entities.last_mut().unwrap().x = enemy_position.0;
    snapshot.entities.last_mut().unwrap().y = enemy_position.1;
    snapshot.tick += 899;
    let (remembered, orders) = run(&mut response, &snapshot);
    assert_eq!(remembered.active, home.active);
    assert!(
        orders.iter().all(
            |a| matches!(a, AiActionRequest::AttackMove { x,y,.. } if (*x,*y) == (rifle.x,rifle.y))
        ),
        "pursuit stays at the last friendly-half location"
    );
    snapshot.tick = last_home_sighting + 900;
    let (expired, _) = run(&mut response, &snapshot);
    assert!(
        expired.active.is_empty(),
        "continued vision across the midpoint cannot extend the timeout"
    );
    assert_eq!(expired.released, home.active);
}
