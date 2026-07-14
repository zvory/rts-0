use super::*;

#[test]
fn cancel_unfinished_building_refunds_full_cost_and_releases_builder() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::Factory, 8, 8);
    let site = entities
        .spawn_building(1, EntityKind::Factory, site_x, site_y, false)
        .expect("factory scaffold should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, site_x, site_y)
        .expect("builder should spawn");
    let handoff = OrderIntent::move_to(site_x + 96.0, site_y);
    {
        let builder = entities.get_mut(worker).expect("builder should exist");
        builder.set_order(Order::build(EntityKind::Factory, 8, 8));
        builder.mark_build_phase(BuildPhase::Constructing { site });
        builder.set_target_id(Some(site));
        builder.append_queued_order(handoff.clone());
    }

    let mut players = vec![player_state(1), player_state(2)];
    let cost = rules::economy::resource_cost(EntityKind::Factory);
    let starting_steel = players[0].steel;
    let starting_oil = players[0].oil;
    assert!(players[0].spend_cost(cost));
    players[0].record_entity_created(EntityKind::Factory);

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, SimCommand::Cancel { building: site })],
    );

    assert!(
        !entities.contains(site),
        "cancel should remove the scaffold immediately"
    );
    let builder = entities
        .get(worker)
        .expect("builder should survive cancellation");
    assert!(
        matches!(builder.order(), Order::Idle),
        "cancel should release the active builder"
    );
    assert_eq!(
        builder.queued_orders(),
        &[handoff],
        "cancel should preserve the builder's queued follow-up orders"
    );
    assert_eq!(
        players[0].steel, starting_steel,
        "cancel should fully refund Steel"
    );
    assert_eq!(
        players[0].oil, starting_oil,
        "cancel should fully refund Oil"
    );
    assert_eq!(
        players[0].score.structure_score, 0,
        "cancelled construction should not inflate structure score"
    );
    assert_eq!(
        players[0].score.buildings_lost, 0,
        "cancelled construction should not count as a destroyed building"
    );
}

#[test]
fn cancel_unworked_scaffold_refunds_owner_and_rejects_enemy() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::ResearchComplex, 8, 8);
    let site = entities
        .spawn_building(1, EntityKind::ResearchComplex, site_x, site_y, false)
        .expect("research scaffold should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let cost = rules::economy::resource_cost(EntityKind::ResearchComplex);
    let starting_steel = players[0].steel;
    let starting_oil = players[0].oil;
    assert!(players[0].spend_cost(cost));
    players[0].record_entity_created(EntityKind::ResearchComplex);

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(2, SimCommand::Cancel { building: site })],
    );
    assert!(
        entities.contains(site),
        "another player cannot cancel the scaffold"
    );
    assert_eq!(players[0].steel, starting_steel - cost.steel);
    assert_eq!(players[0].oil, starting_oil - cost.oil);

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, SimCommand::Cancel { building: site })],
    );
    assert!(
        !entities.contains(site),
        "the owner can cancel a scaffold without a builder"
    );
    assert_eq!(players[0].steel, starting_steel);
    assert_eq!(players[0].oil, starting_oil);
}
