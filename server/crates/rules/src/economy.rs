//! Economy rules: tech requirements, production chains, resource node amounts, cost/supply lookups.
//!
//! `rules::defs` owns kind-specific data; this module answers allowed/cost/supply questions.

use crate::defs;
use crate::faction::catalog_for;
use crate::EntityKind;

/// The faction plan intentionally keeps resource costs shaped as fixed Steel/Oil fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceCost {
    pub steel: u32,
    pub oil: u32,
}

impl ResourceCost {
    pub const fn new(steel: u32, oil: u32) -> Self {
        Self { steel, oil }
    }
}

/// Which units a given building can train.
pub fn trainable_units(building_kind: EntityKind) -> &'static [EntityKind] {
    let units = defs::building_def(building_kind)
        .map(|d| d.trains)
        .unwrap_or(&[]);
    debug_assert!(
        defs::UNITS.iter().all(|d| {
            let listed = units.contains(&d.kind);
            if d.trained_at == Some(building_kind) {
                listed
            } else {
                !listed
            }
        }),
        "building train list and unit trained_at should agree for {building_kind}"
    );
    units
}

/// Which units a given building can train for a specific faction.
pub fn trainable_units_for_faction(faction_id: &str, building_kind: EntityKind) -> Vec<EntityKind> {
    catalog_for(faction_id)
        .map(|catalog| catalog.trainable_units(building_kind))
        .unwrap_or_default()
}

/// Whether `building_kind` is allowed to be placed given the set of building kinds the
/// player already owns (tech requirements).
pub fn build_requirement_met(
    building_kind: EntityKind,
    owned_building_kinds: &[EntityKind],
) -> bool {
    defs::building_def(building_kind)
        .map(|d| requirements_met(d.build_requires, owned_building_kinds))
        .unwrap_or(true)
}

/// Whether `building_kind` is in this faction's catalog and its tech requirements are met.
pub fn build_requirement_met_for_faction(
    faction_id: &str,
    building_kind: EntityKind,
    owned_building_kinds: &[EntityKind],
) -> bool {
    catalog_for(faction_id).is_some_and(|catalog| {
        catalog.allows_building(building_kind)
            && build_requirement_met(building_kind, owned_building_kinds)
    })
}

/// Whether a unit's training tech has been unlocked by completed buildings.
pub fn train_requirement_met(
    unit_kind: EntityKind,
    owned_complete_building_kinds: &[EntityKind],
) -> bool {
    defs::unit_def(unit_kind)
        .map(|d| d.train_requirement.is_met(owned_complete_building_kinds))
        .unwrap_or(true)
}

/// Whether `unit_kind` is in this faction's catalog and its training tech has been unlocked.
pub fn train_requirement_met_for_faction(
    faction_id: &str,
    unit_kind: EntityKind,
    owned_complete_building_kinds: &[EntityKind],
) -> bool {
    catalog_for(faction_id).is_some_and(|catalog| {
        catalog.allows_unit(unit_kind)
            && train_requirement_met(unit_kind, owned_complete_building_kinds)
    })
}

/// Whether `builder_kind` can place `building_kind` for this faction.
pub fn can_build_for_faction(
    faction_id: &str,
    builder_kind: EntityKind,
    building_kind: EntityKind,
) -> bool {
    catalog_for(faction_id).is_some_and(|catalog| {
        catalog.can_build(builder_kind, building_kind)
            || (builder_kind == EntityKind::Worker
                && building_kind == EntityKind::PumpJack
                && catalog.allows_building(EntityKind::PumpJack))
    })
}

/// Whether `unit_kind` can gather resources for this faction.
pub fn can_gather_for_faction(faction_id: &str, unit_kind: EntityKind) -> bool {
    catalog_for(faction_id).is_some_and(|catalog| catalog.can_gather(unit_kind))
}

/// Whether a completed building accepts production/rally commands for this faction.
pub fn can_act_as_production_anchor_for_faction(
    faction_id: &str,
    building_kind: EntityKind,
) -> bool {
    catalog_for(faction_id)
        .is_some_and(|catalog| catalog.can_act_as_production_anchor(building_kind))
}

