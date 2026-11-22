use super::*;

#[test]
fn war_smaller_body_controls_collision() {
    use crate::game::entity::EntityStore;
    let mut entities = EntityStore::new();
    let war = entities
        .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
        .unwrap();
    let other = entities
        .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
        .unwrap();
    let unit = entities.get_mut(war).unwrap();
    unit.set_disciple_kind_for_test(EntityKind::WarAdept);
    assert!((unit.radius() - 9.45).abs() < 0.001);
    let body = unit_body_for_entity(unit).unwrap();
    assert!((body.bounding_radius() - unit.radius()).abs() < 0.001);
    assert!(unit_body_overlap(
        body,
        unit_body_for_entity(entities.get(other).unwrap()).unwrap()
    )
    .is_none());
    entities
        .get_mut(war)
        .unwrap()
        .set_disciple_kind_for_test(EntityKind::Warrior);
    assert!(unit_body_overlap(
        unit_body_for_entity(entities.get(war).unwrap()).unwrap(),
        unit_body_for_entity(entities.get(other).unwrap()).unwrap()
    )
    .is_some());
}
