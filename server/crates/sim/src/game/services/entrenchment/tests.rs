use super::*;
use crate::game::entity::EntityKind;
use crate::protocol::terrain;

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4)],
        expansion_sites: Vec::new(),
    }
}

#[test]
fn occupation_search_skips_nearest_trench_without_legal_slot() {
    let map = flat_map(32);
    let mut trenches = TrenchStore::new();
    trenches
        .create(&map, 320.0, 320.0)
        .expect("nearest trench should seed");
    let farther = trenches.create(&map, 368.0, 320.0).expect("farther trench");
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, 344.0, 320.0)
        .expect("rifleman");
    let snapshot = entities.get(unit).expect("rifleman should exist").clone();
    let nearest_trench = trenches.all().first().copied().expect("nearest trench");
    let blocking_slots = slot_positions(&snapshot, nearest_trench)
        .into_iter()
        .filter(|candidate| {
            distance((snapshot.pos_x, snapshot.pos_y), *candidate) <= SLOT_MAX_CORRECTION_PX
        })
        .collect::<Vec<_>>();
    assert!(!blocking_slots.is_empty(), "fixture should have slots to block");
    for (x, y) in blocking_slots {
        entities
            .spawn_unit(2, EntityKind::Rifleman, x, y)
            .expect("blocker");
    }
    let occ = Occupancy::build(&map, &entities);
    let indexes = EntrenchmentIndexes::build(&map, &entities, &trenches);

    let occupied_trench_counts = build_occupied_trench_counts(&entities);
    let candidate = best_occupation_candidate(
        &map,
        &entities,
        &occ,
        &indexes,
        &occupied_trench_counts,
        &snapshot,
    )
    .expect("farther trench should remain occupiable");

    assert_eq!(candidate.trench_id, farther);
}

#[test]
fn occupation_search_queries_only_local_trench_cells() {
    let map = flat_map(96);
    let mut trenches = TrenchStore::new();
    let center = map.tile_center(48, 48);
    let nearby = trenches
        .create(&map, center.0, center.1)
        .expect("nearby trench should seed");
    let search_radius = config::ENTRENCHMENT_TRENCH_RADIUS_TILES * config::TILE_SIZE as f32
        + SLOT_EXTRA_RADIUS_PX;
    for ty in (2..94).step_by(8) {
        for tx in (2..94).step_by(8) {
            let position = map.tile_center(tx, ty);
            if distance_sq(center, position) > (search_radius + config::TILE_SIZE as f32).powi(2)
            {
                trenches
                    .create(&map, position.0, position.1)
                    .expect("distant trench should seed");
            }
        }
    }
    assert!(trenches.all().len() > 100, "fixture needs a large trench field");

    let mut entities = EntityStore::new();
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, center.0, center.1)
        .expect("rifleman should spawn");
    let entity = entities.get(rifleman).expect("rifleman should exist");
    let index = TrenchSpatialIndex::build(&map, &trenches);

    let candidates = index.occupation_candidates(entity).collect::<Vec<_>>();
    assert_eq!(
        candidates.iter().map(|trench| trench.id).collect::<Vec<_>>(),
        vec![nearby],
        "distant trenches must not enter this unit's candidate work"
    );
}

#[test]
fn slot_index_tracks_prior_same_tick_slot_relocation() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let candidate_id = entities
        .spawn_unit(1, EntityKind::Rifleman, 320.0, 320.0)
        .expect("candidate should spawn");
    let blocker_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 480.0, 320.0)
        .expect("blocker should spawn");
    let candidate = entities
        .get(candidate_id)
        .expect("candidate should exist")
        .clone();
    let mut index = EntrenchmentEntityIndex::build(&map, &entities);

    assert!(
        !slot_overlaps_other_unit(&entities, &index, &candidate, (320.0, 320.0)),
        "the initial distant blocker should not reject the slot"
    );

    let old_position = (480.0, 320.0);
    let new_position = (320.0, 320.0);
    entities
        .get_mut(blocker_id)
        .expect("blocker should exist")
        .set_position(new_position.0, new_position.1);
    index.relocate(blocker_id, old_position, new_position);

    assert!(
        slot_overlaps_other_unit(&entities, &index, &candidate, new_position),
        "later units must observe an earlier same-tick slot correction"
    );
}

#[test]
fn only_firing_attack_orders_hold_ground() {
    let mut entities = EntityStore::new();
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, 320.0, 320.0)
        .expect("rifleman");
    let unit = entities.get_mut(rifleman).expect("rifleman should exist");
    unit.set_order(Order::attack(99));

    assert!(
        !holds_ground(unit),
        "chasing explicit attacks should not advance dig-in"
    );

    unit.mark_attack_phase(AttackPhase::Firing);

    assert!(
        holds_ground(unit),
        "in-range firing attacks should count as holding ground"
    );
}