/// Whether a building may research this upgrade for this faction.
pub fn can_research_for_faction(
    faction_id: &str,
    upgrade_id: &str,
    building_kind: EntityKind,
) -> bool {
    catalog_for(faction_id)
        .is_some_and(|catalog| catalog.allows_research(upgrade_id, building_kind))
}

/// Resource node starting amount for a node kind.
pub fn node_amount(kind: EntityKind) -> u32 {
    defs::node_def(kind).map(|d| d.amount).unwrap_or(0)
}

/// Cost of a unit or building kind as fixed Steel/Oil fields.
pub fn resource_cost(kind: EntityKind) -> ResourceCost {
    let (steel, oil) = cost(kind);
    ResourceCost::new(steel, oil)
}

/// Cost of a unit or building kind as `(steel, oil)`. Returns `(0, 0)` for unknown kinds.
pub fn cost(kind: EntityKind) -> (u32, u32) {
    if let Some(s) = defs::unit_def(kind).map(|d| d.stats) {
        (s.cost_steel, s.cost_oil)
    } else if let Some(s) = defs::building_def(kind).map(|d| d.stats) {
        (s.cost_steel, s.cost_oil)
    } else {
        (0, 0)
    }
}

/// Human-readable notice for a fixed Steel/Oil resource shortage.
pub fn resource_shortage_notice_for_cost(steel: u32, oil: u32, cost: ResourceCost) -> &'static str {
    resource_shortage_notice(steel, oil, cost.steel, cost.oil)
}

/// Human-readable notice for a resource shortage. Oil is reported first because
/// mixed steel/oil units are usually blocked by oil in practice.
pub fn resource_shortage_notice(
    steel: u32,
    oil: u32,
    cost_steel: u32,
    cost_oil: u32,
) -> &'static str {
    if oil < cost_oil {
        "Not enough oil"
    } else if steel < cost_steel {
        "Not enough steel"
    } else {
        "Not enough resources"
    }
}

/// Supply consumed by a unit kind. Returns 0 for non-units.
pub fn supply_cost(kind: EntityKind) -> u32 {
    defs::unit_def(kind).map(|d| d.stats.supply).unwrap_or(0)
}

