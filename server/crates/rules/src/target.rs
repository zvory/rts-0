//! Rules-owned target classification facts.
//!
//! These facts are pure kind-level policy data. Runtime legality such as ownership, fog, smoke,
//! liveness, and shot blockers stays in the simulation because it depends on current world state.

use crate::combat::{self, TargetThreatRole};
use crate::defs::{ArmorClass, WeaponClass};
use crate::{movement_body_class, EntityKind, MovementBodyClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetFacts {
    pub kind: EntityKind,
    pub is_unit: bool,
    pub is_building: bool,
    pub is_resource_node: bool,
    pub armor_class: Option<ArmorClass>,
    pub is_armored: bool,
    pub weapon_class: WeaponClass,
    pub threat_role: TargetThreatRole,
    pub is_anti_armor_threat: bool,
    pub is_support_weapon: bool,
    pub is_field_obstacle: bool,
    pub is_vehicle_body: bool,
    pub is_economy_unit: bool,
    pub is_coax_infantry_priority: bool,
}

impl TargetFacts {
    pub fn for_kind(kind: EntityKind) -> Self {
        let threat_role = combat::target_threat_role(kind);
        Self {
            kind,
            is_unit: kind.is_unit(),
            is_building: kind.is_building(),
            is_resource_node: kind.is_node(),
            armor_class: combat::armor_class(kind),
            is_armored: combat::is_armored(kind),
            weapon_class: combat::weapon_class(kind),
            threat_role,
            is_anti_armor_threat: threat_role == TargetThreatRole::AntiArmorThreat,
            is_support_weapon: threat_role == TargetThreatRole::SupportWeapon,
            is_field_obstacle: threat_role == TargetThreatRole::FieldObstacle,
            is_vehicle_body: movement_body_class(kind) == MovementBodyClass::VehicleBody,
            is_economy_unit: is_economy_unit(kind),
            is_coax_infantry_priority: is_coax_infantry_priority(kind),
        }
    }
}

pub fn target_facts(kind: EntityKind) -> TargetFacts {
    TargetFacts::for_kind(kind)
}

/// Economy units are the current gatherer/build-worker body classes.
pub fn is_economy_unit(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::Worker | EntityKind::Golem)
}

/// Infantry that Tank coax fire ranks ahead of economy workers and fallback targets.
pub fn is_coax_infantry_priority(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman | EntityKind::Panzerfaust | EntityKind::MachineGunner
    )
}

/// Infantry-sized units that anti-tank guns cannot choose as primary targets.
///
/// This includes both factions' current economy bodies as well as Kriegsia's combat infantry.
/// Crewed support weapons, vehicles, Ekat, buildings, and other non-infantry entities stay legal.
pub fn is_anti_tank_gun_infantry_target(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Worker
            | EntityKind::Golem
            | EntityKind::Rifleman
            | EntityKind::Panzerfaust
            | EntityKind::MachineGunner
    )
}

