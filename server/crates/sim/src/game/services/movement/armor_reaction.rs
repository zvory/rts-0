use crate::game::entity::{AttackPhase, Entity, EntityKind, EntityStore, MovePhase, Order};

use super::pivot_drive::{angle_delta, rotate_toward, vehicle_body_turn_rate};

const MIN_DIRECTION_COHERENCE: f32 = 0.2;
const FACING_EPS_RAD: f32 = 1.0e-4;

pub(super) fn turn_stationary_units_toward_direct_ap_threats<F>(
    entities: &mut EntityStore,
    tick: u32,
    mut turn_is_allowed: F,
) where
    F: FnMut(u32, EntityKind, f32, f32, f32) -> bool,
{
    for id in entities.ids() {
        if let Some(unit) = entities.get_mut(id) {
            let Some(combat) = unit.combat.as_mut() else {
                continue;
            };
            combat.incoming_direct_ap_threats.retain(|_, threat| {
                tick.saturating_sub(threat.last_hit_tick)
                    <= crate::rules::combat::DIRECT_AP_ARMOR_REACTION_MEMORY_TICKS
            });
        }

        let Some((owner, desired)) = entities.get(id).and_then(|unit| {
            unit_can_react(unit)
                .then(|| desired_threat_facing(unit).map(|desired| (unit.owner, desired)))?
        }) else {
            continue;
        };
        let Some((kind, x, y, current)) = entities
            .get(id)
            .map(|unit| (unit.kind, unit.pos_x, unit.pos_y, unit.facing()))
        else {
            continue;
        };
        let rotated = rotate_toward(current, desired, vehicle_body_turn_rate(kind));
        if angle_delta(current, rotated).abs() <= FACING_EPS_RAD
            || !turn_is_allowed(owner, kind, x, y, rotated)
        {
            continue;
        }
        if let Some(unit) = entities.get_mut(id) {
            unit.set_facing(rotated);
        }
    }
}

fn unit_can_react(unit: &Entity) -> bool {
    if !crate::rules::combat::unit_reacts_to_direct_ap(unit.kind)
        || unit.hp == 0
        || !unit.path_is_empty()
    {
        return false;
    }
    match unit.order() {
        Order::Idle | Order::HoldPosition => true,
        Order::Attack(order) => order.execution.phase == AttackPhase::Firing,
        Order::AttackMove(_) => unit.move_phase() == Some(MovePhase::Arrived),
        Order::Move(_)
        | Order::Gather(_)
        | Order::Build(_)
        | Order::Deconstruct(_)
        | Order::Ability(_)
        | Order::ArtilleryPointFire(_)
        | Order::ArtilleryBlanketFire(_) => false,
    }
}

