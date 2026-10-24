use super::*;

#[test]
fn supplemental_rifles_reserve_distinct_spaced_posts_outside_the_opening_pocket() {
    let mut observation = los_test_observation(EntityKind::Factory);
    observation.owned.clear();
    let ts = observation.map.tile_size as f32;
    let enemy = EnemyBaseFact {
        player_id: 2,
        start_tile: (25, 5),
        x: 25.5 * ts,
        y: 5.5 * ts,
    };
    let units: Vec<_> = (1..=14).collect();
    let assignments =
        home_defensive_pocket_rifle_assignments(&observation, None, &units, enemy).unwrap();
    assert_eq!(assignments.len(), units.len());
    for slot in assignments.iter().skip(4) {
        for other in assignments
            .iter()
            .filter(|other| other.unit_id != slot.unit_id)
        {
            assert!(
                dist2(slot.x, slot.y, other.x, other.y) >= squared(2.75 * ts),
                "{} crowds {}",
                slot.unit_id,
                other.unit_id
            );
        }
    }
}
use crate::ai_core::observation::{AiBuildIntent, AiEconomy, AiPlayerSummary, AiResourceSummary};

fn los_test_observation(blocker: EntityKind) -> AiObservation {
    let tile_size = config::TILE_SIZE;
    AiObservation {
        player_id: 1,
        tick: 0,
        map: AiMapSummary {
            width: 32,
            height: 32,
            tile_size,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (5, 5),
        players: Vec::new(),
        owned: vec![AiEntitySummary {
            id: 10,
            owner: 1,
            kind: blocker,
            x: 10.5 * tile_size as f32,
            y: 5.5 * tile_size as f32,
            hp: 100,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: false,
        }],
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    }
}

#[test]
fn machine_gunner_below_half_health_requires_replacement() {
    let max_hp = config::unit_stats(EntityKind::MachineGunner)
        .expect("machine gunner stats")
        .hp;
    let half_or_above = max_hp.div_ceil(2);
    assert!(machine_gunner_meets_replacement_health(half_or_above, 50));
    assert!(!machine_gunner_meets_replacement_health(
        half_or_above - 1,
        50
    ));
}

#[test]
fn defensive_firing_lane_rejects_opaque_building_but_not_pump_jack() {
    let ts = config::TILE_SIZE as f32;
    let origin = (5.5 * ts, 5.5 * ts);
    assert!(!defensive_firing_lane_is_clear(
        &los_test_observation(EntityKind::Depot),
        None,
        origin,
        (1.0, 0.0),
        14.0,
    ));
    assert!(defensive_firing_lane_is_clear(
        &los_test_observation(EntityKind::PumpJack),
        None,
        origin,
        (1.0, 0.0),
        14.0,
    ));
}

#[test]
fn defensive_firing_sector_rejects_a_building_masking_its_flank() {
    let mut observation = los_test_observation(EntityKind::Depot);
    let ts = observation.map.tile_size as f32;
    let origin = (5.5 * ts, 5.5 * ts);
    observation.owned[0].y = 7.0 * ts;
    assert!(defensive_firing_lane_is_clear(
        &observation,
        None,
        origin,
        (1.0, 0.0),
        14.0,
    ));
    assert!(!defensive_firing_sector_is_clear(
        &observation,
        None,
        origin,
        (1.0, 0.0),
        14.0,
    ));
}

#[test]
fn defensive_assignment_shifts_out_from_behind_building() {
    let observation = los_test_observation(EntityKind::Depot);
    let ts = observation.map.tile_size as f32;
    let original = DefensiveLineAssignment {
        unit_id: 20,
        x: 5.5 * ts,
        y: 5.5 * ts,
    };
    let adjusted = clear_firing_assignment(
        &observation,
        None,
        original,
        EnemyBaseFact {
            player_id: 2,
            start_tile: (25, 5),
            x: 25.5 * ts,
            y: 5.5 * ts,
        },
        14.0,
    )
    .expect("nearby clear firing position");
    assert_ne!((adjusted.x, adjusted.y), (original.x, original.y));
    let direction = normalized_direction((adjusted.x, adjusted.y), (25.5 * ts, 5.5 * ts))
        .expect("adjusted assignment direction");
    assert!(defensive_firing_sector_is_clear(
        &observation,
        None,
        (adjusted.x, adjusted.y),
        direction,
        14.0,
    ));
}

#[test]
fn machine_gunner_screen_moves_in_front_of_forward_factory() {
    let observation = los_test_observation(EntityKind::Factory);
    let ts = observation.map.tile_size as f32;
    let assignment = DefensiveLineAssignment {
        unit_id: 20,
        x: 5.5 * ts,
        y: 5.5 * ts,
    };
    let adjusted = clear_machine_gunner_screen_assignment(
        &observation,
        None,
        assignment,
        EnemyBaseFact {
            player_id: 2,
            start_tile: (25, 5),
            x: 25.5 * ts,
            y: 5.5 * ts,
        },
    )
    .expect("clear Machine Gunner position in front of Factory");

    assert!(adjusted.x > observation.owned[0].x);
    assert_eq!(adjusted.y, assignment.y);
}

#[test]
fn infantry_at_home_defends_a_forward_building_under_attack() {
    let mut observation = los_test_observation(EntityKind::Factory);
    let ts = observation.map.tile_size as f32;
    observation.owned[0].x = 24.5 * ts;
    observation.owned[0].y = 5.5 * ts;
    for (id, kind) in [(20, EntityKind::Rifleman), (21, EntityKind::MachineGunner)] {
        observation.owned.push(AiEntitySummary {
            id,
            owner: 1,
            kind,
            x: 5.5 * ts,
            y: 5.5 * ts,
            hp: config::unit_stats(kind).expect("infantry stats").hp,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        });
    }
    observation.visible_enemies.push(AiEntitySummary {
        id: 30,
        owner: 2,
        kind: EntityKind::Rifleman,
        x: 27.5 * ts,
        y: 5.5 * ts,
        hp: 100,
        state: AiEntityState::Attack,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: Some(10),
        free_for_combat: true,
    });

    assert_eq!(local_defense_target(&observation), Some(30));
    assert_eq!(local_defense_units(&observation, &[20, 21]), vec![20, 21]);
}

#[test]
fn first_machine_gunner_reserves_a_flank_slot_in_two_unit_formation() {
    let mut observation = los_test_observation(EntityKind::Factory);
    let tile_size = observation.map.tile_size as f32;
    observation.resources.push(AiResourceSummary {
        id: 100,
        kind: EntityKind::Steel,
        x: 7.5 * tile_size,
        y: 5.5 * tile_size,
        remaining: 625,
    });
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (25, 5),
        x: 25.5 * observation.map.tile_size as f32,
        y: 5.5 * observation.map.tile_size as f32,
    };
    let centered =
        main_steel_defensive_line_assignments(&observation, &[20], enemy_base, 6.0, 4.5, 1)
            .expect("centered assignment")[0];
    let reserved =
        main_steel_defensive_line_assignments(&observation, &[20], enemy_base, 6.0, 4.5, 2)
            .expect("reserved flank assignment")[0];
    let offset_tiles = dist2(centered.x, centered.y, reserved.x, reserved.y).sqrt()
        / observation.map.tile_size as f32;

    assert!((offset_tiles - 2.25).abs() < 0.001);
}

