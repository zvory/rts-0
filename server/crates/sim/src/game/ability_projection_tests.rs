use super::*;
use crate::game::entity::EntityKind;

fn human_vs_ai_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Human".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Computer".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ]
}

fn ekat_vs_ai_players() -> [PlayerInit; 2] {
    let mut players = human_vs_ai_players();
    players[0].faction_id = "ekat".to_string();
    players
}

fn first_tile_matching(game: &Game, predicate: impl Fn(f32, f32) -> bool) -> (f32, f32) {
    (0..game.state.map.size)
        .flat_map(|ty| (0..game.state.map.size).map(move |tx| (tx, ty)))
        .find_map(|(tx, ty)| {
            let (x, y) = game.state.map.tile_center(tx, ty);
            predicate(x, y).then_some((x, y))
        })
        .expect("map should contain a matching tile")
}

fn ability_object_spec(
    owner: u32,
    caster_id: u32,
    x: f32,
    y: f32,
) -> ability_runtime::AbilityWorldObjectSpec {
    ability_runtime::AbilityWorldObjectSpec {
        owner,
        caster_id,
        ability: ability::AbilityKind::EkatTeleport,
        kind: ability_runtime::AbilityWorldObjectKind::ReturnMarker,
        x,
        y,
        created_tick: 0,
        expires_tick: 120,
        payload: ability_runtime::AbilityObjectPayload::DashReturn {
            earliest_return_tick: 8,
        },
    }
}

#[test]
fn ability_objects_are_projected_only_when_position_is_visible() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let p1_visible = first_tile_matching(&game, |x, y| game.state.fog.is_visible_world(1, x, y));
    let hidden_from_p1 = first_tile_matching(&game, |x, y| !game.state.fog.is_visible_world(1, x, y));
    let caster = game.state.entities
        .spawn_unit(1, EntityKind::Ekat, p1_visible.0, p1_visible.1)
        .expect("caster should spawn");
    let visible_object = game
        .spawn_ability_world_object_for_test(ability_object_spec(
            1,
            caster,
            p1_visible.0,
            p1_visible.1,
        ))
        .expect("visible object should spawn");
    let hidden_object = game
        .spawn_ability_world_object_for_test(ability_object_spec(
            1,
            caster,
            hidden_from_p1.0,
            hidden_from_p1.1,
        ))
        .expect("hidden object should spawn");

    let snapshot = game.snapshot_for(1);

    assert!(snapshot
        .ability_objects
        .iter()
        .any(|object| object.id == visible_object));
    assert!(!snapshot
        .ability_objects
        .iter()
        .any(|object| object.id == hidden_object));
}

#[test]
fn enemy_ability_object_projection_does_not_leak_owner_only_fields() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let p1_visible_p2_hidden = first_tile_matching(&game, |x, y| {
        game.state.fog.is_visible_world(1, x, y) && !game.state.fog.is_visible_world(2, x, y)
    });
    let p2_visible_p1_hidden = first_tile_matching(&game, |x, y| {
        game.state.fog.is_visible_world(2, x, y) && !game.state.fog.is_visible_world(1, x, y)
    });
    let caster = game.state.entities
        .spawn_unit(
            2,
            EntityKind::Ekat,
            p2_visible_p1_hidden.0,
            p2_visible_p1_hidden.1,
        )
        .expect("enemy caster should spawn");
    let object_id = game
        .spawn_ability_world_object_for_test(ability_object_spec(
            2,
            caster,
            p1_visible_p2_hidden.0,
            p1_visible_p2_hidden.1,
        ))
        .expect("enemy object should spawn");

    let snapshot = game.snapshot_for(1);
    let object = snapshot
        .ability_objects
        .iter()
        .find(|object| object.id == object_id)
        .expect("visible enemy object should project");

    assert_eq!(object.owner, 2);
    assert_eq!(
        object.kind,
        crate::protocol::ability_object_kinds::RETURN_MARKER
    );
    assert_eq!(object.source_caster_id, None);
    assert_eq!(object.owner_state, None);
}

