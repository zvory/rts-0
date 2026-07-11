use crate::game::entity::{EntityStore, OrderIntent, RallyIntent, RallyKind};
use crate::game::map::Map;
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::game::{ability::AbilityKind, entity::EntityKind};

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
        let completed_research = {
            match entities.get_mut(id) {
                Some(b)
                    if b.hp > 0
                        && b.is_building()
                        && !b.under_construction()
                        && !b.research_queue().is_empty() =>
                {
                    let owner = b.owner;
                    if let Some(queue) = b.research_queue_mut() {
                        let front = &mut queue[0];
                        if front.progress < front.total {
                            front.progress = front.progress.saturating_add(1);
                        }
                        if front.progress >= front.total {
                            Some((owner, front.upgrade))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };

        if let Some((owner, upgrade)) = completed_research {
            if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
                player.upgrades.insert(upgrade);
            }
            if upgrade == UpgradeKind::MortarAutocast {
                set_owned_mortar_autocast(entities, owner, true);
            }
            if let Some(b) = entities.get_mut(id) {
                if let Some(queue) = b.research_queue_mut() {
                    if !queue.is_empty() {
                        queue.remove(0);
                    }
                }
            }
            continue;
        }

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
            b.tick_front_production().map(|unit| (owner, unit))
        };

        if let Some((owner, unit)) = ready {
            // Prefer the spawn exit closest to the first rally stage (if any), so units leave from
            // the side of the building facing the rally plan.
            let rally_plan = entities.get(id).map(|b| b.rally_plan()).unwrap_or_default();
            let first_rally = rally_plan.first().map(|r| (r.point.x, r.point.y));
            let Some((sx, sy)) = coordinator.find_spawn_point(entities, id, unit, first_rally)
            else {
                continue;
            };
            let spawn_facing = first_rally
                .and_then(|rally| coordinator.rally_spawn_facing(entities, unit, (sx, sy), rally));
            if let Some(spawned) = entities.spawn_unit(owner, unit, sx, sy) {
                let mortar_autocast_researched = players
                    .iter()
                    .any(|p| p.id == owner && p.upgrades.contains(&UpgradeKind::MortarAutocast));
                if unit == EntityKind::MortarTeam && mortar_autocast_researched {
                    if let Some(e) = entities.get_mut(spawned) {
                        e.set_autocast_enabled(AbilityKind::MortarFire, true);
                    }
                }
                if let Some(facing) = spawn_facing {
                    if let Some(e) = entities.get_mut(spawned) {
                        e.set_facing(facing);
                    }
                }
                if let Some(b) = entities.get_mut(id) {
                    b.remove_front_production();
                }
                if let Some(player) = players.iter_mut().find(|p| p.id == owner) {
                    player.record_entity_created(unit);
                }
                // Send the new unit through the building's rally plan. Plain rally stages default
                // to attack-move for combat units, but worker-like gatherers keep move rally behavior.
                if let Some(first) = rally_plan.first().copied() {
                    coordinator.order_group_move(
                        entities,
                        owner,
                        &[spawned],
                        (first.point.x, first.point.y),
                        rally_stage_attacks(unit, first),
                    );
                    if let Some(e) = entities.get_mut(spawned) {
                        for stage in rally_plan.iter().skip(1).copied() {
                            e.append_queued_order(rally_order_intent(unit, stage));
                        }
                    }
                }
            }
        }
    }
}

fn rally_stage_attacks(unit: EntityKind, rally: RallyIntent) -> bool {
    matches!(rally.kind, RallyKind::AttackMove) || !is_worker_like_gatherer(unit)
}

fn is_worker_like_gatherer(unit: EntityKind) -> bool {
    matches!(unit, EntityKind::Worker | EntityKind::Golem)
}

fn rally_order_intent(unit: EntityKind, rally: RallyIntent) -> OrderIntent {
    if rally_stage_attacks(unit, rally) {
        OrderIntent::attack_move_to(rally.point.x, rally.point.y)
    } else {
        OrderIntent::move_to(rally.point.x, rally.point.y)
    }
}