#[test]
fn home_rifle_coverage_uses_wide_fixed_columns_and_deeper_second_rank() {
    let mut observation = los_test_observation(EntityKind::Factory);
    observation.owned.clear();
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (25, 5),
        x: 25.5 * observation.map.tile_size as f32,
        y: 5.5 * observation.map.tile_size as f32,
    };
    let assignments =
        home_rifleman_coverage_assignments(&observation, None, &[40, 10, 30, 20], enemy_base)
            .expect("coverage assignments");
    let tile_size = observation.map.tile_size as f32;

    assert_eq!(
        assignments
            .iter()
            .map(|slot| slot.unit_id)
            .collect::<Vec<_>>(),
        vec![10, 20, 30, 40]
    );
    assert!(
        (dist2(
            assignments[0].x,
            assignments[0].y,
            assignments[1].x,
            assignments[1].y
        )
        .sqrt()
            / tile_size
            - HOME_RIFLE_RANK_DEPTH_TILES)
            .abs()
            < 0.001
    );
    assert!(
        (dist2(
            assignments[0].x,
            assignments[0].y,
            assignments[2].x,
            assignments[2].y
        )
        .sqrt()
            / tile_size
            - HOME_RIFLE_LATERAL_SPACING_TILES)
            .abs()
            < 0.001
    );
}

#[test]
fn river_defensive_pocket_matches_the_approved_six_unit_shape() {
    let mut observation = los_test_observation(EntityKind::Factory);
    observation.map.width = 126;
    observation.map.height = 126;
    observation.own_start_tile = (9, 9);
    observation.owned.clear();
    let tile_size = observation.map.tile_size as f32;
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (116, 116),
        x: 116.5 * tile_size,
        y: 116.5 * tile_size,
    };

    let rifles =
        home_defensive_pocket_rifle_assignments(&observation, None, &[40, 10, 30, 20], enemy_base)
            .expect("Rifle pocket assignments");
    let machine_gunners =
        defensive_pocket_machine_gunner_assignments(&observation, None, &[60, 50], enemy_base)
            .expect("Machine Gunner pocket assignments");

    assert_eq!(
        rifles.iter().map(|slot| slot.unit_id).collect::<Vec<_>>(),
        vec![10, 20, 30, 40]
    );
    for (assignment, expected) in
        rifles
            .iter()
            .zip([(10.5, 14.5), (14.5, 10.5), (15.8, 14.7), (14.7, 15.8)])
    {
        assert!((assignment.x / tile_size - expected.0).abs() < 0.03);
        assert!((assignment.y / tile_size - expected.1).abs() < 0.03);
    }
    for (assignment, expected) in machine_gunners.iter().zip([(16.39, 13.21), (13.21, 16.39)]) {
        assert!((assignment.x / tile_size - expected.0).abs() < 0.03);
        assert!((assignment.y / tile_size - expected.1).abs() < 0.03);
    }

    let anchor = tile_center(observation.own_start_tile, observation.map.tile_size);
    let direction = normalized_direction(anchor, (enemy_base.x, enemy_base.y)).unwrap();
    let forward = |assignment: &DefensiveLineAssignment| {
        ((assignment.x - anchor.0) * direction.0 + (assignment.y - anchor.1) * direction.1)
            / tile_size
    };
    assert!(forward(&machine_gunners[0]) < forward(&rifles[2]));
    assert!(forward(&machine_gunners[1]) < forward(&rifles[3]));
}

