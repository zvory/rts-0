use super::*;
use crate::game::command::SimCommand;
use crate::game::entity::{EntityKind, Order};
use crate::game::lab::{LabMoveEntity, LabOp};
use crate::game::upgrade::UpgradeKind;
use crate::game::{services, systems, SmokeCloudStore};
use crate::protocol::terrain;

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ]
}

fn three_players() -> [PlayerInit; 3] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ]
}

fn empty_flat_game(players: &[PlayerInit]) -> Game {
    let mut game = Game::new_for_replay(players, 0xA117_4E11);
    for tile in &mut game.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    game.smokes = SmokeCloudStore::new();
    game.mortar_shells = MortarShellStore::default();
    game.artillery_shells = artillery::ArtilleryShellStore::default();
    repair_world(&mut game);
    game
}

fn repair_world(game: &mut Game) {
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.recompute_live_fog(&ids);
    game.refresh_building_memory(&ids);
    game.refresh_trench_memory(&ids);
}

fn grant_entrenchment(game: &mut Game, player: u32) {
    game.players
        .iter_mut()
        .find(|p| p.id == player)
        .expect("player should exist")
        .upgrades
        .insert(UpgradeKind::Entrenchment);
}

fn tick_n(game: &mut Game, ticks: u32) {
    for _ in 0..ticks {
        game.tick();
    }
}

fn trench_contains_point(trench: crate::game::trench::Trench, x: f32, y: f32) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    let dx = x - trench.x;
    let dy = y - trench.y;
    let radius = trench.radius_tiles * config::TILE_SIZE as f32;
    dx * dx + dy * dy <= radius * radius
}

fn entrenchment_dig_ticks(entity: &crate::game::entity::Entity) -> u32 {
    entity
        .movement
        .as_ref()
        .expect("entity should have movement")
        .entrenchment_dig_ticks
}

fn active_trench_occupation(entity: &crate::game::entity::Entity) -> Option<u32> {
    crate::game::entity::active_trench_occupation(entity)
}

#[test]
fn seeded_trenches_persist_and_project_to_full_world_snapshots() {
    let mut game = empty_flat_game(&players());
    let trench_pos = game.map.tile_center(24, 24);
    let trench = game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");

    assert!(
        game.snapshot_for(1).trenches.is_empty(),
        "player with no current or remembered vision should not receive hidden trench terrain"
    );
    assert!(game
        .snapshot_full_for(1)
        .trenches
        .iter()
        .any(|view| view.id == trench && (view.x, view.y) == trench_pos));

    game.tick();

    assert!(game
        .snapshot_full_for(1)
        .trenches
        .iter()
        .any(|view| view.id == trench && (view.x, view.y) == trench_pos));
}

#[test]
fn trench_projection_uses_visibility_then_remembered_terrain() {
    let mut game = empty_flat_game(&players());
    let scout_pos = game.map.tile_center(20, 20);
    let far_pos = game.map.tile_center(4, 50);
    let hidden_pos = game.map.tile_center(50, 50);
    let scout = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    repair_world(&mut game);

    let visible_trench = game
        .spawn_trench_for_test(scout_pos.0, scout_pos.1)
        .expect("visible trench should seed");
    let hidden_trench = game
        .spawn_trench_for_test(hidden_pos.0, hidden_pos.1)
        .expect("hidden trench should seed");

    let visible = game.snapshot_for(1);
    assert!(visible
        .trenches
        .iter()
        .any(|view| view.id == visible_trench));
    assert!(!visible.trenches.iter().any(|view| view.id == hidden_trench));

    game.entities.remove(scout);
    game.entities
        .spawn_unit(1, EntityKind::Rifleman, far_pos.0, far_pos.1)
        .expect("far scout should spawn");
    game.tick();

    let remembered = game.snapshot_for(1);
    assert!(
        remembered
            .trenches
            .iter()
            .any(|view| view.id == visible_trench),
        "discovered trench terrain should remain visible after it falls back into fog"
    );
    assert!(!remembered
        .trenches
        .iter()
        .any(|view| view.id == hidden_trench));
}

