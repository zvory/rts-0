//! Economy rules: tech requirements, production chains, resource node amounts, cost/supply lookups.
//!
//! `rules::defs` owns kind-specific data; this module answers allowed/cost/supply questions.

use crate::game::entity::EntityKind;
use crate::rules::defs;

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
            &[
                EntityKind::Rifleman,
                EntityKind::MachineGunner,
                EntityKind::AtTeam
            ]
        );
        assert_eq!(trainable_units(EntityKind::Factory), &[EntityKind::Tank]);

        assert!(train_requirement_met(EntityKind::Rifleman, &[]));
        assert!(!train_requirement_met(EntityKind::MachineGunner, &[]));
        assert!(!train_requirement_met(EntityKind::AtTeam, &[]));
        assert!(train_requirement_met(
            EntityKind::MachineGunner,
            &[EntityKind::TrainingCentre]
        ));
        assert!(train_requirement_met(
            EntityKind::AtTeam,
            &[EntityKind::TrainingCentre]
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

        assert_eq!(cost(EntityKind::Worker), (50, 0));
        assert_eq!(cost(EntityKind::Tank), (200, 150));
        assert_eq!(cost(EntityKind::CityCentre), (200, 0));
        assert_eq!(cost(EntityKind::Depot), (100, 0));
        assert_eq!(supply_cost(EntityKind::Tank), 6);
        assert_eq!(supply_cost(EntityKind::Depot), 0);
        assert_eq!(
            supply_provided(EntityKind::Depot),
            crate::config::DEPOT_SUPPLY
        );
        assert_eq!(supply_provided(EntityKind::Tank), 0);
    }
}