#[test]
fn first_machine_gunner_keeps_its_side_when_the_mirrored_partner_arrives() {
    let mut observation = los_test_observation(EntityKind::Factory);
    observation.map.width = 126;
    observation.map.height = 126;
    observation.own_start_tile = (9, 9);
    observation.owned.clear();
    let tile_size = observation.map.tile_size as f32;
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (116, 116),
        x: 116.5 * tile_size,
        y: 116.5 * tile_size,
    };

    let first =
        defensive_pocket_machine_gunner_assignments(&observation, None, &[50], enemy_base).unwrap();
    let pair =
        defensive_pocket_machine_gunner_assignments(&observation, None, &[60, 50], enemy_base)
            .unwrap();

    assert_eq!(first[0].unit_id, pair[0].unit_id);
    assert_eq!((first[0].x, first[0].y), (pair[0].x, pair[0].y));
    let anchor = tile_center(observation.own_start_tile, observation.map.tile_size);
    let direction = normalized_direction(anchor, (enemy_base.x, enemy_base.y)).unwrap();
    let perpendicular = (-direction.1, direction.0);
    let projection = |assignment: &DefensiveLineAssignment, axis: (f32, f32)| {
        (assignment.x - anchor.0) * axis.0 + (assignment.y - anchor.1) * axis.1
    };
    assert!((projection(&pair[0], direction) - projection(&pair[1], direction)).abs() < 0.001);
    assert!(
        (projection(&pair[0], perpendicular) + projection(&pair[1], perpendicular)).abs() < 0.001
    );
}

fn river_pocket_observation() -> (AiObservation, EnemyBaseFact) {
    let mut observation = los_test_observation(EntityKind::Factory);
    observation.map.width = 126;
    observation.map.height = 126;
    observation.own_start_tile = (9, 9);
    observation.owned.clear();
    let tile_size = observation.map.tile_size as f32;
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (116, 116),
        x: 116.5 * tile_size,
        y: 116.5 * tile_size,
    };
    (observation, enemy_base)
}

/// An open 126x126 map with starts at (9,9) and (116,116), forest on `forest` and rock on `rock`.
fn forest_test_analysis(forest: &[(u32, u32)], rock: &[(u32, u32)]) -> AiMapAnalysis {
    use rts_sim::protocol::{MapInfo, MapTile, PlayerStart, StartPayload};
    let (width, height) = (126_u32, 126_u32);
    let mut terrain = vec![rts_rules::terrain::MAP_TERRAIN_GRASS; (width * height) as usize];
    for &(x, y) in rock {
        terrain[(y * width + x) as usize] = rts_rules::terrain::MAP_TERRAIN_ROCK;
    }
    let tiles = |list: &[(u32, u32)]| list.iter().map(|&(x, y)| MapTile { x, y }).collect();
    let start = StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        match_run_id: None,
        capabilities: Default::default(),
        diagnostics: Default::default(),
        replay: None,
        lab: None,
        observer_view: None,
        tick: 0,
        map: MapInfo {
            width,
            height,
            tile_size: config::TILE_SIZE,
            elevation: vec![0; terrain.len()],
            sun: None,
            terrain,
            resources: Vec::new(),
            doodads: Vec::new(),
            concealment_tiles: tiles(forest),
            no_vehicle_tiles: tiles(forest),
            no_building_tiles: tiles(forest),
            no_entrenchment_tiles: Vec::new(),
            damage_reduction_tiles: tiles(forest),
            slow_movement_tiles: Vec::new(),
        },
        players: [(9_u32, 9_u32), (116, 116)]
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| {
                let id = index as u32 + 1;
                PlayerStart {
                    id,
                    team_id: id,
                    faction_id: "kriegsia".to_string(),
                    name: format!("P{id}"),
                    color: format!("#{id}{id}{id}"),
                    is_ai: true,
                    start_tile_x: x,
                    start_tile_y: y,
                }
            })
            .collect(),
    };
    AiMapAnalysis::analyze(&start)
}

fn block(xs: std::ops::RangeInclusive<u32>, ys: std::ops::RangeInclusive<u32>) -> Vec<(u32, u32)> {
    ys.flat_map(|y| xs.clone().map(move |x| (x, y))).collect()
}

fn post_tiles(assignments: &[DefensiveLineAssignment], tile_size: f32) -> Vec<(u32, u32)> {
    assignments
        .iter()
        .map(|post| ((post.x / tile_size) as u32, (post.y / tile_size) as u32))
        .collect()
}

