use super::*;

#[test]
fn spectator_union_uses_selected_owner_knowledge_of_destroyed_buildings() {
    for kind in [
        EntityKind::ResourceDepot,
        EntityKind::SteelMine,
        EntityKind::PumpJack,
    ] {
        let mut game = empty_flat_game_with_players(&three_players());
        let scout_pos = game.state.map.tile_center(19, 20);
        let building_pos = game.state.map.tile_center(22, 20);
        let scout = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
            .expect("scout should spawn");
        let building = game
            .state
            .entities
            .spawn_building(3, kind, building_pos.0, building_pos.1, true)
            .expect("building should spawn");
        let surviving_pos = game.state.map.tile_center(40, 40);
        let surviving = game
            .state
            .entities
            .spawn_building(3, kind, surviving_pos.0, surviving_pos.1, true)
            .expect("surviving building should spawn");
        if let Some(node_kind) = kind.extracted_resource_kind() {
            for pos in [building_pos, surviving_pos] {
                game.state
                    .entities
                    .spawn_node(node_kind, pos.0, pos.1)
                    .expect("extractor resource node should spawn");
            }
        }
        advance_to_next_fog_refresh(&mut game);
        assert!(
            game.state.building_memory.get(1, building).is_some(),
            "initial memory: {kind:?}"
        );
        game.state.entities.remove(scout);
        advance_to_next_fog_refresh(&mut game);
        assert!(
            game.state.building_memory.get(1, building).is_some(),
            "memory before destruction: {kind:?}"
        );
        game.state.entities.remove(building);
        advance_to_next_fog_refresh(&mut game);

        let remembers = |game: &Game, selected: Vec<u32>| {
            game.snapshot_for_observer(&ObserverView::Players(selected))
                .remembered_buildings
                .iter()
                .any(|entry| entry.id == building)
        };
        assert!(
            remembers(&game, vec![1]),
            "hidden destruction stays unknown to the enemy: {kind:?}"
        );
        assert!(
            remembers(&game, vec![1, 2]),
            "unrelated selected players do not reveal destruction"
        );
        assert!(
            !remembers(&game, vec![1, 3]),
            "the selected owner knows its building was destroyed"
        );
        assert!(
            !remembers(&game, vec![3, 1]),
            "selection order must not change owner knowledge"
        );
        assert!(
            !remembers(&game, vec![1, 2, 3]),
            "all-player vision must not resurrect destroyed buildings"
        );

        let union = game.snapshot_for_spectator(&[1, 2, 3]);
        let live = union
            .entities
            .iter()
            .find(|entry| entry.id == surviving)
            .expect("surviving buildings must remain live with HP");
        assert!(live.hp > 0);
        assert!(live.max_hp > 0);
        assert!(union
            .remembered_buildings
            .iter()
            .all(|entry| entry.id != surviving));

        let checkpoint = game
            .checkpoint_payload_text_for_test()
            .expect("checkpoint should serialize");
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &checkpoint,
            game.state.map.clone(),
            game.map_metadata().clone(),
        )
        .expect("checkpoint should restore");
        assert!(remembers(&restored, vec![1]));
        assert!(
            !remembers(&restored, vec![1, 3]),
            "owner knowledge must survive replay seek/restore"
        );
    }
}

#[test]
fn spectator_union_does_not_infer_hidden_neutral_trap_destruction_from_its_builder() {
    let mut game = empty_flat_game_with_players(&three_players());
    let scout_pos = game.state.map.tile_center(20, 20);
    let trap_pos = game.state.map.tile_center(22, 20);
    let scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, scout_pos.0, scout_pos.1)
        .expect("scout should spawn");
    let trap = game
        .state
        .entities
        .spawn_building(3, EntityKind::TankTrap, trap_pos.0, trap_pos.1, false)
        .expect("Tank Trap scaffold should spawn");
    advance_to_next_fog_refresh(&mut game);
    game.state.entities.remove(scout);
    // Finish immediately before a fog refresh, after the observers have left.
    game.tick();
    let scaffold = game.state.entities.get_mut(trap).expect("scaffold exists");
    let total = scaffold.construction.as_ref().expect("construction").total;
    assert!(scaffold.set_construction_progress(total.saturating_sub(1)));
    assert_eq!(scaffold.advance_construction(), Some(true));
    advance_to_next_fog_refresh(&mut game);

    let before = game.snapshot_for(1);
    assert!(before.entities.iter().all(|entity| entity.id != trap));
    assert!(before
        .remembered_buildings
        .iter()
        .any(|entry| { entry.id == trap && entry.owner == 3 }));
    assert!(game.state.building_memory.get(3, trap).is_none());
    let selected_before = game.snapshot_for_spectator(&[1, 3]);
    assert!(selected_before
        .entities
        .iter()
        .all(|entity| entity.id != trap));
    assert_eq!(
        before.remembered_buildings,
        selected_before.remembered_buildings
    );

    game.state.entities.remove(trap);
    advance_to_next_fog_refresh(&mut game);
    let after = game.snapshot_for_spectator(&[1, 3]);
    assert_eq!(
        before.remembered_buildings, after.remembered_buildings,
        "the former owner has no privileged knowledge of hidden neutral destruction"
    );
}
