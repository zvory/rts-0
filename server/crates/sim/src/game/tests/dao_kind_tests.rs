use super::*;
use crate::game::ability::AbilityKind;

#[test]
fn dao_kinds_spawn_with_resolved_stats_and_serialize_without_mutable_dao() {
    for (kind, ability, hp, cooldown, range) in [
        (EntityKind::WindAdept, AbilityKind::WindDash, 150, 61, 0.75),
        (EntityKind::WarAdept, AbilityKind::ThrowSword, 176, 91, 1.75),
        (
            EntityKind::MagicAdept,
            AbilityKind::DiscipleShield,
            150,
            91,
            5.0,
        ),
    ] {
        let unit = Entity::new_unit(1, kind, 200.0, 200.0).unwrap();
        assert_eq!((unit.hp, unit.max_hp), (hp, hp));
        assert!(unit.dao_ability_available(ability));
        assert!(!unit.dao_ability_available(AbilityKind::EvolveWar));
        assert_eq!(combat::attack_profile(kind).cooldown, cooldown);
        assert_eq!(combat::attack_profile(kind).range_tiles, range);
        assert_eq!(crate::rules::economy::supply_cost(kind), 3);
        assert!(rts_rules::barbed_wire_affects(kind));
        assert!(crate::rules::target::default_weapon_can_target(
            EntityKind::AntiTankGun,
            kind
        ));
        let mut value = serde_json::to_value(&unit).unwrap();
        assert!(value.get("dao").is_none());
        let restored: Entity = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(restored.kind, kind);
        assert_eq!(restored.dao(), unit.dao());
        value["dao"] = serde_json::json!("DaoOfMagic");
        assert!(
            serde_json::from_value::<Entity>(value).is_err(),
            "obsolete duplicate identity must be rejected"
        );
    }
}
