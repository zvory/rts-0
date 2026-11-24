use super::*;
use crate::game::hero_abilities;

fn flat_map() -> Map {
    let mut map = Map::generate(2, 123);
    map.terrain.fill(crate::protocol::terrain::GRASS);
    map
}

#[test]
fn blink_selects_nearest_legal_lattice_point_and_group_landings_do_not_stack() {
    let map = flat_map();
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(2, EntityKind::Tank, 600.0, 600.0)
        .unwrap();
    let caster = entities
        .spawn_unit(1, EntityKind::WindAdept, 400.0, 600.0)
        .unwrap();
    let requested = (600.0, 600.0);
    let landing = hero_abilities::ekat_dash_destination(
        &map,
        &entities,
        caster,
        requested.0,
        requested.1,
        Some(15.0),
    )
    .unwrap();
    let occ = Occupancy::build(&map, &entities);
    let blockers = UnitBlockers::new(&entities);
    let unit = entities.get(caster).unwrap();
    let chosen_d2 = (landing.0 - requested.0).powi(2) + (landing.1 - requested.1).powi(2);
    for x in -60..=60 {
        for y in -60..=60 {
            if (x * x + y * y) as f32 >= chosen_d2 {
                continue;
            }
            let pos = (requested.0 + x as f32, requested.1 + y as f32);
            assert!(
                !unit_static_standable_with_radius(
                    &map,
                    &occ,
                    unit.kind,
                    pos.0,
                    pos.1,
                    unit.facing(),
                    Some(unit.radius())
                ) || !blockers.placement_clear(unit, pos),
                "found closer valid landing {pos:?} than {landing:?}"
            );
        }
    }
    hero_abilities::move_ekat_to(&mut entities, caster, landing.0, landing.1);
    let started = std::time::Instant::now();
    for _ in 0..16 {
        let caster = entities
            .spawn_unit(1, EntityKind::WindAdept, 400.0, 600.0)
            .unwrap();
        let landing = hero_abilities::ekat_dash_destination(
            &map,
            &entities,
            caster,
            requested.0,
            requested.1,
            Some(15.0),
        )
        .unwrap();
        assert!(
            UnitBlockers::new(&entities).placement_clear(entities.get(caster).unwrap(), landing)
        );
        hero_abilities::move_ekat_to(&mut entities, caster, landing.0, landing.1);
    }
    eprintln!("16 crowded blink searches: {:?}", started.elapsed());
    assert_eq!(
        (
            entities.get(tank).unwrap().pos_x(),
            entities.get(tank).unwrap().pos_y()
        ),
        requested
    );
}

#[test]
fn blink_search_respects_range_and_finds_no_landing_in_solid_terrain() {
    let mut map = flat_map();
    let mut entities = EntityStore::new();
    let caster = entities
        .spawn_unit(1, EntityKind::WindAdept, 400.0, 600.0)
        .unwrap();
    entities
        .spawn_unit(2, EntityKind::Tank, 432.0, 600.0)
        .unwrap();
    let landing =
        hero_abilities::ekat_dash_destination(&map, &entities, caster, 900.0, 600.0, Some(1.0))
            .unwrap();
    assert!((landing.0 - 400.0).det_hypot(landing.1 - 600.0) <= 32.0);
    map.terrain.fill(crate::protocol::terrain::WATER);
    assert!(hero_abilities::ekat_dash_destination(
        &map,
        &entities,
        caster,
        600.0,
        600.0,
        Some(15.0)
    )
    .is_none());
}

#[test]
fn occupied_return_marker_is_rejected_without_displacing_occupant() {
    let map = flat_map();
    let mut entities = EntityStore::new();
    let caster = entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 600.0)
        .unwrap();
    entities
        .spawn_unit(2, EntityKind::Tank, 600.0, 600.0)
        .unwrap();
    assert!(!hero_abilities::ekat_return_destination_valid(
        &map, &entities, caster, 600.0, 600.0
    ));
}
