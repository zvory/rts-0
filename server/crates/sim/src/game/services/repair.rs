//! Worker building repair. Prepaid fractional costs live on buildings, so repeated damage,
//! replacement orders, and additional workers cannot reset billing or multiply rounding overhead.
use super::{dist2, interact_range_for_kind};
use crate::config;
use crate::game::entity::{EntityKind, EntityStore, Order};
use crate::game::PlayerState;
use crate::rules::economy::ResourceCost;

pub(in crate::game) fn repair_system(entities: &mut EntityStore, players: &mut [PlayerState]) {
    let mut workers: Vec<_> = entities
        .iter()
        .filter_map(|w| match w.order() {
            Order::Repair(order) if w.hp > 0 => Some((w.id, w.owner, order.intent.target)),
            _ => None,
        })
        .collect();
    workers.sort_unstable_by_key(|worker| worker.0);
    for &(id, owner, target) in &workers {
        let valid = matches!(entities.get(id), Some(w) if w.kind == EntityKind::Worker)
            && matches!(entities.get(target), Some(t) if t.can_be_repaired_by(owner));
        if !valid {
            if let Some(w) = entities.get_mut(id) {
                w.clear_active_order();
            }
            continue;
        }
        let in_range = match (entities.get(id), entities.get(target)) {
            (Some(w), Some(t)) => {
                let range = interact_range_for_kind(t.kind);
                dist2(w.pos_x(), w.pos_y(), t.pos_x(), t.pos_y()) <= range * range
            }
            _ => false,
        };
        let Some(w) = entities.get_mut(id) else {
            continue;
        };
        if !in_range {
            if let Some(m) = w.movement.as_mut() {
                if let Order::Repair(order) = &mut m.order {
                    order.ticks = 0;
                }
            }
            continue;
        }
        w.clear_path();
        let Some(m) = w.movement.as_mut() else {
            continue;
        };
        let Order::Repair(order) = &mut m.order else {
            continue;
        };
        let interval = config::TICK_HZ / config::BUILDING_REPAIR_HP_PER_SECOND;
        order.ticks = order.ticks.saturating_add(1).min(interval);
        if order.ticks < interval {
            continue;
        }
        let Some(t) = entities.get(target) else {
            continue;
        };
        let base = crate::rules::economy::resource_cost(t.kind);
        let credit = t.repair_credit.unwrap_or_default();
        let (steel, steel_credit) = hp_cost(base.steel, credit[0], t.max_hp);
        let (oil, oil_credit) = hp_cost(base.oil, credit[1], t.max_hp);
        let cost = ResourceCost::new(steel, oil);
        let Some(player) = players.iter_mut().find(|p| p.id == owner) else {
            continue;
        };
        if !player.spend_cost(cost) {
            continue;
        }
        if let Some(t) = entities.get_mut(target) {
            t.restore_hp(1);
            t.repair_credit = Some([steel_credit, oil_credit]);
        }
        if let Some(w) = entities.get_mut(id) {
            if let Some(m) = w.movement.as_mut() {
                if let Order::Repair(order) = &mut m.order {
                    order.ticks = 0;
                }
            }
        }
    }
    // Release every repairer together when the final worker finishes the building, so queued
    // orders can promote on the next tick regardless of worker iteration order.
    for (id, _, target) in workers {
        if entities.get(target).is_none_or(|t| t.hp >= t.max_hp) {
            if let Some(w) = entities.get_mut(id) {
                w.clear_active_order();
            }
        }
    }
}

fn hp_cost(price: u32, credit: u64, max_hp: u32) -> (u32, u64) {
    let denominator = u64::from(max_hp.max(1)) * u64::from(config::BUILDING_REPAIR_COST_DIVISOR);
    let needed = u64::from(price).saturating_sub(credit);
    let charge = needed.div_ceil(denominator);
    (
        charge as u32,
        credit
            .saturating_add(charge * denominator)
            .saturating_sub(u64::from(price)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_billing_preserves_fractional_credit_across_repeated_small_repairs() {
        for (price, max_hp) in [(150, 600), (100, 500), (0, 500), (145, 800)] {
            let (mut credit, mut total) = (0, 0);
            for _ in 0..max_hp {
                let (charge, next) = hp_cost(price, credit, max_hp);
                total += charge;
                credit = next;
            }
            assert_eq!(total, price.div_ceil(config::BUILDING_REPAIR_COST_DIVISOR));
            assert!(credit < u64::from(max_hp) * u64::from(config::BUILDING_REPAIR_COST_DIVISOR));
        }
    }
}
