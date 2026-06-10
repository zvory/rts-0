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
fn debug_starting_loadout_adds_inert_enemy_at_gun_battery_without_profile() {
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
        .expect("debug battery should be represented as an AI player");
    assert!(battery_player.is_ai);

    let human_start = game.players.iter().find(|p| p.id == 1).unwrap().start_tile;
    let max_tile = game.map.size - 1;
    assert_eq!(
        battery_player.start_tile,
        (max_tile - human_start.0, max_tile - human_start.1)
    );

    let map_center = game.map.world_size_px() * 0.5;
    let battery_center = game
        .map
        .tile_center(battery_player.start_tile.0, battery_player.start_tile.1);
    let center_facing = (map_center - battery_center.1).atan2(map_center - battery_center.0);
    let forward = (center_facing.cos(), center_facing.sin());
    let side = (-center_facing.sin(), center_facing.cos());
    let ts = config::TILE_SIZE as f32;
    let guns: Vec<_> = game
        .entities
        .iter()
        .filter(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::AtTeam)
        .collect();
    assert_eq!(guns.len(), DEBUG_INERT_AT_GUN_COUNT);
    for gun in guns {
        let facing_to_center = (map_center - gun.pos_y).atan2(map_center - gun.pos_x);
        assert_eq!(gun.weapon_setup(), WeaponSetup::Deployed);
        assert!(
            (gun.emplacement_facing().unwrap_or(f32::NAN) - facing_to_center).abs() <= 0.001,
            "gun emplacement should point toward map center"
        );
        assert!(
            (gun.weapon_facing().unwrap_or(f32::NAN) - facing_to_center).abs() <= 0.001,
            "gun barrel should point toward map center"
        );
        assert!(
            (gun.facing() - facing_to_center).abs() <= 0.001,
            "gun should face map center"
        );
    }

    let mut rifle_offsets: Vec<_> = game
        .entities
        .iter()
        .filter(|e| e.owner == DEBUG_INERT_ENEMY_ID && e.kind == EntityKind::Rifleman)
        .map(|e| {
            let dx = e.pos_x - battery_center.0;
            let dy = e.pos_y - battery_center.1;
            let front_tiles = (dx * forward.0 + dy * forward.1) / ts;
            let side_tiles = (dx * side.0 + dy * side.1) / ts;
            (front_tiles, side_tiles, e.facing())
        })
        .collect();
    assert_eq!(rifle_offsets.len(), DEBUG_INERT_RIFLEMAN_SCREEN_COUNT);
    rifle_offsets.sort_by(|a, b| a.1.total_cmp(&b.1));

    let center_index = (DEBUG_INERT_RIFLEMAN_SCREEN_COUNT.saturating_sub(1)) as f32 * 0.5;
    for (i, (front_tiles, side_tiles, facing)) in rifle_offsets.into_iter().enumerate() {
        let expected_side = (i as f32 - center_index) * DEBUG_INERT_RIFLEMAN_SCREEN_SPACING_TILES;
        assert!(
            (front_tiles - DEBUG_INERT_RIFLEMAN_SCREEN_FRONT_TILES).abs() <= 0.001,
            "rifleman screen should stand five tiles in front of the AT guns"
        );
        assert!(
            (side_tiles - expected_side).abs() <= 0.001,
            "rifleman screen should form an evenly spaced line"
        );
        assert!(
            (facing - center_facing).abs() <= 0.001,
            "rifleman screen should face map center"
        );
    }
}
