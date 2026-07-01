use super::*;
use crate::game::entity::{EntityKind, Order, OrderIntent, WeaponSetup};
use crate::game::map::Map;

fn lab_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".to_string(),
            color: "#4878c8".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Bravo".to_string(),
            color: "#c84848".to_string(),
            is_ai: false,
        },
    ]
}

fn default_map_game() -> Game {
    let players = lab_players();
    let start_players: Vec<_> = players
        .iter()
        .map(|player| (player.id, player.team_id))
        .collect();
    let map = Map::load_for_players("Default", &start_players, 0xABCD).expect("default lab map");
    let metadata = Map::metadata_for_name("Default").expect("default map metadata");
    Game::new_lab(&players, 0xABCD, map, metadata)
}

fn tile_center(game: &Game, x: u32, y: u32) -> (f32, f32) {
    game.state.map.tile_center(x, y)
}

fn free_unit_position(game: &Game, kind: EntityKind) -> (f32, f32) {
    for ty in 8..game.state.map.size.saturating_sub(8) {
        for tx in 8..game.state.map.size.saturating_sub(8) {
            let (x, y) = game.state.map.tile_center(tx, ty);
            if game
                .validate_unit_position(&game.state.entities, kind, x, y)
                .is_ok()
            {
                return (x, y);
            }
        }
    }
    panic!("no free position found for {kind:?}");
}

#[test]
fn lab_scenario_export_restores_active_and_queued_orders() {
    let mut game = default_map_game();
    let (point_x, point_y) = tile_center(&game, 48, 16);
    let (blanket_x, blanket_y) = tile_center(&game, 48, 24);
    let (x, y) = free_unit_position(&game, EntityKind::Artillery);
    let LabOpOutcome::Spawned {
        entity_id: point_artillery,
    } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Artillery,
            x,
            y,
            completed: true,
        }))
        .expect("artillery should spawn")
    else {
        panic!("unexpected outcome");
    };
    {
        let artillery = game
            .state
            .entities
            .get_mut(point_artillery)
            .expect("spawned artillery");
        artillery.set_weapon_setup(WeaponSetup::Deployed);
        artillery.replace_active_order(Order::artillery_point_fire(point_x, point_y));
        assert!(artillery.append_queued_order(OrderIntent::blanket_fire(blanket_x, blanket_y)));
    }

    let (x, y) = free_unit_position(&game, EntityKind::Artillery);
    let LabOpOutcome::Spawned {
        entity_id: blanket_artillery,
    } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Artillery,
            x,
            y,
            completed: true,
        }))
        .expect("artillery should spawn")
    else {
        panic!("unexpected outcome");
    };
    {
        let artillery = game
            .state
            .entities
            .get_mut(blanket_artillery)
            .expect("spawned artillery");
        artillery.set_weapon_setup(WeaponSetup::Deployed);
        artillery.replace_active_order(Order::artillery_blanket_fire(blanket_x, blanket_y));
    }

    let scenario = game.export_lab_scenario();
    let exported_point = scenario
        .entities
        .iter()
        .find(|entity| entity.id == point_artillery)
        .expect("exported point artillery");
    assert_eq!(
        exported_point.order.as_ref().map(|order| order.kind.as_str()),
        Some("pointFire")
    );
    assert_eq!(exported_point.queued_orders.len(), 1);
    assert_eq!(exported_point.queued_orders[0].kind, "blanketFire");
    let exported_blanket = scenario
        .entities
        .iter()
        .find(|entity| entity.id == blanket_artillery)
        .expect("exported blanket artillery");
    assert_eq!(
        exported_blanket
            .order
            .as_ref()
            .map(|order| order.kind.as_str()),
        Some("blanketFire")
    );

    let mut restored = default_map_game();
    let LabOpOutcome::ScenarioRestored(result) = restored
        .apply_lab_op(LabOp::RestoreScenario(Box::new(scenario)))
        .expect("scenario restore should succeed")
    else {
        panic!("unexpected outcome");
    };
    let point_id = result
        .entity_id_map
        .iter()
        .find(|entry| entry.old_id == point_artillery)
        .expect("point artillery should be remapped")
        .new_id;
    let restored_point = restored
        .state
        .entities
        .get(point_id)
        .expect("restored point artillery");
    assert!(matches!(
        restored_point.order(),
        Order::ArtilleryPointFire(order) if order.intent.x == point_x && order.intent.y == point_y
    ));
    assert!(matches!(
        restored_point.queued_orders(),
        [OrderIntent::BlanketFire(point)] if point.x == blanket_x && point.y == blanket_y
    ));
    let blanket_id = result
        .entity_id_map
        .iter()
        .find(|entry| entry.old_id == blanket_artillery)
        .expect("blanket artillery should be remapped")
        .new_id;
    let restored_blanket = restored
        .state
        .entities
        .get(blanket_id)
        .expect("restored blanket artillery");
    assert!(matches!(
        restored_blanket.order(),
        Order::ArtilleryBlanketFire(order)
            if order.intent.x == blanket_x && order.intent.y == blanket_y
    ));
}

#[test]
fn lab_scenario_restore_hydrates_active_order_runtime_state() {
    let mut game = default_map_game();
    let (move_x, move_y) = tile_center(&game, 44, 18);
    let (x, y) = free_unit_position(&game, EntityKind::Rifleman);
    let LabOpOutcome::Spawned { entity_id: mover } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x,
            y,
            completed: true,
        }))
        .expect("rifleman should spawn")
    else {
        panic!("unexpected outcome");
    };
    {
        let mover = game.state.entities.get_mut(mover).expect("spawned mover");
        mover.replace_active_order(Order::move_to(move_x, move_y));
    }

    let (x, y) = free_unit_position(&game, EntityKind::Worker);
    let LabOpOutcome::Spawned { entity_id: builder } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Worker,
            x,
            y,
            completed: true,
        }))
        .expect("worker should spawn")
    else {
        panic!("unexpected outcome");
    };
    let (build_tile_x, build_tile_y) = (36, 36);
    {
        let builder = game.state.entities.get_mut(builder).expect("spawned builder");
        builder.replace_active_order(Order::build(
            EntityKind::Depot,
            build_tile_x,
            build_tile_y,
        ));
    }

    let scenario = game.export_lab_scenario();
    let mut restored = default_map_game();
    let LabOpOutcome::ScenarioRestored(result) = restored
        .apply_lab_op(LabOp::RestoreScenario(Box::new(scenario)))
        .expect("scenario restore should succeed")
    else {
        panic!("unexpected outcome");
    };
    let restored_mover_id = result
        .entity_id_map
        .iter()
        .find(|entry| entry.old_id == mover)
        .expect("mover should be remapped")
        .new_id;
    let restored_mover = restored
        .state
        .entities
        .get(restored_mover_id)
        .expect("restored mover");
    assert!(matches!(
        restored_mover.order(),
        Order::Move(order) if order.intent.x == move_x && order.intent.y == move_y
    ));
    assert_eq!(restored_mover.path_goal(), Some((move_x, move_y)));

    let restored_builder_id = result
        .entity_id_map
        .iter()
        .find(|entry| entry.old_id == builder)
        .expect("builder should be remapped")
        .new_id;
    let restored_builder = restored
        .state
        .entities
        .get(restored_builder_id)
        .expect("restored builder");
    assert!(matches!(
        restored_builder.order(),
        Order::Build(order)
            if order.intent.kind == EntityKind::Depot
                && order.intent.tile_x == build_tile_x
                && order.intent.tile_y == build_tile_y
    ));
    assert!(
        restored_builder.path_goal().is_some(),
        "active build restore should request a construction staging path"
    );
}
