use super::*;
#[test]
fn additive_armor_is_capped_before_penetration_and_rounded_once() {
    for (kind, bonus, penetration, expected) in [
        (EntityKind::WindAdept, 0.5, 0.0, 50),
        (EntityKind::WarAdept, 1.0, 0.0, 0),
        (EntityKind::WarAdept, 1.0, 0.5, 50),
        (EntityKind::Tank, 0.5, 0.0, 0),
        (EntityKind::Tank, 0.5, 0.5, 50),
        (EntityKind::Tank, 0.5, 1.0, 100),
    ] {
        assert_eq!(
            effective_damage_for_armor_penetration_with_bonus(penetration, kind, 100, None, bonus),
            expected
        );
    }
    assert_eq!(
        effective_damage_for_armor_penetration_with_bonus(0.0, EntityKind::WindAdept, 3, None, 0.5),
        2
    );
    // Preserve the existing integer-quarter native-armor path when no bonus exists.
    assert_eq!(
        effective_damage_for_armor_penetration(0.0, EntityKind::Tank, 3, None),
        0
    );
}
