use super::*;
use crate::ai_core::observation::{AiBuildIntent, AiEconomy, AiResourceSummary};

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
fn home_rifle_envelope_forms_beyond_a_planned_factory_footprint() {
    let mut observation = los_test_observation(EntityKind::Depot);
    observation.owned.clear();
    observation
        .pending_builds
        .push(AiBuildIntent::to_site(99, EntityKind::Factory, 16, 4));
    let ts = observation.map.tile_size as f32;
    let enemy_base = EnemyBaseFact {
        player_id: 2,
        start_tile: (30, 6),
        x: 30.5 * ts,
        y: 6.5 * ts,
    };
    let assignments = home_rifleman_envelope_coverage_assignments(
        &observation,
        None,
        &[10, 20, 30, 40],
        enemy_base,
    )
    .expect("envelope assignments");
    let stats = config::building_stats(EntityKind::Factory).expect("Factory stats");
    let right_edge = (16 + stats.foot_w) as f32 * ts;

    assert!(assignments
        .iter()
        .all(|assignment| assignment.x > right_edge));
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