#[test]
fn spectator_projection_uses_selected_player_trench_vision() {
    let mut game = empty_flat_game(&three_players());
    let p1_base = game.map.tile_center(3, 3);
    let p2_base = game.map.tile_center(55, 3);
    let p3_base = game.map.tile_center(3, 55);
    game.entities
        .spawn_building(1, EntityKind::CityCentre, p1_base.0, p1_base.1, true)
        .expect("p1 base should spawn");
    game.entities
        .spawn_building(2, EntityKind::CityCentre, p2_base.0, p2_base.1, true)
        .expect("p2 base should spawn");
    game.entities
        .spawn_building(3, EntityKind::CityCentre, p3_base.0, p3_base.1, true)
        .expect("p3 base should spawn");
    let scout_pos = game.map.tile_center(32, 32);
    game.entities
        .spawn_unit(2, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("p2 scout should spawn");
    repair_world(&mut game);
    let trench = game
        .spawn_trench_for_test(scout_pos.0, scout_pos.1)
        .expect("trench should seed");

    assert!(game
        .snapshot_for_spectator(&[2])
        .trenches
        .iter()
        .any(|view| view.id == trench));
    assert!(!game
        .snapshot_for_spectator(&[1])
        .trenches
        .iter()
        .any(|view| view.id == trench));
    assert!(game
        .snapshot_for_spectator(&[1, 2])
        .trenches
        .iter()
        .any(|view| view.id == trench));
}

#[test]
fn researched_eligible_infantry_creates_trench_after_90_stationary_ticks() {
    let mut game = empty_flat_game(&players());
    grant_entrenchment(&mut game, 1);
    let pos = game.map.tile_center(24, 24);
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, pos.0, pos.1)
        .expect("rifleman should spawn");
    repair_world(&mut game);

    tick_n(&mut game, config::ENTRENCHMENT_DIG_IN_TICKS - 1);
    assert_eq!(game.trenches.all().len(), 0);
    assert_eq!(
        entrenchment_dig_ticks(game.entities.get(rifleman).expect("rifleman should exist")),
        config::ENTRENCHMENT_DIG_IN_TICKS - 1
    );

    game.tick();
    assert_eq!(game.trenches.all().len(), 1);
    let occupied =
        active_trench_occupation(game.entities.get(rifleman).expect("rifleman should exist"))
            .expect("completed dig-in should occupy the created trench");
    assert!(game
        .trenches
        .all()
        .iter()
        .any(|trench| trench.id == occupied));
    assert!(game
        .snapshot_full_for(1)
        .entities
        .iter()
        .any(|view| view.id == rifleman && view.occupied_trench_id == Some(occupied)));
}

#[test]
fn commanded_movement_cancels_dig_in_progress() {
    let mut game = empty_flat_game(&players());
    grant_entrenchment(&mut game, 1);
    let pos = game.map.tile_center(24, 24);
    let goal = game.map.tile_center(30, 24);
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, pos.0, pos.1)
        .expect("rifleman should spawn");
    repair_world(&mut game);

    tick_n(&mut game, config::ENTRENCHMENT_DIG_IN_TICKS / 2);
    assert!(
        entrenchment_dig_ticks(game.entities.get(rifleman).expect("rifleman should exist")) > 0
    );

    game.enqueue(
        1,
        SimCommand::Move {
            units: vec![rifleman],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    game.tick();

    let unit = game.entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(entrenchment_dig_ticks(unit), 0);
    assert_eq!(active_trench_occupation(unit), None);
    assert_eq!(game.trenches.all().len(), 0);
}

#[test]
fn collision_shove_cancels_dig_in_progress() {
    let mut game = empty_flat_game(&players());
    grant_entrenchment(&mut game, 1);
    let pos = game.map.tile_center(24, 24);
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, pos.0, pos.1)
        .expect("rifleman should spawn");
    repair_world(&mut game);

    tick_n(&mut game, config::ENTRENCHMENT_DIG_IN_TICKS / 2);
    assert!(
        entrenchment_dig_ticks(game.entities.get(rifleman).expect("rifleman should exist")) > 0
    );

    game.entities
        .spawn_unit(2, EntityKind::Rifleman, pos.0 + 1.0, pos.1)
        .expect("blocking rifleman should spawn");
    repair_world(&mut game);

    game.tick();

    let unit = game.entities.get(rifleman).expect("rifleman should exist");
    assert!(
        (unit.pos_x - pos.0).abs() > 0.1 || (unit.pos_y - pos.1).abs() > 0.1,
        "collision fixture should shove the digging unit"
    );
    assert_eq!(entrenchment_dig_ticks(unit), 0);
    assert_eq!(active_trench_occupation(unit), None);
    assert_eq!(game.trenches.all().len(), 0);
}