#[test]
fn pocket_riflemen_hold_a_nearby_forest_edge_only_when_asked() {
    let (observation, enemy_base) = river_pocket_observation();
    let tile_size = observation.map.tile_size as f32;
    // A small wood two tiles off the first pocket post (tile (10,14)), on its open flank.
    let analysis = forest_test_analysis(&block(7..=8, 15..=17), &[]);
    let units = [10, 20, 30, 40];
    let open = pocket_rifle_assignments(&observation, Some(&analysis), &units, enemy_base, false)
        .expect("open pocket");
    let forest = pocket_rifle_assignments(&observation, Some(&analysis), &units, enemy_base, true)
        .expect("forest pocket");

    let open_tiles = post_tiles(&open, tile_size);
    let forest_tiles = post_tiles(&forest, tile_size);
    assert_eq!(open_tiles[0], (10, 14));
    // The nearest forest tile that is no more than a tile farther back and still watches the post
    // and the ground beyond it.
    assert_eq!(forest_tiles[0], (8, 15));
    assert_eq!(
        &forest_tiles[1..],
        &open_tiles[1..],
        "posts without a wood nearby stay put"
    );
}

#[test]
fn pocket_riflemen_never_fall_back_into_a_wood_behind_their_post() {
    let (observation, enemy_base) = river_pocket_observation();
    let tile_size = observation.map.tile_size as f32;
    // Within four tiles of the first post, but every tile is more than a tile farther from the
    // threat than the post itself.
    let analysis = forest_test_analysis(&block(7..=8, 12..=13), &[]);
    let units = [10, 20, 30, 40];
    let open =
        pocket_rifle_assignments(&observation, Some(&analysis), &units, enemy_base, false).unwrap();
    let forest =
        pocket_rifle_assignments(&observation, Some(&analysis), &units, enemy_base, true).unwrap();
    assert_eq!(post_tiles(&forest, tile_size), post_tiles(&open, tile_size));
}

#[test]
fn sight_lines_count_forest_after_the_origin_and_stop_at_rock() {
    let (observation, _) = river_pocket_observation();
    let tile = observation.map.tile_size as f32;
    let center = |x: u32, y: u32| ((x as f32 + 0.5) * tile, (y as f32 + 0.5) * tile);
    let analysis = forest_test_analysis(&block(20..=23, 30..=30), &[(40, 30)]);
    // Starting inside the wood: the origin tile is not counted, the three after it are.
    assert_eq!(
        forest_tiles_on_sight_line(&observation, &analysis, center(20, 30), center(30, 30)),
        Some(3)
    );
    assert_eq!(
        forest_tiles_on_sight_line(&observation, &analysis, center(10, 30), center(30, 30)),
        Some(4)
    );
    assert_eq!(
        forest_tiles_on_sight_line(&observation, &analysis, center(30, 30), center(45, 30)),
        None
    );
}

#[test]
fn four_gun_line_keeps_the_pocket_posts_and_adds_a_wing_on_each_side() {
    let (observation, enemy_base) = river_pocket_observation();
    let tile_size = observation.map.tile_size as f32;
    let pair =
        defensive_pocket_machine_gunner_assignments(&observation, None, &[60, 50], enemy_base)
            .unwrap();
    let line =
        wide_pocket_machine_gunner_assignments(&observation, None, &[80, 60, 70, 50], enemy_base)
            .unwrap();

    assert_eq!(
        line.iter().map(|post| post.unit_id).collect::<Vec<_>>(),
        vec![50, 60, 70, 80]
    );
    for (wide, pocket) in line.iter().zip(&pair) {
        assert_eq!((wide.x, wide.y), (pocket.x, pocket.y));
    }
    let anchor = tile_center(observation.own_start_tile, observation.map.tile_size);
    let direction = normalized_direction(anchor, (enemy_base.x, enemy_base.y)).unwrap();
    let perpendicular = (-direction.1, direction.0);
    let frame = |post: &DefensiveLineAssignment| {
        let offset = (post.x - anchor.0, post.y - anchor.1);
        (
            (offset.0 * direction.0 + offset.1 * direction.1) / tile_size,
            (offset.0 * perpendicular.0 + offset.1 * perpendicular.1) / tile_size,
        )
    };
    for (post, expected) in line
        .iter()
        .zip([(7.5, -2.25), (7.5, 2.25), (8.0, -3.75), (8.0, 3.75)])
    {
        let (forward, lateral) = frame(post);
        assert!(
            (forward - expected.0).abs() < 0.01,
            "{forward} vs {expected:?}"
        );
        assert!(
            (lateral - expected.1).abs() < 0.01,
            "{lateral} vs {expected:?}"
        );
    }
    for post in &line {
        for other in line.iter().filter(|other| other.unit_id != post.unit_id) {
            assert!(dist2(post.x, post.y, other.x, other.y) >= squared(1.45 * tile_size));
        }
        // An attacker standing off the centre guns at Machine Gunner range is in every dug-in
        // gun's reach.
        let (forward, lateral) = frame(post);
        assert!((14.0 - forward).hypot(lateral) <= 7.1);
    }
}

#[test]
fn a_gun_on_its_post_keeps_it_when_another_gun_falls() {
    let (mut observation, enemy_base) = river_pocket_observation();
    let full =
        wide_pocket_machine_gunner_assignments(&observation, None, &[50, 60, 70, 80], enemy_base)
            .unwrap();
    // Gun 50 died on the first post; the others are dug in on theirs and a replacement walks up
    // from the Depot.
    for post in full.iter().skip(1) {
        observation.owned.push(combat_unit(
            post.unit_id,
            EntityKind::MachineGunner,
            post.x,
            post.y,
        ));
    }
    let depot = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation
        .owned
        .push(combat_unit(90, EntityKind::MachineGunner, depot.0, depot.1));

    let line =
        wide_pocket_machine_gunner_assignments(&observation, None, &[60, 70, 80, 90], enemy_base)
            .unwrap();

    let post_of = |posts: &[DefensiveLineAssignment], id: u32| {
        posts
            .iter()
            .find(|post| post.unit_id == id)
            .map(|post| (post.x, post.y))
    };
    for id in [60, 70, 80] {
        assert_eq!(post_of(&line, id), post_of(&full, id), "gun {id} moved");
    }
    assert_eq!(post_of(&line, 90), post_of(&full, 50));
}

