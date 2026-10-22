use super::*;
use rand::{rngs::SmallRng, SeedableRng};
fn open_map(size: u32) -> Map {
    Map {
        width: size,
        height: size,
        terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4), (size - 5, size - 5)],
        ..Default::default()
    }
}
fn visible_fog(map: &Map, entities: &EntityStore) -> Fog {
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2], entities, map);
    fog
}

#[test]
fn cone_geometry_has_expected_single_target_and_dense_coverage() {
    let map = open_map(32);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 300.0)
        .unwrap();
    let center = entities
        .spawn_unit(2, EntityKind::Rifleman, 314.0, 300.0)
        .unwrap();
    let left = entities
        .spawn_unit(2, EntityKind::Rifleman, 313.0, 282.0)
        .unwrap();
    let right = entities
        .spawn_unit(2, EntityKind::Rifleman, 313.0, 318.0)
        .unwrap();
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 3)]);
    let mut rng = SmallRng::seed_from_u64(721);
    let (mut single, mut dense) = (0, 0);
    for _ in 0..30000 {
        for ray in 0..rules::MG_BURST_BULLETS {
            let angle: f32 = if ray == 0 {
                0.0
            } else {
                rng.gen_range(-rules::MG_HALF_SPREAD_RAD..=rules::MG_HALF_SPREAD_RAD)
            };
            let end = (100.0 + angle.cos() * 214.0, 300.0 + angle.sin() * 214.0);
            let single_hit = first_hit(
                &map,
                &entities,
                &teams,
                &[center],
                attacker,
                1,
                center,
                (100.0, 300.0),
                end,
            )
            .is_some();
            if ray == 0 {
                assert!(single_hit, "clear aimed ray must hit the selected target");
            }
            single += usize::from(single_hit);
            dense += usize::from(
                first_hit(
                    &map,
                    &entities,
                    &teams,
                    &[center, left, right],
                    attacker,
                    1,
                    center,
                    (100.0, 300.0),
                    end,
                )
                .is_some(),
            );
        }
    }
    let single_dps = single as f32 / 30000.0 * 5.0;
    let dense_dps = dense as f32 / 30000.0 * 5.0;
    assert!((7.0..8.0).contains(&single_dps), "single DPS {single_dps}");
    assert!(dense_dps > 12.0, "dense DPS {dense_dps}");
}

#[test]
fn bullets_stop_at_first_enemy_and_pass_friendly_infantry() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .unwrap();
    let friend = entities
        .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
        .unwrap();
    let front = entities
        .spawn_unit(2, EntityKind::Rifleman, 145.0, 100.0)
        .unwrap();
    let rear = entities
        .spawn_unit(2, EntityKind::Rifleman, 175.0, 100.0)
        .unwrap();
    let hit = first_hit(
        &map,
        &entities,
        &TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 3)]),
        &[rear, friend, front],
        attacker,
        1,
        rear,
        (100.0, 100.0),
        (300.0, 100.0),
    )
    .unwrap();
    assert_eq!(hit.0, front);
    assert!((hit.1 - 0.18).abs() < 0.001);
}

#[test]
fn burst_damages_front_only_and_emits_fixed_rays_without_hidden_viewer_data() {
    let map = open_map(32);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .unwrap();
    let front = entities
        .spawn_unit(2, EntityKind::Rifleman, 145.0, 100.0)
        .unwrap();
    let rear = entities
        .spawn_unit(2, EntityKind::Rifleman, 175.0, 100.0)
        .unwrap();
    let fog = visible_fog(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    let mut events = HashMap::from([(1, vec![]), (2, vec![]), (3, vec![])]);
    let mut reveals = vec![];
    let profile = rules::weapon_profile(rules::WeaponKind::MachineGunnerMg).unwrap();
    fire(
        &map,
        &mut entities,
        &TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 3)]),
        &spatial,
        &LineOfSight::new(&map),
        &fog,
        &SmokeCloudStore::new(),
        &mut SmallRng::seed_from_u64(1),
        &mut events,
        &mut reveals,
        attacker,
        rear,
        profile,
        209.2,
        1,
    );
    assert_eq!(entities.get(front).unwrap().hp, 39);
    assert_eq!(entities.get(rear).unwrap().hp, 45);
    assert_eq!(
        events[&1]
            .iter()
            .filter(|ev| matches!(
                ev,
                Event::Attack {
                    shot_origin: Some(_),
                    ..
                }
            ))
            .count(),
        rules::MG_BURST_BULLETS
    );
    assert!(events[&3].is_empty());
}

#[test]
fn scout_car_burst_cannot_one_shot_completed_resource_buildings() {
    for (kind, max_hp) in [(EntityKind::SteelMine, 37), (EntityKind::PumpJack, 56)] {
        let map = open_map(16);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::ScoutCar, 100.0, 100.0)
            .unwrap();
        let target = entities
            .spawn_building(2, kind, 180.0, 100.0, true)
            .unwrap();
        let fog = visible_fog(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let mut events = HashMap::from([(1, vec![]), (2, vec![])]);
        fire(
            &map,
            &mut entities,
            &TeamRelations::from_player_teams([(1, 1), (2, 2)]),
            &spatial,
            &LineOfSight::new(&map),
            &fog,
            &SmokeCloudStore::new(),
            &mut SmallRng::seed_from_u64(1),
            &mut events,
            &mut vec![],
            attacker,
            target,
            rules::weapon_profile(rules::WeaponKind::ScoutCarMg).unwrap(),
            250.0,
            1,
        );
        let remaining_hp = entities.get(target).unwrap().hp;
        assert!(
            (max_hp - 9..=max_hp - 3).contains(&remaining_hp),
            "{kind:?} takes one to three Scout Car rays, not a lethal burst"
        );
        assert!(matches!(events[&1].first(), Some(Event::Attack { to, .. }) if *to == target));
        let target_rays = events[&1]
            .iter()
            .filter(|event| matches!(event, Event::Attack { to, .. } if *to == target))
            .count();
        assert_eq!(max_hp - remaining_hp, target_rays as u32 * 3, "{kind:?}");
    }
}

