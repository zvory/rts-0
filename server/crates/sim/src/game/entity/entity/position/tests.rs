use super::*;
use crate::game::entity::EntityKind;

#[test]
fn physical_writes_quantize_and_reject_invalid_pairs_atomically() {
    let step = 1.0 / Coord::SCALE as f32;
    let mut entity = Entity::new_unit(1, EntityKind::Rifleman, 100.0 + step * 0.5, 100.0).unwrap();
    assert_eq!(entity.pos_x(), 100.0 + step);
    entity.set_position(16384.0, 16384.0);
    assert_eq!((entity.pos_x(), entity.pos_y()), (16384.0, 16384.0));
    for invalid in [f32::NAN, f32::INFINITY, f32::MAX] {
        entity.set_position(1.0, invalid);
        assert_eq!((entity.pos_x(), entity.pos_y()), (16384.0, 16384.0));
        assert!(Entity::new_unit(1, EntityKind::Rifleman, invalid, 1.0).is_none());
    }
    let json = serde_json::to_value(&entity).unwrap();
    assert_eq!(json["pos_x"].as_i64(), Some(67_108_864));
    let restored: Entity = serde_json::from_value(json).unwrap();
    assert_eq!(restored.pos_x(), entity.pos_x());
}
