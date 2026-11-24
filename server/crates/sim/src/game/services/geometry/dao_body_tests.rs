use super::*;

#[test]
fn master_rank_enlarges_collision_and_projectile_geometry() {
    use crate::game::entity::EntityStore;
    for kind in [
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        let mut entities = EntityStore::new();
        let id = entities.spawn_unit(1, kind, 100.0, 100.0).unwrap();
        let unit = entities.get_mut(id).unwrap();
        let adept = unit_body_for_entity(unit).unwrap();
        let radius = adept.bounding_radius();
        let neighbor = UnitBody::Circle(CircleBody {
            x: 100.0 + radius * 1.1 + 1.0,
            y: 100.0,
            radius: 1.0,
        });
        let start = (100.0 - radius * 2.0, 100.0 + radius * 1.1);
        let end = (100.0 + radius * 2.0, start.1);
        assert!(unit_body_overlap(adept, neighbor).is_none());
        assert!(segment_intersects_unit_body(start, end, adept).is_none());

        unit.dao_master = true;
        let master = unit_body_for_entity(unit).unwrap();
        assert!((master.bounding_radius() - radius * config::DAO_MASTER_BODY_SCALE).abs() < 0.001);
        assert!((master.bounding_radius() - unit.radius()).abs() < 0.001);
        assert!(unit_body_overlap(master, neighbor).is_some());
        assert!(segment_intersects_unit_body(start, end, master).is_some());
    }
}

#[test]
fn war_smaller_body_controls_collision() {
    use crate::game::entity::EntityStore;
    let mut entities = EntityStore::new();
    let war = entities
        .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
        .unwrap();
    let other = entities
        .spawn_unit(2, EntityKind::Rifleman, 132.0, 100.0)
        .unwrap();
    let unit = entities.get_mut(war).unwrap();
    unit.set_disciple_kind_for_test(EntityKind::WarAdept);
    assert!((unit.radius() - 18.2).abs() < 0.001);
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