#[test]
fn crossroads_starts_use_the_approved_wall_aware_pocket_rotation() {
    let mut observation = los_test_observation(EntityKind::Factory);
    observation.map.width = 126;
    observation.map.height = 126;
    observation.own_start_tile = (47, 8);
    observation.players = vec![
        AiPlayerSummary {
            id: 1,
            team_id: 1,
            start_tile: (47, 8),
            is_ai: true,
            is_alive: true,
        },
        AiPlayerSummary {
            id: 2,
            team_id: 2,
            start_tile: (117, 78),
            is_ai: true,
            is_alive: true,
        },
    ];
    observation.owned.clear();
    let tile_size = observation.map.tile_size as f32;
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (117, 78),
        x: 117.5 * tile_size,
        y: 78.5 * tile_size,
    };
    let (_, direction) =
        defensive_pocket_basis(&observation, None, enemy_base).expect("P1 wall-aware orientation");
    let expected = normalized_direction((0.0, 0.0), (-2.0, 1.0)).unwrap();
    assert!((direction.0 - expected.0).abs() < 0.0001);
    assert!((direction.1 - expected.1).abs() < 0.0001);
    let rifles =
        home_defensive_pocket_rifle_assignments(&observation, None, &[40, 10, 30, 20], enemy_base)
            .expect("P1 Rifle pocket assignments");
    let machine_gunners =
        defensive_pocket_machine_gunner_assignments(&observation, None, &[60, 50], enemy_base)
            .expect("P1 Machine Gunner pocket assignments");
    for (assignment, expected) in rifles.iter().zip([
        (42.43, 7.87),
        (44.96, 12.93),
        (40.56, 12.84),
        (39.86, 11.45),
    ]) {
        assert!((assignment.x / tile_size - expected.0).abs() < 0.03);
        assert!((assignment.y / tile_size - expected.1).abs() < 0.03);
    }
    for (assignment, expected) in machine_gunners.iter().zip([(41.80, 13.87), (39.79, 9.84)]) {
        assert!((assignment.x / tile_size - expected.0).abs() < 0.03);
        assert!((assignment.y / tile_size - expected.1).abs() < 0.03);
    }

    observation.player_id = 2;
    observation.own_start_tile = (117, 78);
    let enemy_base = EnemyBaseFact {
        player_id: 1,
        start_tile: (47, 8),
        x: 47.5 * tile_size,
        y: 8.5 * tile_size,
    };
    let (_, direction) =
        defensive_pocket_basis(&observation, None, enemy_base).expect("P2 wall-aware orientation");
    let expected = normalized_direction((0.0, 0.0), (-5.0, 7.0)).unwrap();
    assert!((direction.0 - expected.0).abs() < 0.0001);
    assert!((direction.1 - expected.1).abs() < 0.0001);
    let rifles =
        home_defensive_pocket_rifle_assignments(&observation, None, &[40, 10, 30, 20], enemy_base)
            .expect("P2 Rifle pocket assignments");
    let machine_gunners =
        defensive_pocket_machine_gunner_assignments(&observation, None, &[60, 50], enemy_base)
            .expect("P2 Machine Gunner pocket assignments");
    for (assignment, expected) in rifles.iter().zip([
        (112.73, 80.31),
        (117.33, 83.60),
        (113.40, 85.59),
        (112.13, 84.68),
    ]) {
        assert!((assignment.x / tile_size - expected.0).abs() < 0.03);
        assert!((assignment.y / tile_size - expected.1).abs() < 0.03);
    }
    for (assignment, expected) in machine_gunners
        .iter()
        .zip([(114.97, 85.91), (111.31, 83.30)])
    {
        assert!((assignment.x / tile_size - expected.0).abs() < 0.03);
        assert!((assignment.y / tile_size - expected.1).abs() < 0.03);
    }
}

#[test]
fn planned_factory_is_part_of_the_local_defense_envelope() {
    let mut observation = los_test_observation(EntityKind::Depot);
    let ts = observation.map.tile_size as f32;
    observation
        .pending_builds
        .push(AiBuildIntent::to_site(99, EntityKind::Factory, 18, 4));
    let stats = config::building_stats(EntityKind::Factory).expect("Factory stats");
    let right_edge = (18 + stats.foot_w) as f32 * ts;
    observation.visible_enemies.push(AiEntitySummary {
        id: 30,
        owner: 2,
        kind: EntityKind::Rifleman,
        x: right_edge + 2.0 * ts,
        y: 6.5 * ts,
        hp: 100,
        state: AiEntityState::Attack,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    });

    assert_eq!(
        local_defense_contact(&observation)
            .expect("planned site contact")
            .target_ids,
        vec![30]
    );
    assert_eq!(
        local_defense_target(&observation),
        None,
        "legacy profiles must not inherit Jeff's planned-site geometry"
    );
}