#[test]
fn firing_facing_and_target_changes_do_not_cancel_stationary_dig_in() {
    let mut game = empty_flat_game(&players());
    grant_entrenchment(&mut game, 1);
    let pos = game.map.tile_center(24, 24);
    let target_pos = game.map.tile_center(27, 24);
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, pos.0, pos.1)
        .expect("rifleman should spawn");
    let target = game
        .entities
        .spawn_building(2, EntityKind::Depot, target_pos.0, target_pos.1, true)
        .expect("target depot should spawn");
    if let Some(unit) = game.entities.get_mut(rifleman) {
        unit.set_order(Order::attack(target));
        unit.set_target_id(Some(target));
    }
    repair_world(&mut game);

    tick_n(&mut game, config::ENTRENCHMENT_DIG_IN_TICKS);

    let unit = game.entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(game.trenches.all().len(), 1);
    assert!(
        active_trench_occupation(unit).is_some(),
        "stationary firing should still complete dig-in"
    );
}

#[test]
fn pre_research_infantry_can_occupy_existing_trench_but_cannot_create_new_one() {
    let mut game = empty_flat_game(&players());
    let trench_pos = game.map.tile_center(24, 24);
    let trench = game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    let rifleman = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, trench_pos.0, trench_pos.1)
        .expect("rifleman should spawn");
    repair_world(&mut game);

    game.tick();
    assert_eq!(
        active_trench_occupation(game.entities.get(rifleman).expect("rifleman should exist")),
        Some(trench)
    );

    let open_pos = game.map.tile_center(40, 40);
    if let Some(unit) = game.entities.get_mut(rifleman) {
        unit.set_position(open_pos.0, open_pos.1);
    }
    repair_world(&mut game);
    tick_n(&mut game, config::ENTRENCHMENT_DIG_IN_TICKS + 5);

    assert_eq!(game.trenches.all().len(), 1);
    assert_eq!(
        active_trench_occupation(game.entities.get(rifleman).expect("rifleman should exist")),
        None
    );
}

#[test]
fn lab_move_clears_trench_occupation_without_waiting_for_tick() {
    let mut game = empty_flat_game(&players());
    let trench_pos = game.map.tile_center(24, 24);
    let trench = game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, trench_pos.0, trench_pos.1)
        .expect("rifleman should spawn");
    repair_world(&mut game);

    game.tick();
    assert_eq!(
        active_trench_occupation(game.entities.get(rifleman).expect("rifleman should exist")),
        Some(trench)
    );

    let open_pos = game.map.tile_center(40, 40);
    game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
        entity_id: rifleman,
        x: open_pos.0,
        y: open_pos.1,
    }))
    .expect("lab move should succeed");

    assert_eq!(
        active_trench_occupation(game.entities.get(rifleman).expect("rifleman should exist")),
        None
    );
    assert!(game.snapshot_full_for(1).entities.iter().any(|view| {
        view.id == rifleman && view.occupied_trench_id.is_none()
    }));
}

#[test]
fn excluded_units_and_buildings_do_not_create_or_occupy_trenches() {
    for kind in [
        EntityKind::Worker,
        EntityKind::MortarTeam,
        EntityKind::AntiTankGun,
        EntityKind::Artillery,
        EntityKind::Ekat,
        EntityKind::Golem,
        EntityKind::Tank,
        EntityKind::ScoutCar,
        EntityKind::CityCentre,
    ] {
        let mut game = empty_flat_game(&players());
        grant_entrenchment(&mut game, 1);
        let pos = game.map.tile_center(24, 24);
        game.spawn_trench_for_test(pos.0, pos.1)
            .expect("trench should seed");
        let entity = if kind.is_building() {
            game.entities
                .spawn_building(1, kind, pos.0, pos.1, true)
                .expect("building should spawn")
        } else {
            game.entities
                .spawn_unit(1, kind, pos.0, pos.1)
                .expect("unit should spawn")
        };
        repair_world(&mut game);

        tick_n(&mut game, config::ENTRENCHMENT_DIG_IN_TICKS + 1);

        assert_eq!(
            active_trench_occupation(game.entities.get(entity).expect("entity should exist")),
            None,
            "{kind:?} should not occupy trenches"
        );
        assert_eq!(
            game.trenches.all().len(),
            1,
            "{kind:?} should not create a trench"
        );
    }
}

