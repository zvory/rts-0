//! Economy rules: tech requirements, production chains, resource node amounts, cost/supply lookups.
//!
//! `rules::defs` owns kind-specific data; this module answers allowed/cost/supply questions.

use crate::defs;
use crate::EntityKind;

/// Which units a given building can train.
pub fn trainable_units(building_kind: EntityKind) -> &'static [EntityKind] {
    let units = defs::building_def(building_kind)
        .map(|d| d.trains)
        .unwrap_or(&[]);
    debug_assert!(
        defs::UNITS
            .iter()
            .all(|d| (d.trained_at == Some(building_kind)) == units.contains(&d.kind)),
        "building train list and unit trained_at should agree for {building_kind}"
    );
    units
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

/// Whether a unit's training tech has been unlocked by completed buildings.
pub fn train_requirement_met(
    unit_kind: EntityKind,
    owned_complete_building_kinds: &[EntityKind],
) -> bool {
    defs::unit_def(unit_kind)
        .map(|d| requirements_met(d.train_requires, owned_complete_building_kinds))
        .unwrap_or(true)
}

/// Resource node starting amount for a node kind.
pub fn node_amount(kind: EntityKind) -> u32 {
    defs::node_def(kind).map(|d| d.amount).unwrap_or(0)
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

/// Supply provided by a building kind. Returns 0 for non-buildings.
pub fn supply_provided(kind: EntityKind) -> u32 {
    defs::building_def(kind)
        .map(|d| d.stats.provides_supply)
        .unwrap_or(0)
}

fn requirements_met(requirements: &[EntityKind], owned: &[EntityKind]) -> bool {
    requirements.iter().all(|req| owned.contains(req))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ww2_production_chain_matches_design() {
        assert_eq!(
            trainable_units(EntityKind::CityCentre),
            &[EntityKind::Worker]
        );
        assert_eq!(
            trainable_units(EntityKind::Barracks),
            &[EntityKind::Rifleman, EntityKind::MachineGunner]
        );
        assert_eq!(
            trainable_units(EntityKind::Factory),
            &[EntityKind::ScoutCar, EntityKind::Tank]
        );
        assert_eq!(trainable_units(EntityKind::ResearchComplex), &[]);
        assert_eq!(
            trainable_units(EntityKind::Steelworks),
            &[
                EntityKind::MortarTeam,
                EntityKind::AtTeam,
                EntityKind::Artillery
            ]
        );

        assert!(train_requirement_met(EntityKind::Rifleman, &[]));
        assert!(!train_requirement_met(EntityKind::MachineGunner, &[]));
        assert!(!train_requirement_met(EntityKind::MortarTeam, &[]));
        assert!(!train_requirement_met(EntityKind::AtTeam, &[]));
        assert!(!train_requirement_met(EntityKind::Tank, &[]));
        assert!(!train_requirement_met(EntityKind::Artillery, &[]));
        assert!(train_requirement_met(
            EntityKind::MachineGunner,
            &[EntityKind::TrainingCentre]
        ));
        assert!(train_requirement_met(
            EntityKind::MortarTeam,
            &[EntityKind::Steelworks]
        ));
        assert!(!train_requirement_met(
            EntityKind::AtTeam,
            &[EntityKind::TrainingCentre]
        ));
        assert!(train_requirement_met(
            EntityKind::AtTeam,
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
        assert_eq!(cost(EntityKind::ScoutCar), (125, 50));
        assert_eq!(cost(EntityKind::Tank), (300, 150));
        assert_eq!(cost(EntityKind::CityCentre), (200, 0));
        assert_eq!(cost(EntityKind::Depot), (100, 0));
        assert_eq!(supply_cost(EntityKind::AtTeam), 3);
        assert_eq!(cost(EntityKind::Artillery), (300, 100));
        assert_eq!(cost(EntityKind::ResearchComplex), (100, 100));
        assert_eq!(supply_cost(EntityKind::Artillery), 5);
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
        assert_eq!(cost(EntityKind::Steelworks), (125, 125));
        assert_eq!(supply_cost(EntityKind::Tank), 6);
        assert_eq!(supply_cost(EntityKind::Depot), 0);
        assert_eq!(
            supply_provided(EntityKind::Depot),
            crate::balance::DEPOT_SUPPLY
        );
        assert_eq!(supply_provided(EntityKind::Tank), 0);
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
