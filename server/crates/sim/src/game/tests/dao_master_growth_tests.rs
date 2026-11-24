use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;
use crate::game::services::{geometry, occupancy::Occupancy, standability};

fn player() -> PlayerInit {
    PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::CULTIVATORS_FACTION_ID.into(),
        name: "Growth".into(),
        color: "#fff".into(),
        is_ai: false,
    }
}

#[test]
fn dao_master_keeps_corner_waypoint_that_only_an_adept_can_skip() {
    let mut game = empty_flat_game(&[player()]);
    game.state
        .entities
        .spawn_building(1, EntityKind::Portal, 592.0, 592.0, true)
        .unwrap();
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::WindAdept, 720.0, 600.0)
        .unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = 200;
    unit.advance_dao_master();
    unit.set_order(Order::move_to(600.0, 720.0));
    unit.set_path(vec![(600.0, 720.0), (720.0, 720.0)]);
    unit.set_path_goal(Some((600.0, 720.0)));
    unit.mark_move_phase(crate::game::entity::MovePhase::Moving);
    game.tick();
    let unit = game.state.entities.get(id).unwrap();
    assert_eq!(
        unit.next_waypoint(),
        Some((720.0, 720.0)),
        "the diagonal clips the enlarged body on the Portal corner"
    );
    assert_eq!(unit.pos_x(), 720.0);
    assert!(unit.pos_y() > 600.0);
    for _ in 0..150 {
        game.tick();
        let unit = game.state.entities.get(id).unwrap();
        let occ = Occupancy::build(&game.state.map, &game.state.entities);
        assert!(standability::unit_static_standable_with_radius(
            &game.state.map,
            &occ,
            unit.kind,
            unit.pos_x(),
            unit.pos_y(),
            unit.facing(),
            Some(unit.radius()),
        ));
    }
    let unit = game.state.entities.get(id).unwrap();
    assert_eq!((unit.pos_x(), unit.pos_y()), (600.0, 720.0));
}

#[test]
fn dao_master_growth_near_portal_repositions_without_losing_orders_or_reload() {
    for ability in [
        AbilityKind::EvolveWind,
        AbilityKind::EvolveWar,
        AbilityKind::EvolveMagic,
    ] {
        let mut game = empty_flat_game(&[player()]);
        let building = game
            .state
            .entities
            .spawn_building(1, EntityKind::Portal, 592.0, 592.0, true)
            .unwrap();
        let id = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Warrior, 700.0, 600.0)
            .unwrap();
        let unit = game.state.entities.get_mut(id).unwrap();
        unit.insight = 60;
        unit.evolve_dao(ability);
        let original_radius = unit.radius();
        unit.set_position(640.0 + original_radius + 0.5, 600.0);
        unit.insight = 200;
        unit.apply_damage(unit.hp - 1, None);
        unit.set_order(Order::HoldPosition);
        unit.set_attack_cd(30);
        game.tick();
        let unit = game.state.entities.get(id).unwrap();
        assert!(unit.dao_master);
        assert_eq!(unit.hp, unit.max_hp);
        assert_eq!(unit.attack_cd(), 29);
        assert!(matches!(unit.order(), Order::HoldPosition));
        assert!(unit.pos_x() > 640.0 + unit.radius());
        let rect = geometry::building_rect_for_entity(
            &game.state.map,
            game.state.entities.get(building).unwrap(),
        )
        .unwrap();
        assert!(!geometry::unit_body_intersects_rect(
            geometry::unit_body_for_entity(unit).unwrap(),
            rect
        ));
        let occ = Occupancy::build(&game.state.map, &game.state.entities);
        assert!(standability::unit_static_standable_with_radius(
            &game.state.map,
            &occ,
            unit.kind,
            unit.pos_x(),
            unit.pos_y(),
            unit.facing(),
            Some(unit.radius())
        ));
    }
}

#[test]
fn dao_master_movement_and_collision_do_not_push_body_inside_portal() {
    let mut game = empty_flat_game(&[player()]);
    game.state
        .entities
        .spawn_building(1, EntityKind::Portal, 592.0, 592.0, true)
        .unwrap();
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 680.0, 600.0)
        .unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = 60;
    unit.evolve_dao(AbilityKind::EvolveWind);
    unit.insight = 200;
    game.tick();
    game.enqueue(
        1,
        Command::Move {
            units: vec![id],
            x: 600.0,
            y: 600.0,
            queued: false,
        },
    );
    for _ in 0..40 {
        game.tick();
        let unit = game.state.entities.get(id).unwrap();
        let occ = Occupancy::build(&game.state.map, &game.state.entities);
        assert!(standability::unit_static_standable_with_radius(
            &game.state.map,
            &occ,
            unit.kind,
            unit.pos_x(),
            unit.pos_y(),
            unit.facing(),
            Some(unit.radius())
        ));
    }
    // A large overlapping neighbour must absorb a push that would put the Master in the wall.
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.set_position(672.0, 600.0);
    unit.hold_position();
    let other = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 700.0, 600.0)
        .unwrap();
    game.state.entities.get_mut(other).unwrap().hold_position();
    game.tick();
    assert!(game.state.entities.get(id).unwrap().pos_x() > 640.0 + 31.2);
}

#[test]
fn dao_master_dash_and_return_reject_adept_sized_building_clearance() {
    let mut game = empty_flat_game(&[player()]);
    game.state
        .entities
        .spawn_building(1, EntityKind::Portal, 592.0, 592.0, true)
        .unwrap();
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::WindAdept, 800.0, 600.0)
        .unwrap();
    game.state.entities.get_mut(id).unwrap().insight = 200;
    game.tick();
    assert!(crate::game::hero_abilities::ekat_dash_destination(
        &game.state.map,
        &game.state.entities,
        id,
        666.5,
        600.0,
        Some(10.0)
    )
    .is_none());
    assert!(!crate::game::hero_abilities::ekat_return_destination_valid(
        &game.state.map,
        &game.state.entities,
        id,
        666.5,
        600.0
    ));
}

#[test]
fn dao_master_lab_moves_reject_adept_clearance_atomically() {
    use crate::game::lab::{LabMoveEntity, LabOp, LabUpdate};
    let mut game = empty_flat_game(&[player()]);
    game.state
        .entities
        .spawn_building(1, EntityKind::Portal, 592.0, 592.0, true)
        .unwrap();
    let id = game
        .state
        .entities
        .spawn_unit(1, EntityKind::WindAdept, 800.0, 600.0)
        .unwrap();
    game.state.entities.get_mut(id).unwrap().insight = 200;
    game.tick();
    let movement = LabMoveEntity {
        entity_id: id,
        x: 666.5,
        y: 600.0,
    };
    assert!(game.apply_lab_op(LabOp::MoveEntity(movement)).is_err());
    assert!(game
        .apply_lab_op(LabOp::ApplyUpdates(vec![LabUpdate::Move(movement)]))
        .is_err());
    let unit = game.state.entities.get(id).unwrap();
    assert_eq!((unit.pos_x(), unit.pos_y()), (800.0, 600.0));
    game.tick();
}
