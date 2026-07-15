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
        let site = entities.get_mut(site).expect("site should exist");
        assert!(site.mark_construction_cost_paid());
    }
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
        vec![(
            1,
            SimCommand::Cancel {
                building: site,
                construction: true,
            },
        )],
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
    assert!(entities
        .get_mut(site)
        .expect("site should exist")
        .mark_construction_cost_paid());

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            2,
            SimCommand::Cancel {
                building: site,
                construction: true,
            },
        )],
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
        vec![(
            1,
            SimCommand::Cancel {
                building: site,
                construction: true,
            },
        )],
    );
    assert!(
        !entities.contains(site),
        "the owner can cancel a scaffold without a builder"
    );
    assert_eq!(players[0].steel, starting_steel);
    assert_eq!(players[0].oil, starting_oil);
}

#[test]
fn cancel_unpaid_authored_scaffold_does_not_refund_or_change_score() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (site_x, site_y) = footprint_center(&map, EntityKind::Depot, 8, 8);
    let site = entities
        .spawn_building(1, EntityKind::Depot, site_x, site_y, false)
        .expect("authored scaffold should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let starting_steel = players[0].steel;
    let starting_oil = players[0].oil;
    let starting_score = players[0].score.structure_score;

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Cancel {
                building: site,
                construction: true,
            },
        )],
    );

    assert!(
        !entities.contains(site),
        "the authored scaffold is cancelled"
    );
    assert_eq!(players[0].steel, starting_steel);
    assert_eq!(players[0].oil, starting_oil);
    assert_eq!(players[0].score.structure_score, starting_score);
}

#[test]
fn cancellation_scope_cannot_cross_from_construction_into_production() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::Barracks, 8, 8);
    let completed = entities
        .spawn_building(1, EntityKind::Barracks, x, y, true)
        .expect("completed barracks should spawn");
    entities
        .get_mut(completed)
        .expect("barracks should exist")
        .push_production(ProdItem {
            unit: EntityKind::Rifleman,
            progress: 0,
            total: 30,
            paid: true,
        });
    let scaffold = entities
        .spawn_building(1, EntityKind::Depot, x + 160.0, y, false)
        .expect("depot scaffold should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Cancel {
                building: completed,
                construction: true,
            },
        )],
    );
    assert_eq!(
        entities
            .get(completed)
            .expect("completed building should survive")
            .prod_queue()
            .len(),
        1,
        "a delayed construction cancellation must not cancel completed-building production"
    );

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Cancel {
                building: scaffold,
                construction: false,
            },
        )],
    );
    assert!(
        entities.contains(scaffold),
        "a production cancellation must not remove an unfinished building"
    );
}
