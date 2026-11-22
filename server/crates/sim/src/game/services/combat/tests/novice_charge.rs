use super::*;
use crate::game::ability::AbilityKind;

fn pair(distance: f32) -> (EntityStore, u32, u32) {
    let mut entities = EntityStore::new();
    let novice = entities.spawn_unit(1, EntityKind::Warrior, 200.0, 200.0).unwrap();
    let target = entities.spawn_unit(2, EntityKind::Rifleman, 200.0 + distance, 200.0).unwrap();
    entities.get_mut(novice).unwrap().set_order(Order::attack(target));
    entities.get_mut(target).unwrap().set_order(Order::HoldPosition);
    (entities, novice, target)
}

#[test]
fn novice_charge_distance_and_retreat_trigger_matrix() {
    let two_tiles = (2 * config::TILE_SIZE) as f32;
    for (distance, delta, expected) in [
        (two_tiles, (0.0, 0.0), true),
        (two_tiles - 1.0, (0.0, 0.0), false),
        (40.0, (1.0, 0.0), true),
        (40.0, (-1.0, 0.0), false),
        (40.0, (0.0, 1.0), false),
    ] {
        let (mut entities, novice, target) = pair(distance);
        entities.get_mut(target).unwrap().set_movement_delta(delta.0, delta.1);
        run_combat_tick(&mut entities);
        let unit = entities.get(novice).unwrap();
        assert_eq!(unit.ability_cooldown_ticks(AbilityKind::Charge) > 0, expected,
            "distance={distance}, movement={delta:?}");
        assert_eq!(unit.movement_speed(), if expected { 3.0 } else { 2.0 });
    }
}

#[test]
fn novice_charge_automatic_acquisition_and_attack_move_trigger() {
    for order in [Order::Idle, Order::attack_move_to(500.0, 200.0)] {
        let (mut entities, novice, _) = pair(100.0);
        entities.get_mut(novice).unwrap().set_order(order);
        run_combat_tick(&mut entities);
        assert_eq!(entities.get(novice).unwrap().ability_cooldown_ticks(AbilityKind::Charge), 450);
    }
}

#[test]
fn novice_charge_never_triggers_for_buildings_friends_or_specialists() {
    for case in 0..3 {
        let (mut entities, novice, _) = pair(100.0);
        match case {
            0 => {
                let building = entities.spawn_building(2, EntityKind::Depot, 300.0, 200.0, true).unwrap();
                entities.get_mut(novice).unwrap().set_order(Order::attack(building));
            }
            1 => {
                let friendly = entities.spawn_unit(1, EntityKind::Rifleman, 300.0, 200.0).unwrap();
                entities.get_mut(novice).unwrap().set_order(Order::attack(friendly));
            }
            _ => entities.get_mut(novice).unwrap().dao = Some(UpgradeKind::DaoOfLightness),
        }
        run_combat_tick(&mut entities);
        assert_eq!(entities.get(novice).unwrap().ability_cooldown_ticks(AbilityKind::Charge), 0);
    }
}

#[test]
fn novice_charge_lasts_thirty_movement_ticks_and_rearms_after_450() {
    let (mut entities, novice, _) = pair(100.0);
    run_combat_tick(&mut entities);
    let map = open_map(32);
    let players = Vec::new();
    for tick in 1..=450 {
        let unit = entities.get(novice).unwrap();
        assert_eq!(unit.movement_speed(), if tick <= 30 { 3.0 } else { 2.0 });
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &players, &occ, &spatial, tick);
        // Use a fresh distant hostile each time to avoid combat killing the original.
        entities.get_mut(novice).unwrap().try_start_novice_charge((1000.0, 200.0), (0.0, 0.0), true);
        assert_eq!(entities.get(novice).unwrap().ability_cooldown_ticks(AbilityKind::Charge),
            if tick == 450 { 450 } else { 450 - tick as u16 });
    }
    let unit = entities.get(novice).unwrap();
    let serialized = serde_json::to_string(unit).unwrap();
    let restored: crate::game::entity::Entity = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored.movement_speed(), 3.0);
    assert_eq!(restored.ability_cooldown_ticks(AbilityKind::Charge), 450);
}
