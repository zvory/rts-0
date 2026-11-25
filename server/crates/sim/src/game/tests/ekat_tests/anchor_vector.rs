use super::*;
use rts_rules::math::GameplayMath as _;

#[test]
fn ekat_magic_anchor_adds_same_inward_vector_for_idle_and_every_move_direction() {
    for destination in [
        None,
        Some((10.0, 0.0)),
        Some((-10.0, 0.0)),
        Some((0.0, 10.0)),
    ] {
        let AnchorVectorStep {
            position: base,
            start,
            center,
            ..
        } = anchor_vector_step(destination, 2, false);
        let AnchorVectorStep {
            position: pulled,
            delta: reported_delta,
            ..
        } = anchor_vector_step(destination, 2, true);
        let dx = center.0 - base.0;
        let dy = center.1 - base.1;
        let distance = (dx * dx + dy * dy).det_sqrt();
        let radius = config::EKAT_MAGIC_ANCHOR_RADIUS_TILES * config::TILE_SIZE as f32;
        let speed = config::unit_stats(EntityKind::Rifleman).unwrap().speed;
        let magnitude = speed * 2.0 * (1.0 - distance / radius);
        assert!(
            (pulled.0 - base.0 - dx / distance * magnitude).abs() < 0.02,
            "destination={destination:?}, pulled={pulled:?}, base={base:?}"
        );
        assert!((pulled.1 - base.1 - dy / distance * magnitude).abs() < 0.02);
        assert!((reported_delta.0 - (pulled.0 - start.0)).abs() < 0.02);
        assert!((reported_delta.1 - (pulled.1 - start.1)).abs() < 0.02);
        // At this distance the inward vector is stronger than normal retreat.
        assert!(
            pulled.0 > start.0,
            "even retreat must move inward near the center"
        );
        if destination == Some((0.0, 10.0)) {
            assert!(
                pulled.1 > start.1,
                "sideways movement remains while bending inward"
            );
        }
    }
}

#[test]
fn ekat_magic_anchor_leaves_owned_and_allied_movement_unchanged() {
    for owner in [1, 3] {
        for destination in [
            None,
            Some((10.0, 0.0)),
            Some((-10.0, 0.0)),
            Some((0.0, 10.0)),
        ] {
            let base = anchor_vector_step(destination, owner, false).position;
            let pulled = anchor_vector_step(destination, owner, true).position;
            assert_eq!(pulled, base, "owner={owner}, destination={destination:?}");
        }
    }
}

struct AnchorVectorStep {
    position: (f32, f32),
    start: (f32, f32),
    center: (f32, f32),
    delta: (f32, f32),
}

// Return final position, start, field center, and recorded movement for one measured tick.
fn anchor_vector_step(
    destination: Option<(f32, f32)>,
    owner: u32,
    with_anchor: bool,
) -> AnchorVectorStep {
    let ally = PlayerInit {
        id: 3,
        team_id: 1,
        ..kriegsia_enemy()
    };
    let mut game = empty_flat_game(&[ekat_player(), kriegsia_enemy(), ally]);
    let start = game.state.map.tile_center(10, 10);
    let center = game.state.map.tile_center(11, 10);
    let unit = game
        .state
        .entities
        .spawn_unit(owner, EntityKind::Rifleman, start.0, start.1)
        .unwrap();
    let caster = game
        .state
        .entities
        .spawn_unit(
            1,
            EntityKind::Ekat,
            center.0,
            center.1 + 6.5 * config::TILE_SIZE as f32,
        )
        .unwrap();
    if with_anchor {
        enqueue_ekat_anchor(&mut game, caster, center);
    }
    game.tick();
    game.state
        .entities
        .get_mut(unit)
        .unwrap()
        .set_position(start.0, start.1);
    if let Some((dx, dy)) = destination {
        game.enqueue(
            owner,
            Command::Move {
                units: vec![unit],
                x: start.0 + dx * config::TILE_SIZE as f32,
                y: start.1 + dy * config::TILE_SIZE as f32,
                queued: false,
            },
        );
    }
    game.tick();
    let entity = game.state.entities.get(unit).unwrap();
    AnchorVectorStep {
        position: (entity.pos_x(), entity.pos_y()),
        start,
        center,
        delta: entity.movement_delta(),
    }
}