fn requirements_met(requirements: &[EntityKind], owned: &[EntityKind]) -> bool {
    requirements.iter().all(|req| owned.contains(req))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faction::DEFAULT_FACTION_ID;

    #[test]
    fn ww2_production_chain_matches_design() {
        assert_eq!(DEFAULT_FACTION_ID, "kriegsia");
        assert_eq!(
            trainable_units(EntityKind::CityCentre),
            &[EntityKind::Worker]
        );
        assert_eq!(trainable_units(EntityKind::Zamok), &[EntityKind::Golem]);
        assert_eq!(
            trainable_units(EntityKind::Barracks),
            &[EntityKind::Rifleman, EntityKind::MachineGunner]
        );
        assert_eq!(
            trainable_units(EntityKind::Factory),
            &[
                EntityKind::ScoutCar,
                EntityKind::Tank,
                EntityKind::CommandCar
            ]
        );
        assert_eq!(trainable_units(EntityKind::ResearchComplex), &[]);
        assert_eq!(
            trainable_units(EntityKind::Steelworks),
            &[
                EntityKind::MortarTeam,
                EntityKind::AntiTankGun,
                EntityKind::Artillery
            ]
        );

        assert!(train_requirement_met(EntityKind::Rifleman, &[]));
        assert!(!train_requirement_met(EntityKind::MachineGunner, &[]));
        assert!(!train_requirement_met(EntityKind::MortarTeam, &[]));
        assert!(!train_requirement_met(EntityKind::AntiTankGun, &[]));
        assert!(!train_requirement_met(EntityKind::Tank, &[]));
        assert!(!train_requirement_met(EntityKind::Artillery, &[]));
        assert!(
            train_requirement_met(EntityKind::ScoutPlane, &[]),
            "Scout Plane has no train prerequisite because it is launched by ability, not trained"
        );
        assert!(train_requirement_met(
            EntityKind::MachineGunner,
            &[EntityKind::TrainingCentre]
        ));
        assert!(train_requirement_met(
            EntityKind::MortarTeam,
            &[EntityKind::Steelworks]
        ));
        assert!(!train_requirement_met(
            EntityKind::AntiTankGun,
            &[EntityKind::TrainingCentre]
        ));
        assert!(train_requirement_met(
            EntityKind::AntiTankGun,
            &[EntityKind::Steelworks]
        ));
        assert!(train_requirement_met(
            EntityKind::Artillery,
            &[EntityKind::Steelworks]
        ));
        assert!(!train_requirement_met(
            EntityKind::Tank,
            &[EntityKind::Steelworks]
        ));
        assert!(train_requirement_met(
            EntityKind::Tank,
            &[EntityKind::Factory]
        ));
        assert_eq!(
            defs::unit_def(EntityKind::ScoutPlane).and_then(|d| d.trained_at),
            None,
            "Scout Plane is not exposed through any production building"
        );

        assert!(!build_requirement_met(EntityKind::TrainingCentre, &[]));
        assert!(!build_requirement_met(
            EntityKind::TrainingCentre,
            &[EntityKind::CityCentre]
        ));
        assert!(!build_requirement_met(
            EntityKind::TrainingCentre,
            &[EntityKind::Barracks]
        ));
        assert!(build_requirement_met(
            EntityKind::TrainingCentre,
            &[EntityKind::CityCentre, EntityKind::Barracks]
        ));

        assert!(!build_requirement_met(EntityKind::Factory, &[]));
        assert!(!build_requirement_met(
            EntityKind::Factory,
            &[EntityKind::CityCentre]
        ));
        assert!(!build_requirement_met(
            EntityKind::Factory,
            &[EntityKind::TrainingCentre]
        ));
        assert!(build_requirement_met(
            EntityKind::Factory,
            &[EntityKind::CityCentre, EntityKind::TrainingCentre]
        ));
        assert!(!build_requirement_met(EntityKind::ResearchComplex, &[]));
        assert!(!build_requirement_met(
            EntityKind::ResearchComplex,
            &[EntityKind::TrainingCentre]
        ));
        assert!(build_requirement_met(
            EntityKind::ResearchComplex,
            &[EntityKind::CityCentre, EntityKind::TrainingCentre]
        ));
        assert!(!build_requirement_met(EntityKind::Steelworks, &[]));
        assert!(build_requirement_met(
            EntityKind::Steelworks,
            &[EntityKind::CityCentre, EntityKind::TrainingCentre]
        ));

        assert_eq!(cost(EntityKind::Worker), (50, 0));
        assert_eq!(cost(EntityKind::Golem), (0, 0));
        assert_eq!(cost(EntityKind::ScoutCar), (125, 50));
        assert_eq!(cost(EntityKind::ScoutPlane), (50, 75));
        assert_eq!(cost(EntityKind::Tank), (425, 150));
        assert_eq!(cost(EntityKind::MortarTeam), (100, 50));
        assert_eq!(cost(EntityKind::Factory), (125, 125));
        assert_eq!(cost(EntityKind::CityCentre), (450, 150));
        assert_eq!(cost(EntityKind::Depot), (100, 0));
        assert_eq!(supply_cost(EntityKind::AntiTankGun), 3);
        assert_eq!(cost(EntityKind::Artillery), (300, 100));
        assert_eq!(cost(EntityKind::ResearchComplex), (100, 100));
        assert_eq!(supply_cost(EntityKind::Artillery), 5);
        assert_eq!(supply_cost(EntityKind::ScoutPlane), 0);
        assert_eq!(
            defs::unit_def(EntityKind::ScoutPlane).map(|d| d.stats.radius),
            Some(0.0),
            "Scout Plane should not reserve or block ground collision"
        );
        assert_eq!(
            defs::unit_def(EntityKind::ScoutPlane).map(|d| d.stats.sight_tiles),
            Some(crate::balance::SCOUT_PLANE_SIGHT_TILES)
        );
        assert_eq!(
            defs::unit_def(EntityKind::Artillery).map(|d| d.stats.radius),
            defs::unit_def(EntityKind::Tank).map(|d| d.stats.radius),
            "artillery should use the same selection/collision radius as tanks"
        );
        assert_eq!(
            crate::balance::ARTILLERY_BODY_LENGTH_PX,
            crate::balance::TANK_BODY_LENGTH_PX,
            "artillery body length should match tanks"
        );
        assert_eq!(
            crate::balance::ARTILLERY_BODY_WIDTH_PX,
            crate::balance::TANK_BODY_WIDTH_PX,
            "artillery body width should match tanks"
        );
        assert_eq!(supply_cost(EntityKind::ScoutCar), 3);
        assert_eq!(supply_cost(EntityKind::MortarTeam), 3);
        assert_eq!(supply_cost(EntityKind::Golem), 4);
        assert_eq!(cost(EntityKind::Steelworks), (150, 100));
        assert_eq!(supply_cost(EntityKind::Tank), 8);
        assert_eq!(supply_cost(EntityKind::Depot), 0);
        assert_eq!(
            trainable_units_for_faction(DEFAULT_FACTION_ID, EntityKind::Factory),
            vec![
                EntityKind::ScoutCar,
                EntityKind::Tank,
                EntityKind::CommandCar
            ]
        );
        assert!(
            can_build_for_faction(DEFAULT_FACTION_ID, EntityKind::Worker, EntityKind::TankTrap),
            "default workers can build Tank Traps"
        );
        assert!(
            can_build_for_faction(DEFAULT_FACTION_ID, EntityKind::Worker, EntityKind::PumpJack),
            "default workers can build contextual Pump Jacks on oil nodes"
        );
        assert!(
            !can_build_for_faction(
                DEFAULT_FACTION_ID,
                EntityKind::Rifleman,
                EntityKind::PumpJack
            ),
            "non-workers cannot build contextual Pump Jacks"
        );
        assert!(build_requirement_met_for_faction(
            DEFAULT_FACTION_ID,
            EntityKind::Factory,
            &[EntityKind::CityCentre, EntityKind::TrainingCentre]
        ));
        assert!(train_requirement_met_for_faction(
            DEFAULT_FACTION_ID,
            EntityKind::Tank,
            &[EntityKind::Factory]
        ));
        assert_eq!(
            trainable_units_for_faction(crate::faction::EKAT_FACTION_ID, EntityKind::Zamok),
            vec![EntityKind::Golem]
        );
        assert!(can_gather_for_faction(
            crate::faction::EKAT_FACTION_ID,
            EntityKind::Golem
        ));
    }

    #[test]
    fn unknown_faction_economy_queries_fail_closed() {
        let faction_id = "unknown_faction";

        assert!(trainable_units_for_faction(faction_id, EntityKind::CityCentre).is_empty());
        assert!(!build_requirement_met_for_faction(
            faction_id,
            EntityKind::Depot,
            &[EntityKind::CityCentre]
        ));
        assert!(!train_requirement_met_for_faction(
            faction_id,
            EntityKind::Worker,
            &[EntityKind::CityCentre]
        ));
        assert!(!can_build_for_faction(
            faction_id,
            EntityKind::Worker,
            EntityKind::Depot
        ));
        assert!(!can_gather_for_faction(faction_id, EntityKind::Worker));
        assert!(!can_act_as_production_anchor_for_faction(
            faction_id,
            EntityKind::CityCentre
        ));
        assert!(!can_research_for_faction(
            faction_id,
            crate::faction::TANK_UNLOCK_UPGRADE,
            EntityKind::ResearchComplex
        ));
    }

    #[test]
    fn resource_shortage_notice_prefers_specific_voice_lines() {
        assert_eq!(resource_shortage_notice(25, 100, 50, 0), "Not enough steel");
        assert_eq!(resource_shortage_notice(100, 5, 50, 25), "Not enough oil");
        assert_eq!(
            resource_shortage_notice(0, 0, 50, 25),
            "Not enough oil",
            "oil should win when both resources are missing"
        );
        assert_eq!(
            resource_shortage_notice(50, 25, 50, 25),
            "Not enough resources",
            "fallback should only be used when there is no shortage"
        );
    }
}