#[test]
fn spectator_and_full_snapshots_project_ability_objects() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let p1_visible = first_tile_matching(&game, |x, y| game.state.fog.is_visible_world(1, x, y));
    let hidden_from_all = first_tile_matching(&game, |x, y| {
        !game.state.fog.is_visible_world(1, x, y) && !game.state.fog.is_visible_world(2, x, y)
    });
    let caster = game.state.entities
        .spawn_unit(1, EntityKind::Ekat, p1_visible.0, p1_visible.1)
        .expect("caster should spawn");
    let visible_id = game
        .spawn_ability_world_object_for_test(ability_object_spec(
            1,
            caster,
            p1_visible.0,
            p1_visible.1,
        ))
        .expect("visible object should spawn");
    let hidden_id = game
        .spawn_ability_world_object_for_test(ability_object_spec(
            1,
            caster,
            hidden_from_all.0,
            hidden_from_all.1,
        ))
        .expect("hidden object should spawn");

    let spectator = game.snapshot_for_spectator(&[1, 2]);
    let full = game.snapshot_full_for(1);

    assert!(spectator
        .ability_objects
        .iter()
        .any(|object| object.id == visible_id && object.owner_state.is_some()));
    assert!(!spectator
        .ability_objects
        .iter()
        .any(|object| object.id == hidden_id));
    assert!(full
        .ability_objects
        .iter()
        .any(|object| object.id == hidden_id && object.source_caster_id == Some(caster)));
}

#[test]
fn owner_entity_abilities_project_active_return_affordance() {
    let players = ekat_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let caster = game.state.entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Ekat)
        .map(|entity| entity.id)
        .expect("Ekat loadout should spawn a caster");
    let (x, y) = game.state.entities
        .get(caster)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("caster should exist");
    let marker_id = game
        .spawn_ability_world_object_for_test(ability_object_spec(1, caster, x, y))
        .expect("return marker should spawn");

    let owner_snapshot = game.snapshot_for(1);
    let owner_ekat = owner_snapshot
        .entities
        .iter()
        .find(|entity| entity.id == caster)
        .expect("owner should see Ekat");
    let affordance = owner_ekat
        .abilities
        .iter()
        .find(|ability| ability.ability == crate::protocol::abilities::EKAT_TELEPORT)
        .expect("Ekat dash affordance should project");
    assert_eq!(affordance.active_object_id, Some(marker_id));
    assert_eq!(affordance.available_tick, Some(8));
    assert_eq!(affordance.expires_in, Some(120));

    let enemy_snapshot = game.snapshot_for(2);
    assert!(enemy_snapshot
        .entities
        .iter()
        .filter(|entity| entity.id == caster)
        .all(|entity| entity.abilities.is_empty()));
}

#[test]
fn recast_return_execution_accepts_active_marker() {
    let players = ekat_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let caster = game.state.entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Ekat)
        .map(|entity| entity.id)
        .expect("Ekat loadout should spawn a caster");
    let (x, y) = game.state.entities
        .get(caster)
        .map(|entity| (entity.pos_x + 96.0, entity.pos_y))
        .expect("caster should exist");
    let marker_id = game
        .spawn_ability_world_object_for_test(ability_object_spec(1, caster, x, y))
        .expect("return marker should spawn");

    let mut events = std::collections::HashMap::new();
    assert!(services::ability_orders::execute_recast_return(
        &game.state.map,
        &mut game.state.entities,
        &mut game.state.ability_runtime,
        &mut events,
        1,
        "ekat",
        ability::AbilityKind::EkatTeleport,
        vec![caster],
        Some(marker_id),
        8,
    ));
}

#[test]
fn recast_return_execution_rejects_missing_too_early_and_stale_state() {
    let players = ekat_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let caster = game.state.entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Ekat)
        .map(|entity| entity.id)
        .expect("Ekat loadout should spawn a caster");
    let (x, y) = game.state.entities
        .get(caster)
        .map(|entity| (entity.pos_x + 96.0, entity.pos_y))
        .expect("caster should exist");
    let marker_id = game
        .spawn_ability_world_object_for_test(ability_object_spec(1, caster, x, y))
        .expect("return marker should spawn");

    let mut events = std::collections::HashMap::new();
    assert!(!services::ability_orders::execute_recast_return(
        &game.state.map,
        &mut game.state.entities,
        &mut game.state.ability_runtime,
        &mut events,
        1,
        "ekat",
        ability::AbilityKind::EkatTeleport,
        vec![caster],
        Some(marker_id + 1),
        8,
    ));
    assert!(!services::ability_orders::execute_recast_return(
        &game.state.map,
        &mut game.state.entities,
        &mut game.state.ability_runtime,
        &mut events,
        1,
        "ekat",
        ability::AbilityKind::EkatTeleport,
        vec![caster],
        Some(marker_id),
        7,
    ));

    game.state.entities.remove(caster);
    assert!(!services::ability_orders::execute_recast_return(
        &game.state.map,
        &mut game.state.entities,
        &mut game.state.ability_runtime,
        &mut events,
        1,
        "ekat",
        ability::AbilityKind::EkatTeleport,
        vec![caster],
        Some(marker_id),
        9,
    ));
}