#[test]
fn terrain_clips_missed_bullets_before_wall() {
    let mut map = open_map(16);
    map.terrain[3 * 16 + 5] = crate::protocol::terrain::ROCK;
    let end = clear_endpoint(&LineOfSight::new(&map), (100.0, 100.0), (300.0, 100.0));
    assert!((159.0..160.0).contains(&end.0));
}

#[test]
fn hidden_incidental_victims_do_not_disclose_impact_positions() {
    for hidden_by in ["fog", "concealment", "smoke"] {
        let mut map = open_map(16);
        if hidden_by == "concealment" {
            map.concealment_tiles = vec![(4, 3)];
        }
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
            .unwrap();
        let hidden = entities
            .spawn_unit(2, EntityKind::Rifleman, 130.0, 100.0)
            .unwrap();
        let intended = entities
            .spawn_unit(2, EntityKind::Rifleman, 190.0, 100.0)
            .unwrap();
        let mut grid = vec![true; 16 * 16];
        if hidden_by == "fog" {
            grid[3 * 16 + 4] = false;
        }
        let fog = Fog::from_checkpoint_grids(
            16,
            16,
            BTreeMap::from([(1, grid)]),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let mut smokes = crate::game::smoke::SmokeCloudStore::new();
        if hidden_by == "smoke" {
            smokes.spawn(138.0, 100.0, 10.0 / 32.0, 100, 0).unwrap();
        }
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let mut events = HashMap::from([(1, vec![])]);
        fire(
            &map,
            &mut entities,
            &TeamRelations::from_player_teams([(1, 1), (2, 2)]),
            &spatial,
            &LineOfSight::with_smoke(&map, &smokes),
            &fog,
            &smokes,
            &mut SmallRng::seed_from_u64(1),
            &mut events,
            &mut vec![],
            attacker,
            intended,
            rules::weapon_profile(rules::WeaponKind::MachineGunnerMg).unwrap(),
            209.2,
            1,
        );
        assert_eq!(
            entities.get(hidden).unwrap().hp,
            39,
            "{hidden_by}: authoritative hits still resolve"
        );
        assert!(
            events[&1].is_empty(),
            "{hidden_by}: hidden body must not disclose its impact: {:?}",
            events[&1]
        );
    }
}

#[test]
fn ray_visibility_checks_hidden_corner_slivers_and_team_vision() {
    let map = open_map(16);
    let mut grid = vec![true; 16 * 16];
    grid[3 * 16 + 4] = false;
    let fog = Fog::from_checkpoint_grids(
        16,
        16,
        BTreeMap::from([(1, grid), (2, vec![true; 16 * 16])]),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let enemies = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let allies = TeamRelations::from_player_teams([(1, 1), (2, 1)]);
    let start = (100.0, 100.0);
    let end = (200.0, 199.0);
    // This ray spends less than half a pixel in hidden tile (4, 3). Four-pixel samples
    // miss it, although the transmitted trajectory would disclose the hidden segment.
    assert!(!ray_visible(&map, &fog, &enemies, 1, start, end));
    assert!(!ray_visible(&map, &fog, &enemies, 1, end, start));
    assert!(ray_visible(&map, &fog, &allies, 1, start, end));
    assert!(ray_visible(&map, &fog, &enemies, 1, start, (120.0, 120.0)));
    assert!(ray_visible(&map, &fog, &enemies, 1, start, start));
    assert!(!ray_visible(&map, &fog, &enemies, 1, start, (-1.0, 100.0)));
}

#[test]
fn shooting_traps_does_not_disclose_a_concealed_shooter() {
    let mut map = open_map(16);
    map.concealment_tiles = vec![(3, 3)];
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .unwrap();
    let trap = entities
        .spawn_building(1, EntityKind::TankTrap, 180.0, 100.0, true)
        .unwrap();
    let hp = entities.get(trap).unwrap().hp;
    let fog = Fog::from_checkpoint_grids(
        16,
        16,
        BTreeMap::from([(1, vec![true; 256]), (2, vec![true; 256])]),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    let mut events = HashMap::from([(1, vec![]), (2, vec![])]);
    let mut reveals = vec![];
    fire(
        &map,
        &mut entities,
        &TeamRelations::from_player_teams([(1, 1), (2, 2)]),
        &spatial,
        &LineOfSight::new(&map),
        &fog,
        &SmokeCloudStore::new(),
        &mut SmallRng::seed_from_u64(1),
        &mut events,
        &mut reveals,
        attacker,
        trap,
        rules::weapon_profile(rules::WeaponKind::MachineGunnerMg).unwrap(),
        209.2,
        1,
    );
    assert!(entities.get(trap).unwrap().hp < hp);
    assert!(reveals.is_empty());
    assert_eq!(events[&1].len(), 3, "owner still sees all three rays");
    assert!(events[&1]
        .iter()
        .all(|event| matches!(event, Event::Attack { reveal: None, .. })));
    assert!(
        events[&2].is_empty(),
        "visible ground must not expose a concealed shooter"
    );
}

mod building_range;
