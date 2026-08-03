use super::*;

#[test]
fn queued_build_skips_occupied_scaffold_and_promotes_next_order() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let (resource_depot_x, resource_depot_y) =
        footprint_center(&map, EntityKind::ResourceDepot, 4, 4);
    entities
        .spawn_building(
            1,
            EntityKind::ResourceDepot,
            resource_depot_x,
            resource_depot_y,
            true,
        )
        .expect("resource depot should spawn");
    let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 16, 16);
    let site = entities
        .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
        .expect("scaffold should spawn");
    let builder = entities
        .spawn_unit(1, EntityKind::Worker, site_x, site_y)
        .expect("builder should spawn");
    {
        let builder = entities.get_mut(builder).expect("builder should exist");
        builder.set_order(Order::build(EntityKind::Depot, 16, 16));
        builder.mark_build_phase(BuildPhase::Constructing { site });
        builder.set_target_id(Some(site));
    }
    let queued_worker = entities
        .spawn_unit(
            1,
            EntityKind::Worker,
            resource_depot_x + 96.0,
            resource_depot_y,
        )
        .expect("queued worker should spawn");
    let fallback = (resource_depot_x + 160.0, resource_depot_y);
    {
        let worker = entities
            .get_mut(queued_worker)
            .expect("queued worker should exist");
        worker.append_queued_order(OrderIntent::build(EntityKind::Depot, 16, 16));
        worker.append_queued_order(OrderIntent::move_to(fallback.0, fallback.1));
    }
    let players = vec![player_state(1)];

    let events = promote_with_players_events(&map, &mut entities, &players);

    let entity = entities
        .get(queued_worker)
        .expect("queued worker should exist");
    assert!(
        matches!(entity.order(), Order::Move(_)),
        "the invalid occupied-scaffold build should fall through to the queued move"
    );
    assert!(entity.queued_orders().is_empty());
    assert!(
        events.get(&1).is_some_and(|events| events.iter().any(
            |event| matches!(event, Event::Notice { msg, .. } if msg == "Cannot build there")
        )),
        "the skipped build should report the occupied site"
    );
    assert_eq!(
        entities
            .get(builder)
            .expect("builder should remain")
            .build_phase(),
        Some(BuildPhase::Constructing { site })
    );
}
