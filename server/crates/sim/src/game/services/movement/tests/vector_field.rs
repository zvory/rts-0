use super::*;
use crate::game::ability::AbilityKind;
use crate::game::ability_runtime::{
    AbilityObjectPayload, AbilityWorldObjectKind, AbilityWorldObjectSpec,
};
fn field(center: (f32, f32), direction: [f32; 2]) -> AbilityRuntime {
    let mut runtime = AbilityRuntime::new();
    runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: 999,
            ability: AbilityKind::VectorField,
            kind: AbilityWorldObjectKind::VectorField,
            x: center.0,
            y: center.1,
            created_tick: 0,
            expires_tick: 100,
            payload: AbilityObjectPayload::VectorField {
                direction,
                size_tiles: 4.0,
            },
        })
        .unwrap();
    runtime
}
#[test]
fn vector_field_rectangle_is_centered_rotated_and_expires() {
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let runtime = field((400.0, 400.0), [0.0, 1.0]);
    assert_eq!(
        runtime.vector_field_at(2, &teams, 463.0, 463.0, 1),
        Some([0.0, 1.0])
    );
    assert_eq!(runtime.vector_field_at(2, &teams, 465.0, 400.0, 1), None);
    assert_eq!(runtime.vector_field_at(2, &teams, 400.0, 465.0, 1), None);
    assert_eq!(runtime.vector_field_at(2, &teams, 400.0, 400.0, 100), None);
}
#[test]
fn vector_field_moves_enemy_bodies_three_pixels_and_spares_owner_allies_and_neutrals() {
    let map = flat_map(2);
    let center = map.tile_center(10, 10);
    let mut players = [
        player_with_oil(1, 0),
        player_with_oil(2, 0),
        player_with_oil(3, 0),
    ];
    players[0].team_id = 1;
    players[1].team_id = 2;
    players[2].team_id = 1;
    for owner in [0, 1, 2, 3] {
        for kind in [EntityKind::Rifleman, EntityKind::Tank] {
            let mut entities = EntityStore::new();
            let id = entities
                .spawn_unit(owner, kind, center.0, center.1)
                .unwrap();
            let occ = Occupancy::build(&map, &entities);
            apply_magic_anchor_pull(
                &map,
                &mut entities,
                &players,
                &occ,
                1,
                &field(center, [0.0, 1.0]),
            );
            let unit = entities.get(id).unwrap();
            assert_eq!(unit.pos_x(), center.0);
            let expected = if owner == 2 { 3.0 } else { 0.0 };
            assert!(
                (unit.pos_y() - center.1 - expected).abs() < 0.02,
                "{kind:?} owner={owner}"
            );
            assert!(
                unit.facing().abs() < 0.001,
                "field displacement does not rotate the body"
            );
        }
    }
}

#[test]
fn vector_field_is_distinct_from_ekat_anchor_for_both_magic_ranks() {
    use crate::rules::faction::{ability_definition, catalog_for, CULTIVATORS_FACTION_ID};
    let catalog = catalog_for(CULTIVATORS_FACTION_ID).unwrap();
    assert!(catalog.allows_ability(AbilityKind::VectorField, EntityKind::MagicAdept));
    assert!(!catalog.allows_ability(AbilityKind::EkatMagicAnchor, EntityKind::MagicAdept));
    assert!(!ability_definition(AbilityKind::VectorField)
        .carriers
        .contains(&EntityKind::Ekat));
    assert!(ability_definition(AbilityKind::EkatMagicAnchor)
        .carriers
        .contains(&EntityKind::Ekat));
    for master in [false, true] {
        let mut entities = EntityStore::new();
        let id = entities
            .spawn_unit(1, EntityKind::MagicAdept, 320.0, 320.0)
            .unwrap();
        entities.get_mut(id).unwrap().dao_master = master;
        assert!(crate::game::ability::carried_by(
            AbilityKind::VectorField,
            entities.get(id).unwrap().kind
        ));
    }
}

#[test]
fn vector_field_master_has_five_tile_square_instead_of_four() {
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let mut runtime = field((400.0, 400.0), [1.0, 0.0]);
    assert_eq!(runtime.vector_field_at(2, &teams, 479.0, 479.0, 1), None);
    runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: 1000,
            ability: AbilityKind::VectorField,
            kind: AbilityWorldObjectKind::VectorField,
            x: 400.0,
            y: 400.0,
            created_tick: 0,
            expires_tick: 100,
            payload: AbilityObjectPayload::VectorField {
                direction: [1.0, 0.0],
                size_tiles: 5.0,
            },
        })
        .unwrap();
    assert_eq!(
        runtime.vector_field_at(2, &teams, 479.0, 479.0, 1),
        Some([1.0, 0.0])
    );
    assert_eq!(runtime.vector_field_at(2, &teams, 481.0, 400.0, 1), None);
}

#[test]
fn vector_field_skips_friendly_overlap_to_find_first_hostile_field() {
    let teams = crate::game::teams::TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 1)]);
    let mut runtime = field((400.0, 400.0), [1.0, 0.0]);
    runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 2,
            caster_id: 1000,
            ability: AbilityKind::VectorField,
            kind: AbilityWorldObjectKind::VectorField,
            x: 400.0,
            y: 400.0,
            created_tick: 0,
            expires_tick: 100,
            payload: AbilityObjectPayload::VectorField {
                direction: [0.0, 1.0],
                size_tiles: 4.0,
            },
        })
        .unwrap();
    for owner in [1, 3] {
        assert_eq!(
            runtime.vector_field_at(owner, &teams, 400.0, 400.0, 1),
            Some([0.0, 1.0])
        );
    }
    assert_eq!(
        runtime.vector_field_at(2, &teams, 400.0, 400.0, 1),
        Some([1.0, 0.0])
    );
}