fn set_owned_mortar_autocast(entities: &mut EntityStore, owner: u32, enabled: bool) {
    for id in entities.ids() {
        if let Some(e) = entities.get_mut(id) {
            if e.owner == owner && e.kind == EntityKind::MortarTeam {
                e.set_autocast_enabled(AbilityKind::MortarFire, enabled);
            }
        }
    }
}

pub(crate) fn sync_owned_autocast_from_upgrades(
    entities: &mut EntityStore,
    owner: u32,
    upgrades: &std::collections::BTreeSet<UpgradeKind>,
) {
    set_owned_mortar_autocast(
        entities,
        owner,
        upgrades.contains(&UpgradeKind::MortarAutocast),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, Order, ProdItem, RallyIntent, RallyKind, ResearchItem};
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
    fn mortar_autocast_research_enables_existing_mortars() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let mortar = entities
            .spawn_unit(1, EntityKind::MortarTeam, 160.0, 160.0)
            .expect("mortar should spawn");
        let (x, y) = footprint_center(&map, EntityKind::ResearchComplex, 10, 10);
        let research_complex = entities
            .spawn_building(1, EntityKind::ResearchComplex, x, y, true)
            .expect("research complex should spawn");
        entities
            .get_mut(research_complex)
            .expect("research complex")
            .push_research(ResearchItem {
                upgrade: UpgradeKind::MortarAutocast,
                progress: 1,
                total: 1,
            });
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        assert!(players[0].upgrades.contains(&UpgradeKind::MortarAutocast));
        assert_eq!(
            entities
                .get(mortar)
                .expect("mortar should exist")
                .autocast_enabled(AbilityKind::MortarFire),
            Some(true)
        );
    }

    #[test]
    fn produced_mortars_start_with_autocast_after_research() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        spawn_building_training(
            &map,
            &mut entities,
            10,
            10,
            EntityKind::Steelworks,
            EntityKind::MortarTeam,
        );
        let mut player = player(1);
        player.upgrades.insert(UpgradeKind::MortarAutocast);
        let mut players = vec![player];

        tick_production(&map, &mut entities, &mut players);

        let mortar = entities
            .iter()
            .find(|e| e.owner == 1 && e.kind == EntityKind::MortarTeam)
            .expect("produced mortar should exist");
        assert_eq!(mortar.autocast_enabled(AbilityKind::MortarFire), Some(true));
    }

    #[test]
    fn panzerfaust_production_completes_from_barracks_queue() {
        let map = flat_map(24);
        let mut entities = EntityStore::new();
        let barracks = spawn_building_training(
            &map,
            &mut entities,
            10,
            10,
            EntityKind::Barracks,
            EntityKind::Panzerfaust,
        );
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        assert!(entities
            .get(barracks)
            .expect("barracks")
            .prod_queue()
            .is_empty());
        let panzerfaust = entities
            .iter()
            .find(|e| e.owner == 1 && e.kind == EntityKind::Panzerfaust && e.hp > 0)
            .expect("Panzerfaust should spawn from completed Barracks queue");
        assert!(
            matches!(panzerfaust.order(), Order::Idle),
            "without a rally point the produced Panzerfaust should stay idle"
        );
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
    fn move_rally_attack_moves_spawned_non_worker_and_prefers_near_exit() {
        let map = flat_map(40);
        let mut entities = EntityStore::new();
        let factory = spawn_factory(&map, &mut entities, 10, 10);
        let factory_x = entities.get(factory).expect("factory").pos_x;
        // Rally far to the +x side of the factory.
        let rally = map.tile_center(30, 11);
        entities
            .get_mut(factory)
            .expect("factory")
            .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
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
        assert_angle_close(
            spawned.facing(),
            (rally.1 - spawned.pos_y).atan2(rally.0 - spawned.pos_x),
        );
        assert!(
            matches!(spawned.order(), Order::AttackMove(_)),
            "spawned non-worker should default to attack-moving to the rally point"
        );
        let goal = spawned.path_goal().expect("rally move should set a goal");
        let dist = ((goal.0 - rally.0).powi(2) + (goal.1 - rally.1).powi(2)).sqrt();
        assert!(
            dist <= crate::config::TILE_SIZE as f32 * 4.0,
            "rally move goal should be near the rally point, was {dist} px away"
        );
    }

    #[test]
    fn queued_rally_plan_defaults_non_worker_move_stages_to_attack_move() {
        let map = flat_map(40);
        let mut entities = EntityStore::new();
        let factory = spawn_factory(&map, &mut entities, 10, 10);
        let first = map.tile_center(30, 11);
        let second = map.tile_center(30, 18);
        {
            let building = entities.get_mut(factory).expect("factory");
            building.set_rally_point(Some(RallyIntent::new(
                RallyKind::AttackMove,
                first.0,
                first.1,
            )));
            assert!(building
                .append_rally_stage(RallyIntent::new(RallyKind::Move, second.0, second.1), 4,));
        }
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        let tank = tanks_owned_by(&entities, 1)
            .into_iter()
            .next()
            .expect("a tank should spawn");
        let spawned = entities.get(tank.0).expect("spawned tank");
        assert!(
            matches!(spawned.order(), Order::AttackMove(_)),
            "first attack-move rally stage should become the active spawn order"
        );
        assert_eq!(
            spawned.queued_orders(),
            &[crate::game::entity::OrderIntent::attack_move_to(
                second.0, second.1
            )],
            "later move rally stages should default to queued attack-move for non-workers"
        );
    }

    #[test]
    fn move_rally_keeps_spawned_worker_on_move_order() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let city_centre = spawn_building_training(
            &map,
            &mut entities,
            10,
            10,
            EntityKind::CityCentre,
            EntityKind::Worker,
        );
        let rally = map.tile_center(18, 10);
        entities
            .get_mut(city_centre)
            .expect("city centre")
            .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        let worker = entities
            .iter()
            .find(|e| e.owner == 1 && e.kind == EntityKind::Worker && e.hp > 0)
            .expect("worker should spawn");
        assert!(
            matches!(worker.order(), Order::Move(_)),
            "spawned workers should keep plain move rallies instead of attack-moving"
        );
    }

    #[test]
    fn same_tile_machine_gunner_default_attack_move_rally_survives_path_request() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let barracks = spawn_building_training(
            &map,
            &mut entities,
            10,
            10,
            EntityKind::Barracks,
            EntityKind::MachineGunner,
        );
        let rally = current_spawn_point_for(&map, &entities, barracks, EntityKind::MachineGunner)
            .expect("barracks should have an MG spawn exit");
        entities
            .get_mut(barracks)
            .expect("barracks")
            .set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));
        let mut players = vec![player(1)];
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
        let mut events = HashMap::new();

        production_system(
            &map,
            &mut entities,
            &mut players,
            &mut coordinator,
            &mut events,
        );
        coordinator.process_awaiting_paths(&mut entities);

        let machine_gunner = entities
            .iter()
            .find(|e| e.owner == 1 && e.kind == EntityKind::MachineGunner && e.hp > 0)
            .expect("machine gunner should spawn");
        assert!(matches!(machine_gunner.order(), Order::AttackMove(_)));
        assert_eq!(
            machine_gunner.move_phase(),
            Some(crate::game::entity::MovePhase::Arrived)
        );
        assert!(machine_gunner.path_is_empty());
    }

    #[test]
    fn rally_spawn_facing_falls_back_when_oriented_body_would_clip() {
        let map = flat_map(16);
        let mut entities = EntityStore::new();
        spawn_factory(&map, &mut entities, 4, 4);
        let spawn = map.tile_center(5, 3);
        let rally = map.tile_center(5, 10);
        let desired_facing = (rally.1 - spawn.1).atan2(rally.0 - spawn.0);
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        assert!(
            standability::unit_static_standable(&map, &occ, EntityKind::Tank, spawn.0, spawn.1),
            "test setup needs a legal default-facing tank spawn"
        );
        assert!(
            !standability::unit_static_standable_with_facing(
                &map,
                &occ,
                EntityKind::Tank,
                spawn.0,
                spawn.1,
                desired_facing,
            ),
            "test setup needs the rally-facing hull to clip the factory"
        );
        assert_eq!(
            coordinator.rally_spawn_facing(&entities, EntityKind::Tank, spawn, rally),
            None,
            "rally-facing preference should be skipped when the rotated hull is illegal"
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

    #[test]
    fn no_rally_anti_tank_gun_spawn_uses_rotation_safe_circular_body() {
        let map = flat_map(32);
        let mut entities = EntityStore::new();
        let gun_works = spawn_building_training(
            &map,
            &mut entities,
            10,
            10,
            EntityKind::Steelworks,
            EntityKind::AntiTankGun,
        );
        let mut players = vec![player(1)];

        tick_production(&map, &mut entities, &mut players);

        assert!(
            entities
                .get(gun_works)
                .expect("gun works")
                .prod_queue()
                .is_empty(),
            "anti-tank gun should spawn when a no-rally Gun Works exit is available"
        );
        let anti_tank_gun = entities
            .iter()
            .find(|e| e.owner == 1 && e.kind == EntityKind::AntiTankGun && e.hp > 0)
            .expect("anti-tank gun should spawn");
        assert!(
            matches!(anti_tank_gun.order(), Order::Idle),
            "without a rally point the anti-tank gun should stay idle at its spawn exit"
        );
        let occ = Occupancy::build(&map, &entities_without(&entities, anti_tank_gun.id));
        for facing in [
            0.0,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            std::f32::consts::PI * 1.5,
        ] {
            assert!(
                standability::unit_static_standable_with_facing(
                    &map,
                    &occ,
                    EntityKind::AntiTankGun,
                    anti_tank_gun.pos_x,
                    anti_tank_gun.pos_y,
                    facing,
                ),
                "anti-tank gun spawn should remain legal at facing {facing:.3}"
            );
        }
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            base_sites: vec![],
        }
    }

    fn player(id: u32) -> PlayerState {
        PlayerState {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("p{id}"),
            color: "#fff".to_string(),
            start_tile: (0, 0),
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            is_ai: false,
            score: ScoreState::default(),
            upgrades: Default::default(),
            ability_cooldowns: Default::default(),
        }
    }

    fn spawn_factory(map: &Map, entities: &mut EntityStore, tile_x: u32, tile_y: u32) -> u32 {
        let (x, y) = footprint_center(map, EntityKind::Factory, tile_x, tile_y);
        let id = entities
            .spawn_building(1, EntityKind::Factory, x, y, true)
            .expect("factory should spawn");
        entities
            .get_mut(id)
            .expect("factory")
            .push_production(ProdItem {
                unit: EntityKind::Tank,
                progress: 1,
                total: 1,
            });
        id
    }

    fn spawn_building_training(
        map: &Map,
        entities: &mut EntityStore,
        tile_x: u32,
        tile_y: u32,
        building: EntityKind,
        unit: EntityKind,
    ) -> u32 {
        let (x, y) = footprint_center(map, building, tile_x, tile_y);
        let id = entities
            .spawn_building(1, building, x, y, true)
            .expect("producer should spawn");
        entities
            .get_mut(id)
            .expect("producer")
            .push_production(ProdItem {
                unit,
                progress: 1,
                total: 1,
            });
        id
    }

    fn assert_angle_close(actual: f32, expected: f32) {
        let delta = (actual - expected + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        assert!(
            delta.abs() <= 0.001,
            "expected angle {actual:.4} to be close to {expected:.4}"
        );
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

    fn current_spawn_point_for(
        map: &Map,
        entities: &EntityStore,
        producer: u32,
        unit: EntityKind,
    ) -> Option<(f32, f32)> {
        let occ = Occupancy::build(map, entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let coordinator = MoveCoordinator::new(&mut pathing, map, &occ, 1);
        coordinator.find_spawn_point(entities, producer, unit, None)
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
