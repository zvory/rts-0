use crate::game::entity::EntityStore;
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::PlayerState;

/// Advance each building's front production item; on completion spawn the unit adjacent to the
/// building and remove the item from the queue. If every spawn point is blocked, keep the complete
/// item queued and retry next tick. Supply was already reserved on enqueue, so spawning does not
/// re-charge it. Cost was charged at enqueue too.
pub(crate) fn production_system(
    _map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    coordinator: &mut MoveCoordinator<'_>,
    _events: &mut std::collections::HashMap<u32, Vec<crate::protocol::Event>>,
) {
    for id in entities.ids() {
        let ready = {
            let b = match entities.get_mut(id) {
                Some(b)
                    if b.hp > 0
                        && b.is_building()
                        && !b.under_construction()
                        && !b.prod_queue().is_empty() =>
                {
                    b
                }
                _ => continue,
            };
            let owner = b.owner;
            let Some(queue) = b.prod_queue_mut() else {
                continue;
            };
            let front = &mut queue[0];
            if front.progress < front.total {
                front.progress = front.progress.saturating_add(1);
            }
            if front.progress >= front.total {
                Some((owner, front.unit))
            } else {
                None
            }
        };

        if let Some((owner, unit)) = ready {
            // Prefer the spawn exit closest to the rally point (if any), so units leave from the
            // side of the building facing the rally.
            let rally = entities.get(id).and_then(|b| b.rally_point());
            let Some((sx, sy)) = coordinator.find_spawn_point(entities, id, unit, rally) else {
                continue;
            };
            if let Some(spawned) = entities.spawn_unit(owner, unit, sx, sy) {
                if let Some(b) = entities.get_mut(id) {
                    if let Some(queue) = b.prod_queue_mut() {
                        if !queue.is_empty() {
                            queue.remove(0);
                        }
                    }
                }
                if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
                    player.record_entity_created(unit);
                }
                // Send the new unit toward the rally point with a plain move order.
                if let Some(rally) = rally {
                    coordinator.order_group_move(entities, owner, &[spawned], rally, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, Order, ProdItem};
    use crate::game::map::Map;
    use crate::game::services::occupancy::{footprint_center, Occupancy};
    use crate::game::services::pathing::PathingService;
    use crate::game::services::standability;
    use crate::game::ScoreState;
    use crate::protocol::terrain;
    use std::collections::HashMap;

    #[test]
    fn tank_spawn_search_avoids_occupied_exit() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let factory = spawn_factory(&map, &mut entities, 10, 10);
        let first_exit = current_spawn_point(&map, &entities, factory).expect("initial exit");
        entities
            .spawn_unit(1, EntityKind::Tank, first_exit.0, first_exit.1)
            .expect("blocker should spawn");
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        let spawned = tanks_owned_by(&entities, 1);
        assert_eq!(spawned.len(), 2, "blocker plus produced tank should exist");
        let produced = spawned
            .into_iter()
            .find(|(_, x, y)| (*x, *y) != first_exit)
            .expect("produced tank should use a different exit");
        assert_ne!((produced.1, produced.2), first_exit);
        let occ = Occupancy::build(&map, &entities);
        assert!(standability::unit_spawn_standable(
            &map,
            &occ,
            &entities_without(&entities, produced.0),
            EntityKind::Tank,
            produced.1,
            produced.2,
        ));
    }

    #[test]
    fn tank_production_waits_when_all_exits_blocked() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let factory = spawn_factory(&map, &mut entities, 10, 10);
        block_all_spawn_points(&map, &mut entities, factory);
        let before = tank_count(&entities);
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        assert_eq!(tank_count(&entities), before);
        let queue = entities.get(factory).expect("factory").prod_queue();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].progress, queue[0].total);
    }

    #[test]
    fn blocked_tank_production_spawns_after_exit_clears() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let factory = spawn_factory(&map, &mut entities, 10, 10);
        let blockers = block_all_spawn_points(&map, &mut entities, factory);
        let before = tank_count(&entities);
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);
        assert_eq!(tank_count(&entities), before);

        entities.remove(blockers[0]);
        tick_production(&map, &mut entities, &mut players);

        assert_eq!(tank_count(&entities), before);
        assert!(entities
            .get(factory)
            .expect("factory")
            .prod_queue()
            .is_empty());
    }

    #[test]
    fn multiple_factories_do_not_spawn_units_on_same_point_in_one_tick() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        spawn_factory(&map, &mut entities, 10, 10);
        spawn_factory(&map, &mut entities, 14, 10);
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        let tanks = tanks_owned_by(&entities, 1);
        assert_eq!(tanks.len(), 2);
        assert_ne!((tanks[0].1, tanks[0].2), (tanks[1].1, tanks[1].2));
    }

    #[test]
    fn spawn_search_rejects_body_clipping_adjacent_building() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        spawn_factory(&map, &mut entities, 10, 10);
        let (dx, dy) = footprint_center(&map, EntityKind::Depot, 14, 10);
        entities
            .spawn_building(1, EntityKind::Depot, dx, dy, true)
            .expect("depot should spawn");
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        let produced = tanks_owned_by(&entities, 1)
            .into_iter()
            .next()
            .expect("tank should spawn away from depot");
        let occ = Occupancy::build(&map, &entities);
        assert!(standability::unit_spawn_standable(
            &map,
            &occ,
            &entities_without(&entities, produced.0),
            EntityKind::Tank,
            produced.1,
            produced.2,
        ));
    }

    #[test]
    fn rally_point_moves_spawned_unit_and_prefers_near_exit() {
        let map = flat_map(40);
        let mut entities = EntityStore::new();
        let factory = spawn_factory(&map, &mut entities, 10, 10);
        let factory_x = entities.get(factory).expect("factory").pos_x;
        // Rally far to the +x side of the factory.
        let rally = map.tile_center(30, 11);
        entities
            .get_mut(factory)
            .expect("factory")
            .set_rally_point(Some(rally));
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        let tank = tanks_owned_by(&entities, 1)
            .into_iter()
            .next()
            .expect("a tank should spawn");
        assert!(
            tank.1 > factory_x,
            "tank should exit toward the rally (+x side), got x={} vs factory x={}",
            tank.1,
            factory_x
        );
        let spawned = entities.get(tank.0).expect("spawned tank");
        assert!(
            matches!(spawned.order(), Order::Move(_)),
            "spawned unit should receive a move order to the rally point"
        );
        let goal = spawned.path_goal().expect("rally move should set a goal");
        let dist = ((goal.0 - rally.0).powi(2) + (goal.1 - rally.1).powi(2)).sqrt();
        assert!(
            dist <= crate::config::TILE_SIZE as f32 * 4.0,
            "rally move goal should be near the rally point, was {dist} px away"
        );
    }

    #[test]
    fn no_rally_spawns_idle_unit() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        spawn_factory(&map, &mut entities, 10, 10);
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        let tank = tanks_owned_by(&entities, 1)
            .into_iter()
            .next()
            .expect("a tank should spawn");
        assert!(
            matches!(entities.get(tank.0).expect("tank").order(), Order::Idle),
            "without a rally point a freshly produced unit should stay idle"
        );
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }

    fn player(id: u32) -> PlayerState {
        PlayerState {
            id,
            name: format!("p{id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            is_ai: false,
            score: ScoreState::default(),
        }
    }

    fn spawn_factory(map: &Map, entities: &mut EntityStore, tile_x: u32, tile_y: u32) -> u32 {
        let (x, y) = footprint_center(map, EntityKind::TankFactory, tile_x, tile_y);
        let id = entities
            .spawn_building(1, EntityKind::TankFactory, x, y, true)
            .expect("factory should spawn");
        entities
            .get_mut(id)
            .expect("factory")
            .prod_queue_mut()
            .expect("queue")
            .push(ProdItem {
                unit: EntityKind::Tank,
                progress: 1,
                total: 1,
            });
        id
    }

    fn tick_production(map: &Map, entities: &mut EntityStore, players: &mut [PlayerState]) {
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        let mut events = HashMap::new();
        production_system(map, entities, players, &mut coordinator, &mut events);
    }

    fn current_spawn_point(map: &Map, entities: &EntityStore, factory: u32) -> Option<(f32, f32)> {
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        coordinator.find_spawn_point(entities, factory, EntityKind::Tank, None)
    }

    fn block_all_spawn_points(map: &Map, entities: &mut EntityStore, factory: u32) -> Vec<u32> {
        let mut blockers = Vec::new();
        while let Some((x, y)) = current_spawn_point(map, entities, factory) {
            let id = entities
                .spawn_unit(2, EntityKind::Tank, x, y)
                .expect("blocker should spawn");
            blockers.push(id);
        }
        assert!(!blockers.is_empty(), "test should block at least one exit");
        blockers
    }

    fn tank_count(entities: &EntityStore) -> usize {
        entities
            .iter()
            .filter(|e| e.kind == EntityKind::Tank && e.hp > 0)
            .count()
    }

    fn tanks_owned_by(entities: &EntityStore, owner: u32) -> Vec<(u32, f32, f32)> {
        entities
            .iter()
            .filter(|e| e.owner == owner && e.kind == EntityKind::Tank && e.hp > 0)
            .map(|e| (e.id, e.pos_x, e.pos_y))
            .collect()
    }

    fn entities_without(entities: &EntityStore, removed: u32) -> EntityStore {
        let mut clone = EntityStore::new();
        for e in entities.iter() {
            if e.id != removed {
                clone.insert(e.clone());
            }
        }
        clone
    }
}
