use crate::game::entity::{AttackPhase, Entity, EntityKind, EntityStore, MovePhase, Order};

use super::pivot_drive::{angle_delta, rotate_toward, vehicle_body_turn_rate};

const FACING_EPS_RAD: f32 = 1.0e-4;

pub(super) fn turn_stationary_tanks_toward_locked_ap_source<F>(
    entities: &mut EntityStore,
    tick: u32,
    mut turn_is_allowed: F,
) where
    F: FnMut(u32, EntityKind, f32, f32, f32) -> bool,
{
    for id in entities.ids() {
        if let Some(combat) = entities.get_mut(id).and_then(|tank| tank.combat.as_mut()) {
            if combat.tank_armor_reaction_lock.is_some_and(|lock| {
                tick.saturating_sub(lock.acquired_tick)
                    >= crate::rules::combat::TANK_ARMOR_REACTION_LOCK_TICKS
            }) {
                combat.tank_armor_reaction_lock = None;
            }
        }

        let Some((owner, kind, x, y, current, desired)) = entities.get(id).and_then(|tank| {
            tank_can_react(tank).then(|| {
                locked_source_facing(tank).map(|desired| {
                    (
                        tank.owner,
                        tank.kind,
                        tank.pos_x,
                        tank.pos_y,
                        tank.facing(),
                        desired,
                    )
                })
            })?
        }) else {
            continue;
        };

        let rotated = rotate_toward(current, desired, vehicle_body_turn_rate(kind));
        if angle_delta(current, rotated).abs() <= FACING_EPS_RAD
            || !turn_is_allowed(owner, kind, x, y, rotated)
        {
            continue;
        }
        if let Some(tank) = entities.get_mut(id) {
            tank.set_facing(rotated);
        }
    }
}

fn tank_can_react(tank: &Entity) -> bool {
    if !crate::rules::combat::unit_uses_tank_armor_reaction(tank.kind)
        || tank.hp == 0
        || !tank.path_is_empty()
    {
        return false;
    }
    match tank.order() {
        Order::Idle | Order::HoldPosition => true,
        Order::Attack(order) => order.execution.phase == AttackPhase::Firing,
        Order::AttackMove(_) => tank.move_phase() == Some(MovePhase::Arrived),
        Order::Move(_)
        | Order::Gather(_)
        | Order::Build(_)
        | Order::Deconstruct(_)
        | Order::Ability(_)
        | Order::ArtilleryPointFire(_)
        | Order::ArtilleryBlanketFire { .. } => false,
    }
}