#[test]
fn incomplete_factory_is_part_of_the_local_defense_envelope() {
    let mut observation = los_test_observation(EntityKind::Depot);
    let ts = observation.map.tile_size as f32;
    let (factory_x, factory_y) =
        building_center((18, 4), EntityKind::Factory, observation.map.tile_size)
            .expect("Factory footprint");
    observation.owned.push(AiEntitySummary {
        id: 20,
        owner: 1,
        kind: EntityKind::Factory,
        x: factory_x,
        y: factory_y,
        hp: 50,
        state: AiEntityState::Build,
        is_complete: false,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    });
    let stats = config::building_stats(EntityKind::Factory).expect("Factory stats");
    observation.visible_enemies.push(AiEntitySummary {
        id: 30,
        owner: 2,
        kind: EntityKind::Rifleman,
        x: (18 + stats.foot_w) as f32 * ts + 2.0 * ts,
        y: factory_y,
        hp: 100,
        state: AiEntityState::Attack,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    });

    assert_eq!(
        local_defense_contact(&observation)
            .expect("incomplete building contact")
            .target_ids,
        vec![30]
    );
}

#[test]
fn planned_factory_does_not_move_the_standing_rifle_formation() {
    let mut observation = los_test_observation(EntityKind::Depot);
    observation.owned.clear();
    observation
        .pending_builds
        .push(AiBuildIntent::to_site(99, EntityKind::Factory, 16, 4));
    assert!(defensive_formation_sites(&observation).is_empty());
    assert_eq!(defended_building_sites(&observation, true).len(), 1);
}

#[test]
fn standing_rifle_formation_ignores_extractors() {
    let mut observation = los_test_observation(EntityKind::Depot);
    observation.owned.push(AiEntitySummary {
        id: 21,
        owner: 1,
        kind: EntityKind::PumpJack,
        x: 20.5 * observation.map.tile_size as f32,
        y: 20.5 * observation.map.tile_size as f32,
        hp: 100,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: false,
    });

    assert_eq!(defensive_formation_sites(&observation).len(), 1);
    assert_eq!(defended_building_sites(&observation, false).len(), 2);
}

#[test]
fn standing_rifle_line_keeps_three_fifths_on_the_primary_approach() {
    let assignments = (0..10)
        .map(|index| primary_weighted_approach_slot(index, 10, 3).0)
        .collect::<Vec<_>>();

    assert_eq!(
        assignments
            .iter()
            .filter(|approach| **approach == 0)
            .count(),
        6
    );
    assert_eq!(
        assignments
            .iter()
            .filter(|approach| **approach == 1)
            .count(),
        2
    );
    assert_eq!(
        assignments
            .iter()
            .filter(|approach| **approach == 2)
            .count(),
        2
    );
}

#[test]
fn strongest_local_sector_wins_over_a_lone_flanker() {
    let mut observation = los_test_observation(EntityKind::Depot);
    let ts = observation.map.tile_size as f32;
    for (id, x, y) in [
        (30, 11.5 * ts, 5.5 * ts),
        (31, 12.5 * ts, 5.5 * ts),
        (32, 5.5 * ts, 11.5 * ts),
    ] {
        observation.visible_enemies.push(AiEntitySummary {
            id,
            owner: 2,
            kind: EntityKind::Rifleman,
            x,
            y,
            hp: 100,
            state: AiEntityState::Attack,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        });
    }

    let contact = local_defense_contact(&observation).expect("local contact");
    assert_eq!(contact.target_ids, vec![30, 31]);
    assert!(
        contact.intercept.0 <= contact.centroid.0,
        "contact: {contact:?}"
    );
    assert!((contact.intercept.1 - contact.centroid.1).abs() < f32::EPSILON);
}

fn stationary_defense_observation() -> AiObservation {
    let mut observation = los_test_observation(EntityKind::Factory);
    let ts = observation.map.tile_size as f32;
    // The Factory sits off the firing line so the enemy is inside the defended envelope.
    observation.owned[0].x = 16.5 * ts;
    observation.owned[0].y = 9.5 * ts;
    observation
        .owned
        .push(combat_unit(40, EntityKind::Tank, 7.5 * ts, 5.5 * ts));
    observation.visible_enemies.push(AiEntitySummary {
        owner: 2,
        state: AiEntityState::Attack,
        target_id: Some(10),
        ..combat_unit(30, EntityKind::Rifleman, 20.5 * ts, 5.5 * ts)
    });
    observation
}

fn combat_unit(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
    AiEntitySummary {
        id,
        owner: 1,
        kind,
        x,
        y,
        hp: config::unit_stats(kind).expect("unit stats").hp,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    }
}

fn stationary_defense_commands(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    defenders: &[u32],
) -> Vec<Command> {
    let facts = AiFacts::from_observation(observation);
    let mut actions = AiActionContext::new(&facts, SpendBudget::new(1000, 1000, 20, 80));
    respond_to_local_incident(&mut actions, observation, memory, defenders, None, true);
    actions.into_commands()
}