/// Whether an attacker's default weapon is allowed to choose this kind as its primary target.
pub fn default_weapon_can_target(attacker_kind: EntityKind, target_kind: EntityKind) -> bool {
    attacker_kind != EntityKind::AntiTankGun || !is_anti_tank_gun_infantry_target(target_kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct ExpectedTargetFacts {
        is_unit: bool,
        is_building: bool,
        is_resource_node: bool,
        armor_class: Option<ArmorClass>,
        weapon_class: WeaponClass,
        threat_role: TargetThreatRole,
        is_vehicle_body: bool,
        is_economy_unit: bool,
        is_coax_infantry_priority: bool,
    }

    #[test]
    fn target_facts_cover_every_entity_kind() {
        let expected = [
            (
                EntityKind::Worker,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::SmallArms,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: true,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Golem,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::SmallArms,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: true,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Rifleman,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::SmallArms,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: true,
                },
            ),
            (
                EntityKind::Panzerfaust,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::SmallArms,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: true,
                },
            ),
            (
                EntityKind::MachineGunner,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::SmallArms,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: true,
                },
            ),
            (
                EntityKind::AntiTankGun,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::AntiTank,
                    threat_role: TargetThreatRole::AntiArmorThreat,
                    is_vehicle_body: true,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::MortarTeam,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::SmallArms,
                    threat_role: TargetThreatRole::SupportWeapon,
                    is_vehicle_body: true,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Artillery,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::SupportWeapon,
                    is_vehicle_body: true,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::ScoutCar,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::SmallArms,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: true,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::ScoutPlane,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Tank,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::AntiTank,
                    threat_role: TargetThreatRole::AntiArmorThreat,
                    is_vehicle_body: true,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::CommandCar,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: true,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Ekat,
                ExpectedTargetFacts {
                    is_unit: true,
                    is_building: false,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::CityCentre,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Zamok,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Depot,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Barracks,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::TrainingCentre,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::ResearchComplex,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Factory,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Steelworks,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::TankTrap,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Armored),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::FieldObstacle,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::PumpJack,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: true,
                    is_resource_node: false,
                    armor_class: Some(ArmorClass::Small),
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Steel,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: false,
                    is_resource_node: true,
                    armor_class: None,
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
            (
                EntityKind::Oil,
                ExpectedTargetFacts {
                    is_unit: false,
                    is_building: false,
                    is_resource_node: true,
                    armor_class: None,
                    weapon_class: WeaponClass::None,
                    threat_role: TargetThreatRole::Ordinary,
                    is_vehicle_body: false,
                    is_economy_unit: false,
                    is_coax_infantry_priority: false,
                },
            ),
        ];

        assert_eq!(EntityKind::ALL.len(), expected.len());
        for (kind, expected) in expected {
            let facts = target_facts(kind);
            assert_eq!(facts.kind, kind);
            assert_eq!(facts.is_unit, expected.is_unit, "{kind} unit fact");
            assert_eq!(
                facts.is_building, expected.is_building,
                "{kind} building fact"
            );
            assert_eq!(
                facts.is_resource_node, expected.is_resource_node,
                "{kind} resource-node fact"
            );
            assert_eq!(
                facts.armor_class, expected.armor_class,
                "{kind} armor class"
            );
            assert_eq!(
                facts.is_armored,
                expected.armor_class == Some(ArmorClass::Armored),
                "{kind} armored fact"
            );
            assert_eq!(
                facts.weapon_class, expected.weapon_class,
                "{kind} weapon class"
            );
            assert_eq!(
                facts.threat_role, expected.threat_role,
                "{kind} threat role"
            );
            assert_eq!(
                facts.is_anti_armor_threat,
                expected.threat_role == TargetThreatRole::AntiArmorThreat,
                "{kind} anti-armor threat fact"
            );
            assert_eq!(
                facts.is_support_weapon,
                expected.threat_role == TargetThreatRole::SupportWeapon,
                "{kind} support weapon fact"
            );
            assert_eq!(
                facts.is_field_obstacle,
                expected.threat_role == TargetThreatRole::FieldObstacle,
                "{kind} field obstacle fact"
            );
            assert_eq!(
                facts.is_vehicle_body, expected.is_vehicle_body,
                "{kind} vehicle-body fact"
            );
            assert_eq!(
                facts.is_economy_unit, expected.is_economy_unit,
                "{kind} economy-unit fact"
            );
            assert_eq!(
                facts.is_coax_infantry_priority, expected.is_coax_infantry_priority,
                "{kind} coax infantry-priority fact"
            );
        }
    }

    #[test]
    fn anti_tank_gun_primary_target_policy_excludes_only_infantry() {
        let infantry = [
            EntityKind::Worker,
            EntityKind::Golem,
            EntityKind::Rifleman,
            EntityKind::Panzerfaust,
            EntityKind::MachineGunner,
        ];

        for kind in EntityKind::ALL {
            assert_eq!(
                is_anti_tank_gun_infantry_target(kind),
                infantry.contains(&kind),
                "{kind} infantry classification"
            );
            assert_eq!(
                default_weapon_can_target(EntityKind::AntiTankGun, kind),
                !infantry.contains(&kind),
                "anti-tank gun primary-target policy for {kind}"
            );
            assert!(
                default_weapon_can_target(EntityKind::Tank, kind),
                "the infantry exclusion must be specific to anti-tank guns"
            );
        }
    }
}
