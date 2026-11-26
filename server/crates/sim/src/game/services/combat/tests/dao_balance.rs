use super::*;
use crate::game::ability::AbilityKind;

#[test]
fn dao_primary_attacks_apply_ranked_penetration_and_war_master_damage_to_tanks() {
    for (kind, master, damage) in [
        (EntityKind::Warrior, false, 14),
        (EntityKind::WindAdept, false, 10),
        (EntityKind::WindAdept, true, 18),
        (EntityKind::MagicAdept, false, 10),
        (EntityKind::MagicAdept, true, 18),
        (EntityKind::WarAdept, false, 35),
        (EntityKind::WarAdept, true, 84),
    ] {
        for double_damage in [false, true] {
            let mut entities = EntityStore::new();
            let id = entities
                .spawn_unit(1, EntityKind::Warrior, 200.0, 200.0)
                .unwrap();
            let target = entities
                .spawn_unit(2, EntityKind::Tank, 240.0, 200.0)
                .unwrap();
            let unit = entities.get_mut(id).unwrap();
            unit.set_disciple_kind_for_test(kind);
            if master {
                unit.insight = config::DISCIPLE_EVOLVED_INSIGHT_CAP;
                unit.advance_dao_master();
            }
            if double_damage {
                unit.receive_shop_item(AbilityKind::DoubleDamage);
            }
            unit.set_order(Order::attack(target));
            entities
                .get_mut(target)
                .unwrap()
                .set_order(Order::HoldPosition);
            run_combat_through_orb_impacts(&mut entities);
            // Item doubling precedes armor rounding: 46 * 0.4375 rounds to 20;
            // 56 * 0.625 rounds to 35 rather than twice the rounded 18.
            let expected = if double_damage && master && kind != EntityKind::WarAdept {
                35
            } else if double_damage && kind == EntityKind::Warrior {
                29
            } else {
                damage * if double_damage { 2 } else { 1 }
            };
            assert_eq!(
                292 - entities.get(target).unwrap().hp,
                expected,
                "{kind:?} master={master} double_damage={double_damage}"
            );
        }
    }
}
