use super::*;
use crate::game::ability::AbilityKind;
use crate::game::ability_runtime::{
    AbilityObjectPayload, AbilityWorldObjectKind, AbilityWorldObjectSpec,
};

fn anchor(center: (f32, f32), expires_tick: u32) -> AbilityRuntime {
    let mut runtime = AbilityRuntime::new();
    runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: 999,
            ability: AbilityKind::EkatMagicAnchor,
            kind: AbilityWorldObjectKind::MagicAnchor,
            x: center.0,
            y: center.1,
            created_tick: 0,
            expires_tick,
            payload: AbilityObjectPayload::MagicAnchor {
                radius: config::EKAT_MAGIC_ANCHOR_RADIUS_TILES * config::TILE_SIZE as f32,
            },
        })
        .unwrap();
    runtime
}

#[test]
fn anchor_pull_clamps_at_center_and_ignores_expired_or_outside_fields() {
    let map = flat_map(2);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let center = map.tile_center(12, 12);
    for (offset, expires_tick, expected_offset) in [
        (-0.25, 100, 0.0),
        (0.0, 100, 0.0),
        (-0.25, 1, -0.25),
        (-96.0, 100, -96.0),
        (-100.0, 100, -100.0),
    ] {
        let mut entities = EntityStore::new();
        let unit = entities
            .spawn_unit(2, EntityKind::Rifleman, center.0 + offset, center.1)
            .unwrap();
        let occ = Occupancy::build(&map, &entities);
        apply_magic_anchor_pull(
            &map,
            &mut entities,
            &players,
            &occ,
            1,
            &anchor(center, expires_tick),
        );
        assert!((pos(&entities, unit).0 - center.0 - expected_offset).abs() < 0.01);
    }
}

#[test]
fn anchor_vector_respects_enemy_body_blockers_and_preserves_path() {
    let map = flat_map(2);
    let players = [player_with_oil(1, 0), player_with_oil(2, 0)];
    let start = map.tile_center(10, 10);
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(2, EntityKind::Rifleman, start.0, start.1)
        .unwrap();
    let radius = entities.get(unit).unwrap().radius();
    entities
        .spawn_unit(
            1,
            EntityKind::Rifleman,
            start.0 + 2.0 * radius + 0.1,
            start.1,
        )
        .unwrap();
    let destination = (start.0, start.1 + 200.0);
    entities.get_mut(unit).unwrap().set_path(vec![destination]);
    let occ = Occupancy::build(&map, &entities);
    let runtime = anchor((start.0 + 32.0, start.1), 100);
    apply_magic_anchor_pull(&map, &mut entities, &players, &occ, 1, &runtime);
    assert_eq!(
        pos(&entities, unit),
        start,
        "pull must not cross an enemy body"
    );
    assert!(
        !entities.get(unit).unwrap().path_is_empty(),
        "pull must preserve movement intent"
    );
}
