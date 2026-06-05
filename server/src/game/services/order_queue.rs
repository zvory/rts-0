use std::collections::BTreeMap;

use crate::game::entity::{Entity, EntityStore, MovePhase, Order, OrderIntent, MAX_QUEUED_ORDERS};
use crate::game::services::move_coordinator::MoveCoordinator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PointPromotionKey {
    owner: u32,
    attack_move: bool,
    x_bits: u32,
    y_bits: u32,
}

impl PointPromotionKey {
    fn new(owner: u32, attack_move: bool, x: f32, y: f32) -> Option<Self> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some(PointPromotionKey {
            owner,
            attack_move,
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
        })
    }

    fn point(self) -> (f32, f32) {
        (f32::from_bits(self.x_bits), f32::from_bits(self.y_bits))
    }
}

/// Promote completed movement orders into the next queued point intent.
///
/// Later phases will add validation for attack, gather, and build intents. For phase 1 those
/// unsupported intents are skipped at promotion time so a hostile or future client cannot wedge
/// a unit behind an inert queue entry.
pub(crate) fn promote_ready_orders(
    entities: &mut EntityStore,
    coordinator: &mut MoveCoordinator<'_>,
) {
    let ready: Vec<u32> = entities
        .iter()
        .filter(|e| ready_for_next_order(e))
        .map(|e| e.id)
        .collect();
    if ready.is_empty() {
        return;
    }

    let mut groups: BTreeMap<PointPromotionKey, Vec<u32>> = BTreeMap::new();
    for id in ready {
        let Some((owner, attack_move, x, y)) = pop_next_point_intent(entities, id) else {
            continue;
        };
        let Some(key) = PointPromotionKey::new(owner, attack_move, x, y) else {
            continue;
        };
        groups.entry(key).or_default().push(id);
    }

    for (key, ids) in groups {
        coordinator.order_group_move(entities, key.owner, &ids, key.point(), key.attack_move);
    }
}

fn ready_for_next_order(e: &Entity) -> bool {
    if !e.is_unit() || e.queued_orders().is_empty() || !e.path_is_empty() {
        return false;
    }
    match e.order() {
        Order::Idle => true,
        Order::Move(_) | Order::AttackMove(_) => matches!(
            e.move_phase(),
            Some(MovePhase::Arrived | MovePhase::PathFailed)
        ),
        Order::Attack(_) | Order::Gather(_) | Order::Build(_) => false,
    }
}

fn pop_next_point_intent(entities: &mut EntityStore, id: u32) -> Option<(u32, bool, f32, f32)> {
    let owner = entities.get(id)?.owner;
    for _ in 0..MAX_QUEUED_ORDERS {
        let intent = entities.get_mut(id)?.pop_queued_order()?;
        match intent {
            OrderIntent::Move(point) if point.x.is_finite() && point.y.is_finite() => {
                return Some((owner, false, point.x, point.y));
            }
            OrderIntent::AttackMove(point) if point.x.is_finite() && point.y.is_finite() => {
                return Some((owner, true, point.x, point.y));
            }
            OrderIntent::Move(_) | OrderIntent::AttackMove(_) => continue,
            OrderIntent::Attack(_) | OrderIntent::Gather(_) | OrderIntent::Build(_) => continue,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order, OrderIntent};
    use crate::game::map::Map;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::occupancy::Occupancy;
    use crate::game::services::pathing::PathingService;
    use crate::protocol::terrain;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4)],
            expansion_sites: Vec::new(),
        }
    }

    fn promote(map: &Map, entities: &mut EntityStore) {
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        promote_ready_orders(entities, &mut coordinator);
    }

    #[test]
    fn idle_unit_promotes_first_queued_move() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        entities
            .get_mut(unit)
            .expect("unit should exist")
            .append_queued_order(OrderIntent::move_to(180.0, 100.0));

        promote(&map, &mut entities);

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::Move(_)));
        assert_eq!(entity.move_phase(), Some(MovePhase::AwaitingPath));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn attack_move_engagement_without_arrival_does_not_promote() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let entity = entities.get_mut(unit).expect("unit should exist");
            entity.set_order(Order::attack_move_to(300.0, 100.0));
            entity.mark_move_phase(MovePhase::Moving);
            entity.append_queued_order(OrderIntent::move_to(360.0, 100.0));
        }

        promote(&map, &mut entities);

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::AttackMove(_)));
        assert_eq!(entity.queued_orders().len(), 1);
    }

    #[test]
    fn arrived_attack_move_promotes_after_reaching_destination() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
            .expect("rifleman should spawn");
        {
            let entity = entities.get_mut(unit).expect("unit should exist");
            entity.set_order(Order::attack_move_to(120.0, 100.0));
            entity.mark_move_phase(MovePhase::Arrived);
            entity.append_queued_order(OrderIntent::attack_move_to(180.0, 100.0));
        }

        promote(&map, &mut entities);

        let entity = entities.get(unit).expect("unit should exist");
        assert!(matches!(entity.order(), Order::AttackMove(_)));
        assert_eq!(entity.move_phase(), Some(MovePhase::AwaitingPath));
        assert!(entity.queued_orders().is_empty());
    }
}