pub(super) fn locked_source_facing(tank: &Entity) -> Option<f32> {
    let lock = tank.combat.as_ref()?.tank_armor_reaction_lock?;
    let dx = lock.source_x - tank.pos_x;
    let dy = lock.source_y - tank.pos_y;
    (dx.is_finite() && dy.is_finite() && dx.hypot(dy) > f32::EPSILON).then(|| dy.atan2(dx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::game::entity::EntityStore;

    fn turn_once(entities: &mut EntityStore, tick: u32) {
        turn_stationary_tanks_toward_locked_ap_source(entities, tick, |_, _, _, _, _| true);
    }

    #[test]
    fn held_tank_turns_toward_first_source_without_losing_range() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 300.0, 300.0)
            .expect("tank should spawn");
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.hold_position();
        tank.set_facing(0.0);
        tank.set_target_id(Some(99));
        tank.set_weapon_facing(-0.5);
        tank.combat
            .as_mut()
            .expect("tank should have combat")
            .tank_stationary_range_ticks = config::TICK_HZ as u16 * 3;
        tank.lock_tank_armor_reaction_source((300.0, 400.0), 10);
        tank.lock_tank_armor_reaction_source((200.0, 300.0), 11);

        turn_once(&mut entities, 11);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert!(matches!(tank.order(), Order::HoldPosition));
        assert_eq!(tank.target_id(), Some(99));
        assert_eq!(tank.weapon_facing(), Some(-0.5));
        assert!(
            (tank.facing() - vehicle_body_turn_rate(EntityKind::Tank)).abs() <= 0.0001,
            "the second source must not redirect the hull"
        );
        assert_eq!(
            tank.combat
                .as_ref()
                .expect("tank should have combat")
                .tank_stationary_range_ticks,
            config::TICK_HZ as u16 * 3
        );
    }

    #[test]
    fn later_hits_refresh_without_redirecting_the_lock() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 300.0, 300.0)
            .expect("tank should spawn");
        {
            let tank = entities.get_mut(tank_id).expect("tank should exist");
            tank.lock_tank_armor_reaction_source((400.0, 300.0), 10);
            tank.lock_tank_armor_reaction_source((300.0, 400.0), 11);
            tank.lock_tank_armor_reaction_source(
                (200.0, 300.0),
                10 + crate::rules::combat::TANK_ARMOR_REACTION_LOCK_TICKS - 1,
            );
        }

        let lock = entities
            .get(tank_id)
            .expect("tank should exist")
            .combat
            .as_ref()
            .and_then(|combat| combat.tank_armor_reaction_lock)
            .expect("first source should remain locked");
        assert_eq!(
            (lock.source_x, lock.source_y, lock.acquired_tick),
            (
                400.0,
                300.0,
                10 + crate::rules::combat::TANK_ARMOR_REACTION_LOCK_TICKS - 1
            )
        );

        let expiry_tick = lock.acquired_tick + crate::rules::combat::TANK_ARMOR_REACTION_LOCK_TICKS;
        turn_once(&mut entities, expiry_tick);
        assert!(entities
            .get(tank_id)
            .and_then(|tank| tank.combat.as_ref())
            .is_some_and(|combat| combat.tank_armor_reaction_lock.is_none()));

        entities
            .get_mut(tank_id)
            .expect("tank should exist")
            .lock_tank_armor_reaction_source((300.0, 400.0), expiry_tick);
        let lock = entities
            .get(tank_id)
            .expect("tank should exist")
            .combat
            .as_ref()
            .and_then(|combat| combat.tank_armor_reaction_lock)
            .expect("expired lock should be replaced");
        assert_eq!(
            (lock.source_x, lock.source_y, lock.acquired_tick),
            (300.0, 400.0, expiry_tick)
        );
    }

    #[test]
    fn active_movement_takes_precedence() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 300.0, 300.0)
            .expect("tank should spawn");
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.set_facing(0.0);
        tank.set_order(Order::move_to(500.0, 300.0));
        tank.set_path(vec![(500.0, 300.0)]);
        tank.set_path_goal(Some((500.0, 300.0)));
        tank.mark_move_phase(MovePhase::Moving);
        tank.lock_tank_armor_reaction_source((300.0, 400.0), 10);

        turn_once(&mut entities, 11);

        assert!(entities
            .get(tank_id)
            .is_some_and(|tank| tank.facing().abs() <= 0.0001));
    }

    #[test]
    fn rejected_hull_turn_does_not_change_facing_or_position() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 15.0, 300.0)
            .expect("tank should spawn");
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.set_facing(std::f32::consts::FRAC_PI_2);
        tank.lock_tank_armor_reaction_source((0.0, 300.0), 10);
        let before = (tank.pos_x, tank.pos_y, tank.facing());

        let mut checked_legality = false;
        turn_stationary_tanks_toward_locked_ap_source(&mut entities, 11, |owner, kind, x, y, _| {
            checked_legality = true;
            assert_eq!((owner, kind, x, y), (1, EntityKind::Tank, 15.0, 300.0));
            false
        });

        let tank = entities.get(tank_id).expect("tank should exist");
        assert!(checked_legality);
        assert_eq!((tank.pos_x, tank.pos_y, tank.facing()), before);
    }
}