#[test]
fn occupied_trenches_reject_second_eligible_occupant() {
    let mut game = empty_flat_game(&players());
    let trench_pos = game.map.tile_center(24, 24);
    let trench = game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    let radius = config::ENTRENCHMENT_TRENCH_RADIUS_TILES * config::TILE_SIZE as f32;
    let first = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, trench_pos.0, trench_pos.1)
        .expect("first rifleman should spawn");
    repair_world(&mut game);

    game.tick();
    assert_eq!(
        active_trench_occupation(game.entities.get(first).expect("first should exist")),
        Some(trench)
    );

    let second_start = (trench_pos.0 + radius + 10.0, trench_pos.1);
    let second = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, second_start.0, second_start.1)
        .expect("second rifleman should spawn");
    repair_world(&mut game);

    game.tick();

    let first_entity = game.entities.get(first).expect("first should exist");
    let second_entity = game.entities.get(second).expect("second should exist");
    assert_eq!(active_trench_occupation(first_entity), Some(trench));
    assert_eq!(active_trench_occupation(second_entity), None);
    assert!(
        !trench_contains_point(
            *game
                .trenches
                .all()
                .iter()
                .find(|view| view.id == trench)
                .expect("trench should still exist"),
            second_entity.pos_x,
            second_entity.pos_y
        ),
        "second occupant should remain outside the already occupied trench footprint"
    );
}

#[test]
fn adjacent_researched_infantry_dig_separate_trenches() {
    let mut game = empty_flat_game(&players());
    grant_entrenchment(&mut game, 1);
    let pos = game.map.tile_center(24, 24);
    let separation = 22.0;
    let first = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, pos.0, pos.1)
        .expect("first rifleman should spawn");
    let second = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, pos.0 + separation, pos.1)
        .expect("second rifleman should spawn");
    repair_world(&mut game);

    tick_n(&mut game, config::ENTRENCHMENT_DIG_IN_TICKS);

    let first_trench =
        active_trench_occupation(game.entities.get(first).expect("first should exist"))
            .expect("first should occupy its own trench");
    let second_trench =
        active_trench_occupation(game.entities.get(second).expect("second should exist"))
            .expect("second should occupy its own trench");
    assert_ne!(first_trench, second_trench);
    assert_eq!(game.trenches.all().len(), 2);
}

#[test]
fn slotting_rejects_positions_blocked_by_tank_traps() {
    let mut game = empty_flat_game(&players());
    let trench_pos = game.map.tile_center(24, 24);
    game.spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    game.entities
        .spawn_building(2, EntityKind::TankTrap, trench_pos.0, trench_pos.1, true)
        .expect("tank trap should spawn");
    let radius = config::ENTRENCHMENT_TRENCH_RADIUS_TILES * config::TILE_SIZE as f32;
    let start = (trench_pos.0 + radius + 14.0, trench_pos.1);
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman should spawn");
    repair_world(&mut game);

    game.tick();

    let unit = game.entities.get(rifleman).expect("rifleman should exist");
    assert_eq!(active_trench_occupation(unit), None);
    assert!(
        (unit.pos_x - start.0).abs() < 0.1 && (unit.pos_y - start.1).abs() < 0.1,
        "blocked slotting should not move the unit through a Tank Trap"
    );
}

#[test]
fn occupied_visible_units_project_without_revealing_hidden_occupants() {
    let mut game = empty_flat_game(&players());
    let trench_pos = game.map.tile_center(24, 24);
    let scout_pos = game.map.tile_center(26, 24);
    let far_pos = game.map.tile_center(4, 50);
    let trench = game
        .spawn_trench_for_test(trench_pos.0, trench_pos.1)
        .expect("trench should seed");
    let occupant = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, trench_pos.0, trench_pos.1)
        .expect("occupant should spawn");
    let scout = game
        .entities
        .spawn_unit(1, EntityKind::Worker, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    repair_world(&mut game);

    game.tick();
    let visible = game.snapshot_for(1);
    assert!(visible
        .entities
        .iter()
        .any(|view| view.id == occupant && view.occupied_trench_id == Some(trench)));

    game.entities.remove(scout);
    game.entities
        .spawn_unit(1, EntityKind::Worker, far_pos.0, far_pos.1)
        .expect("far scout should spawn");
    game.tick();

    let hidden = game.snapshot_for(1);
    assert!(
        hidden.trenches.iter().any(|view| view.id == trench),
        "remembered trench terrain should stay visible"
    );
    assert!(
        !hidden.entities.iter().any(|view| view.id == occupant),
        "hidden occupied unit should not be projected"
    );
}
