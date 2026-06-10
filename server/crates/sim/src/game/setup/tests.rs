use super::*;

fn owned_kind_count(game: &Game, owner: u32, kind: EntityKind) -> usize {
    game.entities
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind)
        .count()
}

#[test]
fn debug_starting_loadout_applies_to_humans_only() {
    let players = [
        PlayerInit {
            id: 1,
            name: "Human".to_string(),
            color: "#cc1111".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "AI".to_string(),
            color: "#1133bb".to_string(),
            is_ai: true,
        },
    ];
    let game = Game::new_with_debug_starting_loadout_and_random_ai_profiles(
        &players,
        config::QUICKSTART_STEEL,
        config::QUICKSTART_OIL,
        1,
    );

    assert_eq!(owned_kind_count(&game, 1, EntityKind::Depot), 15);
    assert_eq!(owned_kind_count(&game, 1, EntityKind::Steelworks), 1);
    assert_eq!(owned_kind_count(&game, 1, EntityKind::TrainingCentre), 1);
    assert_eq!(owned_kind_count(&game, 1, EntityKind::Barracks), 2);
    assert_eq!(owned_kind_count(&game, 1, EntityKind::Factory), 2);
    for kind in [
        EntityKind::Worker,
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::MortarTeam,
        EntityKind::AtTeam,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        assert_eq!(owned_kind_count(&game, 1, kind), 5, "{kind}");
    }

    assert_eq!(owned_kind_count(&game, 2, EntityKind::Depot), 0);
    assert_eq!(owned_kind_count(&game, 2, EntityKind::Barracks), 0);
    assert_eq!(
        owned_kind_count(&game, 2, EntityKind::Worker),
        config::STARTING_WORKERS as usize
    );
}

#[test]
fn debug_starting_loadout_adds_inert_enemy_mortar_corner_without_profile() {
    let players = [PlayerInit {
        id: 1,
        name: "Human".to_string(),
        color: "#cc1111".to_string(),
        is_ai: false,
    }];
    let game = Game::new_with_debug_starting_loadout_and_random_ai_profiles(
        &players,
        config::QUICKSTART_STEEL,
        config::QUICKSTART_OIL,
        1,
    );
    game.assert_invariants();

    let battery_player = game
        .players
        .iter()
        .find(|p| p.id == DEBUG_INERT_ENEMY_ID)
        .expect("debug mortar corner should be represented as an AI player");
    assert!(battery_player.is_ai);

    let human_start = game.players.iter().find(|p| p.id == 1).unwrap().start_tile;
    assert_eq!(
        battery_player.start_tile,
        debug_clockwise_adjacent_corner_tile(&game.map, human_start)
    );

    let map_center = game.map.world_size_px() * 0.5;
    let clump_center = game
        .map
        .tile_center(battery_player.start_tile.0, battery_player.start_tile.1);
    let center_facing = (map_center - clump_center.1).atan2(map_center - clump_center.0);
    let ts = config::TILE_SIZE as f32;
    let mortars: Vec<_> = game
        .entities
        .iter()
        .filter(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::MortarTeam)
        .collect();
    assert_eq!(mortars.len(), DEBUG_INERT_MORTAR_COUNT);
    for mortar in mortars {
        let facing_to_center = (map_center - mortar.pos_y).atan2(map_center - mortar.pos_x);
        assert_eq!(mortar.weapon_setup(), WeaponSetup::Deployed);
        assert!(
            (mortar.weapon_facing().unwrap_or(f32::NAN) - facing_to_center).abs() <= 0.001,
            "mortar weapon should point toward map center"
        );
        assert!(
            (mortar.facing() - facing_to_center).abs() <= 0.001,
            "mortar should face map center"
        );
    }

    let mut mortar_offsets: Vec<_> = game
        .entities
        .iter()
        .filter(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::MortarTeam)
        .map(|e| {
            (
                ((e.pos_x - clump_center.0) / ts).round() as i32,
                ((e.pos_y - clump_center.1) / ts).round() as i32,
            )
        })
        .collect();
    mortar_offsets.sort_unstable();
    assert_eq!(mortar_offsets, [(-2, 0), (0, -2), (0, 2), (2, -2), (2, 0)]);

    let scout_car = game
        .entities
        .iter()
        .find(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::ScoutCar)
        .expect("debug mortar clump should have a scout car inside it");
    assert!((scout_car.pos_x - clump_center.0).abs() <= 0.001);
    assert!((scout_car.pos_y - clump_center.1).abs() <= 0.001);
    assert!((scout_car.facing() - center_facing).abs() <= 0.001);

    let mut depot_offsets: Vec<_> = game
        .entities
        .iter()
        .filter(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::Depot)
        .map(|e| {
            (
                ((e.pos_x - clump_center.0) / ts).round() as i32,
                ((e.pos_y - clump_center.1) / ts).round() as i32,
            )
        })
        .collect();
    depot_offsets.sort_unstable();
    assert_eq!(depot_offsets, [(-5, 0), (0, -5), (0, 5), (5, 0)]);
}
