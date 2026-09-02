    use super::*;
    use crate::ai_core::observation::{AiAbilitySummary, AiEconomy};
    use crate::ai_core::profiles::JEFFS_AI;

    fn target_test_entity(id: u32, kind: EntityKind, x: f32, y: f32) -> AiEntitySummary {
        AiEntitySummary {
            id,
            owner: if id == 1 { 1 } else { 2 },
            kind,
            x,
            y,
            hp: 300,
            state: AiEntityState::Idle,
            is_complete: true,
            production_queue_len: None,
            production_kind: None,
            latched_node: None,
            target_id: None,
            free_for_combat: true,
        }
    }

    fn regroup_test_observation(owned: Vec<AiEntitySummary>) -> AiObservation {
        AiObservation {
            player_id: 1,
            tick: 0,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size: 32,
            },
            economy: AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: owned.len() as u32,
                supply_cap: 100,
            },
            own_start_tile: (10, 10),
            players: Vec::new(),
            owned,
            resources: Vec::new(),
            visible_allies: Vec::new(),
            visible_enemies: Vec::new(),
            ability_states: Vec::new(),
            smokes: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        }
    }

    #[test]
    fn tank_push_selects_half_of_available_riflemen() {
        let riflemen = (1..=5)
            .map(|id| {
                let mut unit =
                    target_test_entity(id, EntityKind::Rifleman, id as f32 * 32.0, 320.0);
                unit.owner = 1;
                unit
            })
            .collect::<Vec<_>>();
        let observation = regroup_test_observation(riflemen);
        let memory = AiDecisionMemory::for_profile(&JEFFS_AI);

        let selected = select_rifle_escorts(&observation, &memory, (0.0, 320.0));

        assert_eq!(selected, vec![5]);
    }

    #[test]
    fn tank_push_caps_the_rifle_screen_at_six() {
        let riflemen = (1..=20)
            .map(|id| {
                let mut unit =
                    target_test_entity(id, EntityKind::Rifleman, id as f32 * 32.0, 320.0);
                unit.owner = 1;
                unit
            })
            .collect::<Vec<_>>();
        let observation = regroup_test_observation(riflemen);
        let memory = AiDecisionMemory::for_profile(&JEFFS_AI);

        let selected = select_rifle_escorts(&observation, &memory, (10.0 * 32.0, 320.0));

        assert_eq!(selected.len(), 6);
        assert!(selected.iter().all(|id| *id > 4));
    }

    #[test]
    fn rifle_screen_stays_two_tiles_ahead_of_the_tank_front() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };

        let point =
            rifle_screen_point((320.0, 640.0), (960.0, 640.0), map).expect("forward screen point");

        assert_eq!(point, (384.0, 640.0));
    }

    #[test]
    fn rifle_screen_spreads_escorts_across_the_tank_front() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };

        let points = rifle_screen_points((320.0, 640.0), (960.0, 640.0), map, 3);

        assert_eq!(points, vec![(384.0, 576.0), (384.0, 640.0), (384.0, 704.0)]);
    }

    #[test]
    fn large_rifle_screen_uses_a_staggered_second_rank() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };

        let points = rifle_screen_points((320.0, 640.0), (960.0, 640.0), map, 6);

        assert_eq!(
            points,
            vec![
                (384.0, 544.0),
                (384.0, 608.0),
                (384.0, 672.0),
                (384.0, 736.0),
                (336.0, 608.0),
                (336.0, 672.0),
            ]
        );
    }

    #[test]
    fn containment_uses_stationary_tank_range_and_forward_scout_vision() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };
        let policy = ExpansionContainmentPolicy {
            tank_standoff_tiles: 13.5,
            scout_trailing_tiles: 1.5,
            scout_forward_tiles: 2.0,
            flank_tiles: 5.0,
            contact_stop_tiles: 18.0,
            minimum_tanks_to_continue: 2,
            recovery_tanks_to_continue: 3,
            additional_tanks_per_repush: 1,
            repush_regroup_radius_tiles: 5.0,
        };
        let objective = (2_000.0, 1_000.0);
        let (tank, scout) = containment_points((200.0, 1_000.0), objective, map, policy).unwrap();

        let tank_distance = dist2(objective.0, objective.1, tank.0, tank.1).sqrt() / 32.0;
        let scout_distance = dist2(scout.0, scout.1, tank.0, tank.1).sqrt() / 32.0;
        assert!((tank_distance - 13.5).abs() < 0.001);
        assert!((scout_distance - 2.0).abs() < 0.001);
        assert!(scout.0 < objective.0);

        let trailing =
            scout_trailing_point((1_000.0, 1_000.0), (200.0, 1_000.0), objective, map, 1.5)
                .unwrap();
        assert_eq!((1_000.0 - trailing.0) / 32.0, 1.5);
    }

    #[test]
    fn containment_flank_rotates_with_the_players() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };
        let policy = JEFFS_AI.expansion_containment.unwrap();
        let world_size = map.width as f32 * map.tile_size as f32;
        let own_base = (200.0, 1_000.0);
        let objective = (2_000.0, 1_000.0);
        let original = containment_points(own_base, objective, map, policy).unwrap();
        let rotated = containment_points(
            (world_size - own_base.0, world_size - own_base.1),
            (world_size - objective.0, world_size - objective.1),
            map,
            policy,
        )
        .unwrap();

        for (actual, expected) in [
            (rotated.0 .0, world_size - original.0 .0),
            (rotated.0 .1, world_size - original.0 .1),
            (rotated.1 .0, world_size - original.1 .0),
            (rotated.1 .1, world_size - original.1 .1),
        ] {
            assert!((actual - expected).abs() < 0.001);
        }
    }

    #[test]
    fn each_repush_adds_one_tank_to_the_grouped_cohort() {
        let policy = JEFFS_AI.expansion_containment.unwrap();
        assert_eq!(containment_repush_tank_count(policy, 1), 3);
        assert_eq!(containment_repush_tank_count(policy, 2), 4);
        assert_eq!(containment_repush_tank_count(policy, 3), 5);
        assert_eq!(containment_regroup_radius_tiles(policy, 3), 3.0);
        assert_eq!(containment_regroup_radius_tiles(policy, 4), 4.5);
        assert_eq!(containment_regroup_radius_tiles(policy, 5), 6.0);
    }

    #[test]
    fn repush_selects_units_nearest_the_forward_rally_point() {
        let observation = regroup_test_observation(vec![
            target_test_entity(1, EntityKind::Tank, 100.0, 100.0),
            target_test_entity(2, EntityKind::Tank, 500.0, 500.0),
            target_test_entity(3, EntityKind::Tank, 515.0, 500.0),
            target_test_entity(4, EntityKind::Tank, 530.0, 500.0),
        ]);
        let mut candidates = vec![1, 2, 3, 4];

        select_nearest_units(&observation, &mut candidates, (520.0, 500.0), 3);

        assert_eq!(candidates, vec![3, 4, 2]);
    }

    #[test]
    fn repush_requires_a_compact_group_near_its_rally_point() {
        let compact = regroup_test_observation(vec![
            target_test_entity(1, EntityKind::Tank, 490.0, 500.0),
            target_test_entity(2, EntityKind::Tank, 510.0, 500.0),
            target_test_entity(3, EntityKind::Tank, 500.0, 510.0),
            target_test_entity(4, EntityKind::ScoutCar, 500.0, 490.0),
        ]);
        let scattered = regroup_test_observation(vec![
            target_test_entity(1, EntityKind::Tank, 300.0, 500.0),
            target_test_entity(2, EntityKind::Tank, 700.0, 500.0),
            target_test_entity(3, EntityKind::Tank, 500.0, 300.0),
            target_test_entity(4, EntityKind::ScoutCar, 500.0, 700.0),
        ]);
        let cohort = [1, 2, 3, 4];

        assert!(compact_group_near(
            &compact,
            &cohort,
            (500.0, 500.0),
            5.0 * 32.0
        ));
        assert!(!compact_group_near(
            &scattered,
            &cohort,
            (500.0, 500.0),
            5.0 * 32.0
        ));
    }

    #[test]
    fn anti_armor_threats_outrank_every_economic_target() {
        assert_eq!(outbound_wave_target_priority(EntityKind::Tank), 0);
        assert_eq!(outbound_wave_target_priority(EntityKind::AntiTankGun), 0);
        assert_eq!(outbound_wave_target_priority(EntityKind::Panzerfaust), 0);
        assert!(
            outbound_wave_target_priority(EntityKind::MachineGunner)
                > outbound_wave_target_priority(EntityKind::Tank)
        );
        assert!(
            outbound_wave_target_priority(EntityKind::Worker)
                > outbound_wave_target_priority(EntityKind::Panzerfaust)
        );
    }

    #[test]
    fn main_resource_depot_is_acquired_outside_nominal_standoff_radius() {
        let tile_size = 32;
        let tank = target_test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0);
        let resource_depot =
            target_test_entity(2, EntityKind::ResourceDepot, 25.0 * 32.0, 10.0 * 32.0);
        let observation = AiObservation {
            player_id: 1,
            tick: 0,
            map: AiMapSummary {
                width: 64,
                height: 64,
                tile_size,
            },
            economy: AiEconomy {
                steel: 0,
                oil: 0,
                supply_used: 1,
                supply_cap: 100,
            },
            own_start_tile: (10, 10),
            players: Vec::new(),
            owned: vec![tank],
            resources: Vec::new(),
            visible_allies: Vec::new(),
            visible_enemies: vec![resource_depot],
            ability_states: Vec::new(),
            smokes: Vec::new(),
            pending_builds: Vec::new(),
            upgrades: Vec::new(),
        };

        assert_eq!(
            visible_strategic_building_target_within_tiles(&observation, &[1], 13.5),
            None
        );
        assert_eq!(
            visible_strategic_building_target_within_tiles(&observation, &[1], 18.0),
            Some(2)
        );
    }

    #[test]
    fn stationary_target_requires_every_tank_to_be_in_range() {
        let mut first = target_test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0);
        first.owner = 1;
        let mut second = target_test_entity(3, EntityKind::Tank, 3.0 * 32.0, 10.0 * 32.0);
        second.owner = 1;
        let enemy = target_test_entity(100, EntityKind::Tank, 23.0 * 32.0, 10.0 * 32.0);
        let mut observation = regroup_test_observation(vec![first, second]);
        observation.visible_enemies.push(enemy);

        assert_eq!(
            shared_stationary_tank_target(&observation, &[1, 3], 13.5, None, None),
            None
        );
        assert_eq!(
            shared_stationary_tank_target(&observation, &[1], 13.5, None, None),
            Some(100)
        );
    }

    fn smoke_test_observation(focus: (u32, f32, f32), other: (u32, f32, f32)) -> AiObservation {
        let mut tank_one = target_test_entity(1, EntityKind::Tank, 10.0 * 32.0, 10.0 * 32.0);
        tank_one.owner = 1;
        tank_one.target_id = Some(focus.0);
        let mut tank_two = target_test_entity(3, EntityKind::Tank, 10.0 * 32.0, 11.0 * 32.0);
        tank_two.owner = 1;
        tank_two.target_id = Some(focus.0);
        let mut scout = target_test_entity(4, EntityKind::ScoutCar, 10.0 * 32.0, 10.0 * 32.0);
        scout.owner = 1;
        let mut engineering =
            target_test_entity(5, EntityKind::EngineeringComplex, 8.0 * 32.0, 8.0 * 32.0);
        engineering.owner = 1;
        let mut observation =
            regroup_test_observation(vec![tank_one, tank_two, scout, engineering]);
        observation.tick = 100;
        let mut focus_tank = target_test_entity(focus.0, EntityKind::Tank, focus.1, focus.2);
        focus_tank.hp = 220;
        let mut other_tank = target_test_entity(other.0, EntityKind::Tank, other.1, other.2);
        other_tank.hp = 292;
        observation.visible_enemies = vec![focus_tank, other_tank];
        observation.ability_states.push(AiAbilitySummary {
            entity_id: 4,
            kind: AbilityKind::Smoke,
            cooldown_left: 0,
            remaining_uses: Some(2),
            available_tick: Some(0),
            lockout_until_tick: None,
            charge_recharge_left: None,
        });
        observation
    }

    #[test]
    fn smoke_is_applied_to_healthy_rear_tank_and_focus_is_preserved() {
        let observation = smoke_test_observation(
            (100, 20.0 * 32.0, 10.0 * 32.0),
            (101, 22.0 * 32.0, 15.0 * 32.0),
        );
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment_focus_target = Some(100);
        memory.containment_focus_stable_since = Some(90);
        let mut focus = 100;

        let _ = maybe_issue_isolation_smoke(
            &mut actions,
            &observation,
            &[1, 3],
            4,
            &mut focus,
            &mut memory,
            true,
        );
        issue_hp_aware_tank_volley(
            &mut actions,
            &observation,
            &[1, 3],
            100,
            13.5,
            memory.containment_smoke_target,
        );
        let commands = actions.into_commands();

        assert_eq!(memory.containment_smoke_target, Some(101));
        assert!(matches!(
            commands.first(),
            Some(Command::UseAbility { units, ability: AbilityKind::Smoke, x: Some(_), y: Some(_), .. }) if units == &[4]
        ));
        assert!(commands.iter().any(|command| {
            matches!(command, Command::Attack { units, target: 100, .. } if units == &[1, 3])
        }));
        assert!(!commands
            .iter()
            .any(|command| { matches!(command, Command::Attack { target: 101, .. }) }));
    }

    #[test]
    fn rear_focus_switches_the_smoke_candidate_to_the_forward_tank() {
        let observation = smoke_test_observation(
            (101, 22.0 * 32.0, 15.0 * 32.0),
            (100, 20.0 * 32.0, 10.0 * 32.0),
        );
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment_focus_target = Some(101);
        memory.containment_focus_stable_since = Some(90);
        let mut focus = 101;

        let _ = maybe_issue_isolation_smoke(
            &mut actions,
            &observation,
            &[1, 3],
            4,
            &mut focus,
            &mut memory,
            true,
        );

        assert_eq!(memory.containment_smoke_target, Some(100));
    }

    #[test]
    fn stale_split_tank_orders_do_not_suppress_a_coordinated_smoke_volley() {
        let mut observation = smoke_test_observation(
            (100, 20.0 * 32.0, 10.0 * 32.0),
            (101, 22.0 * 32.0, 15.0 * 32.0),
        );
        observation
            .owned
            .iter_mut()
            .find(|unit| unit.id == 3)
            .unwrap()
            .target_id = Some(101);
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment_focus_target = Some(100);
        memory.containment_focus_stable_since = Some(80);
        let mut focus = 100;

        let _ = maybe_issue_isolation_smoke(
            &mut actions,
            &observation,
            &[1, 3],
            4,
            &mut focus,
            &mut memory,
            true,
        );

        assert_eq!(memory.containment_smoke_target, Some(101));
        assert!(matches!(
            actions.into_commands().first(),
            Some(Command::UseAbility {
                ability: AbilityKind::Smoke,
                ..
            })
        ));
    }

    #[test]
    fn local_defense_smoke_does_not_wait_for_frontal_focus_stability() {
        let observation = smoke_test_observation(
            (100, 20.0 * 32.0, 10.0 * 32.0),
            (101, 22.0 * 32.0, 15.0 * 32.0),
        );
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

        let directive = maybe_issue_local_defense_smoke(
            &mut actions,
            &observation,
            &[1, 3],
            &[1, 3, 4],
            &[100, 101],
            &mut memory,
        );

        assert_eq!(
            directive,
            Some(LocalDefenseSmokeDirective::Obscure {
                target: 101,
                scout: 4,
            })
        );
        assert!(matches!(
            actions.into_commands().first(),
            Some(Command::UseAbility {
                ability: AbilityKind::Smoke,
                ..
            })
        ));
    }

    #[test]
    fn large_local_tank_response_keeps_ordinary_defense_targeting() {
        let mut observation = smoke_test_observation(
            (100, 20.0 * 32.0, 10.0 * 32.0),
            (101, 22.0 * 32.0, 15.0 * 32.0),
        );
        let mut third_tank = target_test_entity(6, EntityKind::Tank, 11.0 * 32.0, 10.0 * 32.0);
        third_tank.owner = 1;
        observation.owned.push(third_tank);
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);

        let directive = maybe_issue_local_defense_smoke(
            &mut actions,
            &observation,
            &[1, 3, 6],
            &[1, 3, 4, 6],
            &[100, 101],
            &mut memory,
        );

        assert_eq!(directive, None);
        assert!(actions.into_commands().is_empty());
    }

    #[test]
    fn lone_local_tank_is_smoked_only_when_an_exposed_target_can_be_engaged() {
        let mut observation = smoke_test_observation(
            (100, 20.0 * 32.0, 10.0 * 32.0),
            (101, 50.0 * 32.0, 50.0 * 32.0),
        );
        observation.visible_enemies.push(target_test_entity(
            102,
            EntityKind::Panzerfaust,
            18.0 * 32.0,
            15.0 * 32.0,
        ));
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment_focus_target = Some(100);
        memory.containment_focus_stable_since = Some(80);
        let mut focus = 100;

        let _ = maybe_issue_isolation_smoke(
            &mut actions,
            &observation,
            &[1, 3],
            4,
            &mut focus,
            &mut memory,
            true,
        );

        assert_eq!(focus, 102);
        assert_eq!(memory.containment_smoke_target, Some(100));
        assert_eq!(memory.containment_smoke_focus_target, Some(102));
        assert!(matches!(
            actions.into_commands().first(),
            Some(Command::UseAbility {
                ability: AbilityKind::Smoke,
                ..
            })
        ));
    }

    #[test]
    fn lone_local_tank_is_suppressed_while_grouped_tanks_hold_fire() {
        let observation = smoke_test_observation(
            (100, 20.0 * 32.0, 10.0 * 32.0),
            (101, 50.0 * 32.0, 50.0 * 32.0),
        );
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment_focus_target = Some(100);
        memory.containment_focus_stable_since = Some(80);
        let mut focus = 100;

        let _ = maybe_issue_isolation_smoke(
            &mut actions,
            &observation,
            &[1, 3],
            4,
            &mut focus,
            &mut memory,
            true,
        );
        issue_hp_aware_tank_volley(
            &mut actions,
            &observation,
            &[1, 3],
            focus,
            13.5,
            memory.containment_smoke_target,
        );
        let commands = actions.into_commands();

        assert_eq!(memory.containment_smoke_target, Some(100));
        assert_eq!(memory.containment_smoke_focus_target, None);
        assert!(matches!(
            commands.first(),
            Some(Command::UseAbility {
                ability: AbilityKind::Smoke,
                ..
            })
        ));
        assert!(commands.iter().any(|command| {
            matches!(command, Command::HoldPosition { units, .. } if units == &[1, 3])
        }));
        assert!(!commands
            .iter()
            .any(|command| matches!(command, Command::Attack { target: 100, .. })));
    }

    #[test]
    fn distant_second_tank_requests_a_bounded_scout_launch_position() {
        let observation = smoke_test_observation(
            (100, 20.0 * 32.0, 10.0 * 32.0),
            (101, 25.0 * 32.0, 14.0 * 32.0),
        );
        assert!(!target_is_in_shared_tank_range(
            &observation,
            &[1, 3],
            101,
            15.5,
        ));
        let facts = AiFacts::from_observation(&observation);
        let mut actions = AiActionContext::new(&facts, SpendBudget::new(0, 0, 0, 100));
        let mut memory = AiDecisionMemory::for_profile(&JEFFS_AI);
        memory.containment_focus_target = Some(100);
        memory.containment_focus_stable_since = Some(90);
        let mut focus = 100;

        let launch = maybe_issue_isolation_smoke(
            &mut actions,
            &observation,
            &[1, 3],
            4,
            &mut focus,
            &mut memory,
            true,
        )
        .expect("bounded launch point");
        let tank_center = group_center(&observation, &[1, 3]).unwrap();
        let forward = normalized_direction(tank_center, (20.0 * 32.0, 10.0 * 32.0)).unwrap();
        let lateral_axis = (-forward.1, forward.0);
        let relative = (launch.0 - tank_center.0, launch.1 - tank_center.1);
        let forward_tiles =
            (relative.0 * forward.0 + relative.1 * forward.1) / observation.map.tile_size as f32;
        let lateral_tiles = (relative.0 * lateral_axis.0 + relative.1 * lateral_axis.1).abs()
            / observation.map.tile_size as f32;

        assert!(forward_tiles <= CONTAINMENT_SCOUT_SMOKE_FORWARD_LIMIT_TILES + 0.001);
        assert!(lateral_tiles <= CONTAINMENT_SCOUT_SMOKE_LATERAL_LIMIT_TILES + 0.001);
        assert!(actions.into_commands().is_empty());
    }

    #[test]
    fn rifle_sector_prioritizes_panzerfausts_and_never_targets_tanks() {
        let mut rifle = target_test_entity(1, EntityKind::Rifleman, 12.0 * 32.0, 10.0 * 32.0);
        rifle.owner = 1;
        let mut observation = regroup_test_observation(vec![rifle]);
        observation.visible_enemies = vec![
            target_test_entity(100, EntityKind::Tank, 14.0 * 32.0, 10.0 * 32.0),
            target_test_entity(101, EntityKind::Rifleman, 14.0 * 32.0, 10.5 * 32.0),
            target_test_entity(102, EntityKind::Panzerfaust, 14.5 * 32.0, 9.5 * 32.0),
        ];

        assert_eq!(
            rifle_sector_target(
                &observation,
                1,
                (12.0 * 32.0, 10.0 * 32.0),
                (10.0 * 32.0, 10.0 * 32.0),
                (30.0 * 32.0, 10.0 * 32.0),
            ),
            Some(102)
        );
    }

    #[test]
    fn endgame_search_visits_inner_and_outer_base_rings() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };
        let enemy_base = EnemyBaseFact {
            player_id: 2,
            start_tile: (50, 50),
            x: 50.5 * 32.0,
            y: 50.5 * 32.0,
        };
        let own_base = (9.5 * 32.0, 9.5 * 32.0);
        assert_eq!(
            endgame_search_point(own_base, enemy_base, map, 0),
            (1616.0, 1616.0)
        );
        assert_eq!(
            endgame_search_point(own_base, enemy_base, map, 1),
            (1872.0, 1616.0)
        );
        assert_eq!(
            endgame_search_point(own_base, enemy_base, map, 9),
            (2128.0, 1616.0)
        );
        assert_eq!(
            endgame_search_point(own_base, enemy_base, map, ENDGAME_SEARCH_OFFSETS.len()),
            endgame_search_point(own_base, enemy_base, map, 0)
        );
    }

    #[test]
    fn endgame_search_ring_rotates_with_the_players() {
        let map = AiMapSummary {
            width: 100,
            height: 100,
            tile_size: 32,
        };
        let world_size = map.width as f32 * map.tile_size as f32;
        let own_base = (9.5 * 32.0, 9.5 * 32.0);
        let enemy_base = EnemyBaseFact {
            player_id: 2,
            start_tile: (80, 80),
            x: 80.5 * 32.0,
            y: 80.5 * 32.0,
        };
        let rotated_enemy_base = EnemyBaseFact {
            player_id: 2,
            start_tile: (19, 19),
            x: world_size - enemy_base.x,
            y: world_size - enemy_base.y,
        };
        for waypoint in 0..ENDGAME_SEARCH_OFFSETS.len() {
            let original = endgame_search_point(own_base, enemy_base, map, waypoint);
            let rotated = endgame_search_point(
                (world_size - own_base.0, world_size - own_base.1),
                rotated_enemy_base,
                map,
                waypoint,
            );
            assert_eq!(rotated, (world_size - original.0, world_size - original.1));
        }
    }