#[test]
fn spotted_defending_tank_holds_at_stationary_range() {
    let mut observation = stationary_defense_observation();
    let ts = observation.map.tile_size as f32;
    observation
        .owned
        .push(combat_unit(41, EntityKind::Rifleman, 6.5 * ts, 5.5 * ts));
    let mut memory = AiDecisionMemory::default();
    let commands = stationary_defense_commands(&observation, &mut memory, &[40, 41]);
    assert!(
        commands.iter().any(
            |command| matches!(command, Command::HoldPosition { units, .. } if units == &[40])
        ),
        "{commands:?}"
    );
    assert!(!commands
        .iter()
        .any(|command| matches!(command, Command::Attack { units, .. } if units.contains(&40))));
}

#[test]
fn unspotted_defending_tank_closes_to_its_own_sight_before_holding() {
    let observation = stationary_defense_observation();
    let ts = observation.map.tile_size as f32;
    let mut memory = AiDecisionMemory::default();
    let commands = stationary_defense_commands(&observation, &mut memory, &[40]);
    let park = commands.iter().find_map(|command| match command {
        Command::Move { units, x, y, .. } if units == &[40] => Some((*x, *y)),
        _ => None,
    });
    let (x, y) = park.unwrap_or_else(|| panic!("tank should close in: {commands:?}"));
    let distance = dist2(x, y, 20.5 * ts, 5.5 * ts).sqrt() / ts;
    assert!((distance - 9.0).abs() < 0.01, "parked {distance} tiles out");
}

#[test]
fn building_losing_hp_in_fog_sends_defenders_to_search() {
    let mut observation = stationary_defense_observation();
    observation.visible_enemies.clear();
    let mut memory = AiDecisionMemory::default();
    assert!(stationary_defense_commands(&observation, &mut memory, &[40]).is_empty());

    observation.tick += 9;
    observation.owned[0].hp -= 20;
    let commands = stationary_defense_commands(&observation, &mut memory, &[40]);
    let factory = (observation.owned[0].x, observation.owned[0].y);
    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::AttackMove { units, x, y, .. } if units == &[40] && (*x, *y) == factory
        )),
        "{commands:?}"
    );
}

#[test]
fn defending_tank_does_not_park_behind_a_building() {
    let mut observation = stationary_defense_observation();
    let ts = observation.map.tile_size as f32;
    // A Barracks between the Tank and the raider blocks the shot, so the Tank must move to a
    // point with a clear line of fire instead of holding.
    observation.owned.push(AiEntitySummary {
        id: 11,
        kind: EntityKind::Barracks,
        x: 14.5 * ts,
        y: 5.5 * ts,
        free_for_combat: false,
        ..combat_unit(11, EntityKind::Rifleman, 0.0, 0.0)
    });
    observation
        .owned
        .push(combat_unit(41, EntityKind::Rifleman, 6.5 * ts, 5.5 * ts));
    let mut memory = AiDecisionMemory::default();
    let commands = stationary_defense_commands(&observation, &mut memory, &[40, 41]);
    assert!(!commands
        .iter()
        .any(|command| matches!(command, Command::HoldPosition { units, .. } if units == &[40])));
    let park = commands.iter().find_map(|command| match command {
        Command::Move { units, x, y, .. } if units == &[40] => Some((*x, *y)),
        _ => None,
    });
    let (x, y) = park.unwrap_or_else(|| panic!("tank should reposition: {commands:?}"));
    assert!(
        (y - 5.5 * ts).abs() > ts,
        "park point ({x}, {y}) is still on the blocked line"
    );
}

fn raid_observation(raiders: u32) -> AiObservation {
    let mut observation = stationary_defense_observation();
    let ts = observation.map.tile_size as f32;
    observation
        .owned
        .retain(|unit| unit.kind != EntityKind::Tank);
    observation
        .owned
        .push(combat_unit(41, EntityKind::Rifleman, 10.5 * ts, 5.5 * ts));
    observation
        .owned
        .push(combat_unit(42, EntityKind::Rifleman, 10.5 * ts, 7.5 * ts));
    let raider = observation.visible_enemies[0].clone();
    observation.visible_enemies = (0..raiders)
        .map(|index| AiEntitySummary {
            id: 30 + index,
            y: (5.5 + index as f32 * 0.5) * ts,
            ..raider.clone()
        })
        .collect();
    observation
}

