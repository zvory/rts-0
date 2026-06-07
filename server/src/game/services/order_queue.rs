use std::collections::BTreeMap;

use crate::config;
use crate::game::entity::{
    BuildPhase, Entity, EntityKind, EntityStore, MovePhase, Order, OrderIntent, MAX_QUEUED_ORDERS,
};
use crate::game::map::Map;
use crate::game::services::construction::resumable_site_for_build_intent;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::standability;
use crate::game::services::world_query;
use crate::game::PlayerState;
use crate::rules;

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

/// Outcome of popping the next queued intent for a unit. Move/AttackMove are batched into a
/// group move per destination point; gather/build are issued directly per worker.
enum PromotedIntent {
    PointMove(PointPromotionKey),
    Gather { node: u32 },
    Build { kind: EntityKind, tx: u32, ty: u32 },
}

/// Promote completed orders into the next queued intent.
///
/// Move/AttackMove intents are batched by destination so co-arriving units share a formation,
/// while Gather and Build intents are issued one worker at a time. Queued Attack intents are
/// still skipped silently in Phase 3 (Phase 4 will validate and promote them).
pub(crate) fn promote_ready_orders(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
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
        let Some(promoted) = pop_next_valid_intent(map, entities, players, id) else {
            continue;
        };
        match promoted {
            PromotedIntent::PointMove(key) => {
                groups.entry(key).or_default().push(id);
            }
            PromotedIntent::Gather { node } => {
                coordinator.order_gather(entities, id, node);
            }
            PromotedIntent::Build { kind, tx, ty } => {
                coordinator.order_build(entities, id, kind, tx, ty);
            }
        }
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

fn pop_next_valid_intent(
    map: &Map,
    entities: &mut EntityStore,
    players: &[PlayerState],
    id: u32,
) -> Option<PromotedIntent> {
    let owner = entities.get(id)?.owner;
    for _ in 0..MAX_QUEUED_ORDERS {
        let intent = entities.get_mut(id)?.pop_queued_order()?;
        match intent {
            OrderIntent::Move(point) => {
                if let Some(key) = PointPromotionKey::new(owner, false, point.x, point.y) {
                    return Some(PromotedIntent::PointMove(key));
                }
            }
            OrderIntent::AttackMove(point) => {
                if let Some(key) = PointPromotionKey::new(owner, true, point.x, point.y) {
                    return Some(PromotedIntent::PointMove(key));
                }
            }
            OrderIntent::Gather(gather) => {
                if gather_intent_valid(entities, owner, id, gather.node) {
                    return Some(PromotedIntent::Gather { node: gather.node });
                }
            }
            OrderIntent::Build(build) => {
                if build_intent_valid(
                    map,
                    entities,
                    players,
                    owner,
                    id,
                    build.kind,
                    build.tile_x,
                    build.tile_y,
                ) {
                    return Some(PromotedIntent::Build {
                        kind: build.kind,
                        tx: build.tile_x,
                        ty: build.tile_y,
                    });
                }
            }
            // Attack queueing arrives in Phase 4; silently drop until then.
            OrderIntent::Attack(_) => continue,
        }
    }
    None
}

fn gather_intent_valid(entities: &EntityStore, owner: u32, worker: u32, node: u32) -> bool {
    let is_worker = matches!(entities.get(worker), Some(e) if e.kind == EntityKind::Worker);
    if !is_worker {
        return false;
    }
    let node_ok = matches!(entities.get(node), Some(n)
        if n.is_node() && n.remaining().unwrap_or(0) > 0);
    if !node_ok {
        return false;
    }
    if !world_query::resource_has_completed_mining_cc(entities, owner, node) {
        return false;
    }
    if matches!(entities.node_slot_holder(node), Some(holder) if holder != worker) {
        return false;
    }
    true
}

fn build_intent_valid(
    map: &Map,
    entities: &EntityStore,
    players: &[PlayerState],
    owner: u32,
    worker: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    if !matches!(entities.get(worker), Some(e) if e.kind == EntityKind::Worker) {
        return false;
    }
    if matches!(entities.get(worker), Some(e)
        if matches!(e.build_phase(), Some(BuildPhase::Constructing { .. })))
    {
        return false;
    }
    if config::building_stats(kind).is_none() {
        return false;
    }
    let owned = world_query::completed_building_kinds(entities, owner);
    if !rules::economy::build_requirement_met(kind, &owned) {
        return false;
    }
    if tile_x >= map.size || tile_y >= map.size {
        return false;
    }
    let can_resume =
        resumable_site_for_build_intent(map, entities, owner, kind, tile_x, tile_y).is_some();
    if !can_resume
        && !standability::building_site_clear_for_build_intent(
            map, entities, kind, tile_x, tile_y, worker,
        )
    {
        return false;
    }
    let ps = match players.iter().find(|p| p.id == owner) {
        Some(p) => p,
        None => return false,
    };
    let (cost_steel, cost_oil) = rules::economy::cost(kind);
    if ps.steel < cost_steel || ps.oil < cost_oil {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore, MovePhase, Order, OrderIntent};
    use crate::game::map::Map;
    use crate::game::services::move_coordinator::MoveCoordinator;
    use crate::game::services::occupancy::{footprint_center, Occupancy};
    use crate::game::services::pathing::PathingService;
    use crate::game::ScoreState;
    use crate::protocol::terrain;

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(4, 4)],
            expansion_sites: Vec::new(),
        }
    }

    fn player_state(id: u32) -> PlayerState {
        PlayerState {
            id,
            name: format!("Player {id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 1_000,
            oil: 1_000,
            supply_used: 0,
            supply_cap: 20,
            is_ai: false,
            score: ScoreState::default(),
        }
    }

    fn promote(map: &Map, entities: &mut EntityStore) {
        let players = vec![player_state(1)];
        promote_with_players(map, entities, &players);
    }

    fn promote_with_players(map: &Map, entities: &mut EntityStore, players: &[PlayerState]) {
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(1024, 32);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        promote_ready_orders(map, entities, players, &mut coordinator);
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

    #[test]
    fn idle_worker_promotes_queued_gather_on_valid_node() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
            .expect("city centre should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cx + 64.0, cy)
            .expect("steel node should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cx, cy + 16.0)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .append_queued_order(OrderIntent::gather(node));

        promote(&map, &mut entities);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(matches!(entity.order(), Order::Gather(_)));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_gather_on_depleted_node_is_skipped_silently() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
            .expect("city centre should spawn");
        let node = entities
            .spawn_node(EntityKind::Steel, cx + 64.0, cy)
            .expect("node should spawn");
        // Deplete the node manually so it survives in-store but has nothing to mine.
        if let Some(n) = entities.get_mut(node) {
            if let Some(resource) = n.resource_node.as_mut() {
                resource.remaining = 0;
            }
        }
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cx, cy + 16.0)
            .expect("worker should spawn");
        let fallback = (cx + 96.0, cy);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.append_queued_order(OrderIntent::gather(node));
            w.append_queued_order(OrderIntent::move_to(fallback.0, fallback.1));
        }

        promote(&map, &mut entities);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "depleted gather should be skipped and the next move intent should promote"
        );
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn idle_worker_promotes_queued_build_on_clear_site() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("worker should spawn");
        entities
            .get_mut(worker)
            .expect("worker should exist")
            .append_queued_order(OrderIntent::build(EntityKind::Depot, 16, 16));

        let players = vec![player_state(1)];
        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(matches!(entity.order(), Order::Build(_)));
        assert!(entity.queued_orders().is_empty());
    }

    #[test]
    fn queued_build_skips_when_player_cannot_afford() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 4, 4);
        entities
            .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
            .expect("city centre should spawn");
        let worker = entities
            .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y)
            .expect("worker should spawn");
        let fallback = (cc_x + 160.0, cc_y);
        {
            let w = entities.get_mut(worker).expect("worker should exist");
            w.append_queued_order(OrderIntent::build(EntityKind::Depot, 16, 16));
            w.append_queued_order(OrderIntent::move_to(fallback.0, fallback.1));
        }
        let mut players = vec![player_state(1)];
        players[0].steel = 0;
        players[0].oil = 0;

        promote_with_players(&map, &mut entities, &players);

        let entity = entities.get(worker).expect("worker should exist");
        assert!(
            matches!(entity.order(), Order::Move(_)),
            "unaffordable build should be skipped and the next move intent should promote"
        );
        assert!(entity.queued_orders().is_empty());
    }
}
