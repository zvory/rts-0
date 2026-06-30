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

    let occupied_trench_counts = build_occupied_trench_counts(&entities);
    let candidate = best_occupation_candidate(
        &map,
        &entities,
        &occ,
        &trenches,
        &occupied_trench_counts,
        &snapshot,
    )
    .expect("farther trench should remain occupiable");

    assert_eq!(candidate.trench_id, farther);
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