fn desired_threat_facing(unit: &Entity) -> Option<f32> {
    let combat = unit.combat.as_ref()?;
    let mut weighted_x = 0.0_f32;
    let mut weighted_y = 0.0_f32;
    let mut total_weight = 0.0_f32;
    for threat in combat.incoming_direct_ap_threats.values() {
        let dx = threat.source_x - unit.pos_x;
        let dy = threat.source_y - unit.pos_y;
        let distance = (dx * dx + dy * dy).sqrt();
        let weight = threat.damage_weight as f32;
        if !distance.is_finite() || distance <= f32::EPSILON || weight <= 0.0 {
            continue;
        }
        weighted_x += dx / distance * weight;
        weighted_y += dy / distance * weight;
        total_weight += weight;
    }
    if total_weight <= 0.0 {
        return None;
    }
    let magnitude = (weighted_x * weighted_x + weighted_y * weighted_y).sqrt();
    if !magnitude.is_finite() || magnitude / total_weight < MIN_DIRECTION_COHERENCE {
        return None;
    }
    Some(weighted_y.atan2(weighted_x))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::game::entity::EntityStore;

    fn turn_once(entities: &mut EntityStore, tick: u32) {
        turn_stationary_units_toward_direct_ap_threats(entities, tick, |_, _, _, _, _| true);
    }

    #[test]
    fn held_tank_turns_toward_distinct_source_average_without_losing_range() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 300.0, 300.0)
            .expect("tank should spawn");
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.hold_position();
        tank.set_facing(0.0);
        tank.set_target_id(Some(99));
        tank.set_weapon_facing(-0.5);
        tank.set_desired_weapon_facing(-0.25);
        tank.combat
            .as_mut()
            .expect("tank should have combat")
            .tank_stationary_range_ticks = config::TICK_HZ as u16 * 3;
        tank.record_incoming_direct_ap_threat(11, (400.0, 300.0), 100, 10);
        tank.record_incoming_direct_ap_threat(12, (300.0, 400.0), 100, 10);
        assert!(
            (desired_threat_facing(tank).expect("average should resolve")
                - std::f32::consts::FRAC_PI_4)
                .abs()
                <= 0.0001
        );

        turn_once(&mut entities, 11);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert!(matches!(tank.order(), Order::HoldPosition));
        assert_eq!(tank.target_id(), Some(99));
        assert_eq!(tank.weapon_facing(), Some(-0.5));
        assert!(
            (tank.facing() - vehicle_body_turn_rate(EntityKind::Tank)).abs() <= 0.0001,
            "tank should begin turning toward the 45-degree source average"
        );
        assert_eq!(
            tank.combat
                .as_ref()
                .expect("tank should have combat")
                .tank_stationary_range_ticks,
            config::TICK_HZ as u16 * 3,
            "in-place armor response should preserve stationary range"
        );
    }

    #[test]
    fn repeated_source_updates_instead_of_overweighting_old_direction() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 300.0, 300.0)
            .expect("tank should spawn");
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.set_facing(std::f32::consts::FRAC_PI_4);
        tank.record_incoming_direct_ap_threat(11, (400.0, 300.0), 100, 10);
        tank.record_incoming_direct_ap_threat(11, (300.0, 400.0), 100, 11);

        turn_once(&mut entities, 12);

        let tank = entities.get(tank_id).expect("tank should exist");
        assert!(
            (tank.facing()
                - (std::f32::consts::FRAC_PI_4 + vehicle_body_turn_rate(EntityKind::Tank)))
            .abs()
                <= 0.0001,
            "the refreshed source should point north, not average with its stale position"
        );
        assert_eq!(
            tank.combat
                .as_ref()
                .expect("tank should have combat")
                .incoming_direct_ap_threats
                .len(),
            1
        );
    }

    #[test]
    fn opposing_equal_threats_do_not_make_tank_jiggle() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 300.0, 300.0)
            .expect("tank should spawn");
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.set_facing(0.4);
        tank.record_incoming_direct_ap_threat(11, (400.0, 300.0), 100, 10);
        tank.record_incoming_direct_ap_threat(12, (200.0, 300.0), 100, 10);

        turn_once(&mut entities, 11);

        assert!((entities.get(tank_id).expect("tank should exist").facing() - 0.4).abs() <= 0.0001);
    }

    #[test]
    fn active_move_path_takes_precedence_and_expired_threat_is_pruned() {
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
        tank.record_incoming_direct_ap_threat(11, (300.0, 400.0), 100, 10);

        turn_once(&mut entities, 11);
        assert!(
            entities
                .get(tank_id)
                .expect("tank should exist")
                .facing()
                .abs()
                <= 0.0001
        );

        turn_once(
            &mut entities,
            10 + crate::rules::combat::DIRECT_AP_ARMOR_REACTION_MEMORY_TICKS + 1,
        );
        assert!(entities
            .get(tank_id)
            .expect("tank should exist")
            .combat
            .as_ref()
            .expect("tank should have combat")
            .incoming_direct_ap_threats
            .is_empty());
    }

    #[test]
    fn rejected_hull_turn_does_not_change_facing_or_position() {
        let mut entities = EntityStore::new();
        let tank_id = entities
            .spawn_unit(1, EntityKind::Tank, 15.0, 300.0)
            .expect("tank should spawn");
        let tank = entities.get_mut(tank_id).expect("tank should exist");
        tank.set_facing(std::f32::consts::FRAC_PI_2);
        tank.record_incoming_direct_ap_threat(11, (0.0, 300.0), 100, 10);
        let before = (tank.pos_x, tank.pos_y, tank.facing());

        let mut checked_legality = false;
        turn_stationary_units_toward_direct_ap_threats(
            &mut entities,
            11,
            |owner, kind, x, y, _| {
                checked_legality = true;
                assert_eq!((owner, kind, x, y), (1, EntityKind::Tank, 15.0, 300.0));
                false
            },
        );

        let tank = entities.get(tank_id).expect("tank should exist");
        assert!(checked_legality);
        assert_eq!((tank.pos_x, tank.pos_y, tank.facing()), before);
    }
}
