use super::*;
use std::collections::BTreeSet;

#[test]
fn unit_reload_has_all_four_offsets_across_units_and_repeated_attacks() {
    for base in [10, 12, 24, 32, 61, 91, 180, 900] {
        let sample = |id, tick| {
            randomized_attack_reload(EntityKind::Rifleman, id, "rifleman_rifle", tick, base)
        };
        let expected = BTreeSet::from([base - 1, base, base + 1, base + 2]);
        assert_eq!(
            (1..=128).map(|id| sample(id, 100)).collect::<BTreeSet<_>>(),
            expected
        );
        assert_eq!(
            (1..=128)
                .map(|tick| sample(1, tick))
                .collect::<BTreeSet<_>>(),
            expected
        );
    }
}

#[test]
fn excluded_attackers_and_zero_reload_are_unchanged_and_arithmetic_is_bounded() {
    for tick in 0..128 {
        for kind in [EntityKind::Ekat, EntityKind::ResourceDepot] {
            assert_eq!(randomized_attack_reload(kind, 1, "attack", tick, 32), 32);
        }
        assert_eq!(
            randomized_attack_reload(EntityKind::Rifleman, 1, "attack", tick, 0),
            0
        );
        assert!((1..=3).contains(&randomized_attack_reload(
            EntityKind::Rifleman,
            1,
            "attack",
            tick,
            1
        )));
        assert!(
            randomized_attack_reload(EntityKind::Rifleman, u32::MAX, "attack", tick, u32::MAX)
                >= u32::MAX - 1
        );
    }
}

#[test]
fn independent_weapons_do_not_share_a_fixed_offset() {
    assert!((0..128).any(|tick| {
        randomized_attack_reload(EntityKind::Tank, 1, "tank_cannon", tick, 12)
            != randomized_attack_reload(EntityKind::Tank, 1, "tank_coax", tick, 12)
    }));
}