fn attackers(commands: &[Command]) -> Vec<u32> {
    let mut units: Vec<u32> = commands
        .iter()
        .filter_map(|command| match command {
            Command::Attack { units, .. } => Some(units.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    units.sort_unstable();
    units
}

#[test]
fn a_raid_on_buildings_is_answered_even_below_two_to_one() {
    let mut observation = raid_observation(6);
    let mut memory = AiDecisionMemory::default();
    // Two Riflemen cannot reach twice the value of six, so a probe is left alone.
    let quiet = stationary_defense_commands(&observation, &mut memory, &[41, 42]);
    assert!(attackers(&quiet).is_empty(), "{quiet:?}");
    assert!(attack_movers(&quiet).is_empty(), "{quiet:?}");

    // Once the raiders start killing buildings, both go to the attacked building. They
    // attack-move there rather than attack a raider, so they cannot chase it out of the base.
    observation.tick += 9;
    observation.owned[0].hp -= 20;
    let raid = stationary_defense_commands(&observation, &mut memory, &[41, 42]);
    assert_eq!(attack_movers(&raid), vec![41, 42], "{raid:?}");
    assert!(attackers(&raid).is_empty(), "{raid:?}");
}

#[test]
fn machine_gunners_are_never_sent_out_as_spotters() {
    let mut observation = stationary_defense_observation();
    let ts = observation.map.tile_size as f32;
    observation.owned.push(combat_unit(
        45,
        EntityKind::MachineGunner,
        8.5 * ts,
        7.5 * ts,
    ));
    let mut memory = AiDecisionMemory::default();
    for _ in 0..3 {
        let commands = stationary_defense_commands(&observation, &mut memory, &[40, 45]);
        assert!(
            !commands.iter().any(
                |command| matches!(command, Command::Move { units, .. } if units.contains(&45))
            ),
            "{commands:?}"
        );
        observation.tick += 9;
    }
}

fn attack_movers(commands: &[Command]) -> Vec<u32> {
    let mut units: Vec<u32> = commands
        .iter()
        .filter_map(|command| match command {
            Command::AttackMove { units, .. } => Some(units.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    units.sort_unstable();
    units
}

fn ordered_units(commands: &[Command]) -> BTreeSet<u32> {
    commands
        .iter()
        .flat_map(|command| match command {
            Command::Move { units, .. }
            | Command::AttackMove { units, .. }
            | Command::Attack { units, .. }
            | Command::HoldPosition { units, .. } => units.clone(),
            _ => Vec::new(),
        })
        .collect()
}

/// Raid with Riflemen 41 and 42 out of reach and 43 within 5 tiles of a raider. With `dug_in`,
/// 41 and 42 have stood still long enough after Entrenchment to be in trenches.
fn raid_with_rifle_in_reach(in_reach: bool, dug_in: bool) -> Vec<Command> {
    let mut observation = raid_observation(3);
    let ts = observation.map.tile_size as f32;
    if in_reach {
        observation
            .owned
            .push(combat_unit(43, EntityKind::Rifleman, 16.5 * ts, 5.5 * ts));
    }
    let mut memory = AiDecisionMemory::default();
    if dug_in {
        observation.upgrades.push(UpgradeKind::Entrenchment);
        memory.sync_defender_posture(&observation);
        observation.tick += rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS;
        memory.sync_defender_posture(&observation);
    }
    let defenders: Vec<u32> = if in_reach {
        vec![41, 42, 43]
    } else {
        vec![41, 42]
    };
    stationary_defense_commands(&observation, &mut memory, &defenders);
    observation.tick += 9;
    observation.owned[0].hp -= 20;
    stationary_defense_commands(&observation, &mut memory, &defenders)
}

#[test]
fn a_rifleman_already_in_reach_of_a_raid_fights_where_it_stands() {
    let commands = raid_with_rifle_in_reach(true, false);
    assert!(!ordered_units(&commands).contains(&43), "{commands:?}");
    assert_eq!(attack_movers(&commands), vec![41, 42], "{commands:?}");
}

#[test]
fn dug_in_riflemen_keep_their_trenches_while_someone_reaches_the_raid() {
    let commands = raid_with_rifle_in_reach(true, true);
    let ordered = ordered_units(&commands);
    assert!(
        !ordered.contains(&41) && !ordered.contains(&42),
        "{commands:?}"
    );
}

#[test]
fn dug_in_riflemen_leave_their_trenches_when_nobody_reaches_the_raid() {
    let commands = raid_with_rifle_in_reach(false, true);
    assert_eq!(attack_movers(&commands), vec![41, 42], "{commands:?}");
}

#[test]
fn a_lone_raider_draws_a_bounded_response_and_leaves_the_trenches_alone() {
    let mut observation = raid_observation(1);
    let ts = observation.map.tile_size as f32;
    for id in 43..=46 {
        observation.owned.push(combat_unit(
            id,
            EntityKind::Rifleman,
            9.5 * ts,
            (3.0 + (id - 43) as f32) * ts,
        ));
    }
    let mut memory = AiDecisionMemory::default();
    let defenders = [41, 42, 43, 44, 45, 46];
    stationary_defense_commands(&observation, &mut memory, &defenders);
    observation.tick += 9;
    observation.owned[0].hp -= 20;
    let commands = stationary_defense_commands(&observation, &mut memory, &defenders);
    assert_eq!(
        attack_movers(&commands).len(),
        4,
        "at least four: {commands:?}"
    );

    // Dug in and out of reach: a single raider never empties the trenches.
    let commands = raid_with_single_raider_and_trenches();
    assert!(attack_movers(&commands).is_empty(), "{commands:?}");
}

fn raid_with_single_raider_and_trenches() -> Vec<Command> {
    let mut observation = raid_observation(1);
    let mut memory = AiDecisionMemory::default();
    observation.upgrades.push(UpgradeKind::Entrenchment);
    memory.sync_defender_posture(&observation);
    observation.tick += rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS;
    memory.sync_defender_posture(&observation);
    stationary_defense_commands(&observation, &mut memory, &[41, 42]);
    observation.tick += 9;
    observation.owned[0].hp -= 20;
    stationary_defense_commands(&observation, &mut memory, &[41, 42])
}
