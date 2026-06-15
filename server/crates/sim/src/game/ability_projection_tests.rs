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

fn first_tile_matching(game: &Game, predicate: impl Fn(f32, f32) -> bool) -> (f32, f32) {
    (0..game.map.size)
        .flat_map(|ty| (0..game.map.size).map(move |tx| (tx, ty)))
        .find_map(|(tx, ty)| {
            let (x, y) = game.map.tile_center(tx, ty);
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
    let p1_visible = first_tile_matching(&game, |x, y| game.fog.is_visible_world(1, x, y));
    let hidden_from_p1 = first_tile_matching(&game, |x, y| !game.fog.is_visible_world(1, x, y));
    let caster = game
        .entities
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
        game.fog.is_visible_world(1, x, y) && !game.fog.is_visible_world(2, x, y)
    });
    let p2_visible_p1_hidden = first_tile_matching(&game, |x, y| {
        game.fog.is_visible_world(2, x, y) && !game.fog.is_visible_world(1, x, y)
    });
    let caster = game
        .entities
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
    let p1_visible = first_tile_matching(&game, |x, y| game.fog.is_visible_world(1, x, y));
    let hidden_from_all = first_tile_matching(&game, |x, y| {
        !game.fog.is_visible_world(1, x, y) && !game.fog.is_visible_world(2, x, y)
    });
    let caster = game
        .entities
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
