//! Economy rules: tech requirements, production chains, resource node amounts, cost/supply lookups.
//!
//! `config.rs` answers "what number?"; this module answers "is this allowed / what does it cost?".

use crate::config;
use crate::game::entity::EntityKind;

/// Which units a given building can train.
pub fn trainable_units(building_kind: EntityKind) -> &'static [EntityKind] {
    match building_kind {
        EntityKind::IndustrialCenter => &[EntityKind::Worker],
        EntityKind::Barracks => &[
            EntityKind::Rifleman,
            EntityKind::MachineGunner,
            EntityKind::AtTeam,
        ],
        EntityKind::TankFactory => &[EntityKind::Tank],
        _ => &[],
    }
}

/// Whether `building_kind` is allowed to be placed given the set of building kinds the
/// player already owns (tech requirements).
pub fn build_requirement_met(
    building_kind: EntityKind,
    owned_building_kinds: &[EntityKind],
) -> bool {
    match building_kind {
        EntityKind::Barracks | EntityKind::TrainingCentre => {
            owned_building_kinds.contains(&EntityKind::IndustrialCenter)
        }
        EntityKind::TankFactory => {
            owned_building_kinds.contains(&EntityKind::IndustrialCenter)
                && owned_building_kinds.contains(&EntityKind::TrainingCentre)
        }
        _ => true,
    }
}

/// Whether a unit's training tech has been unlocked by completed buildings.
pub fn train_requirement_met(
    unit_kind: EntityKind,
    owned_complete_building_kinds: &[EntityKind],
) -> bool {
    match unit_kind {
        EntityKind::MachineGunner | EntityKind::AtTeam => {
            owned_complete_building_kinds.contains(&EntityKind::TrainingCentre)
        }
        _ => true,
    }
}

/// Resource node starting amount for a node kind.
pub fn node_amount(kind: EntityKind) -> u32 {
    match kind {
        EntityKind::Steel => config::STEEL_PATCH_AMOUNT,
        EntityKind::Oil => config::OIL_GEYSER_AMOUNT,
        _ => 0,
    }
}

/// Cost of a unit or building kind as `(steel, oil)`. Returns `(0, 0)` for unknown kinds.
pub fn cost(kind: EntityKind) -> (u32, u32) {
    if let Some(s) = config::unit_stats(kind) {
        (s.cost_steel, s.cost_oil)
    } else if let Some(s) = config::building_stats(kind) {
        (s.cost_steel, s.cost_oil)
    } else {
        (0, 0)
    }
}

/// Supply consumed by a unit kind. Returns 0 for non-units.
pub fn supply_cost(kind: EntityKind) -> u32 {
    config::unit_stats(kind).map(|s| s.supply).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ww2_production_chain_matches_design() {
        assert_eq!(
            trainable_units(EntityKind::IndustrialCenter),
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
        assert_eq!(
            trainable_units(EntityKind::TankFactory),
            &[EntityKind::Tank]
        );

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

        assert!(!build_requirement_met(EntityKind::TankFactory, &[]));
        assert!(!build_requirement_met(
            EntityKind::TankFactory,
            &[EntityKind::IndustrialCenter]
        ));
        assert!(!build_requirement_met(
            EntityKind::TankFactory,
            &[EntityKind::TrainingCentre]
        ));
        assert!(build_requirement_met(
            EntityKind::TankFactory,
            &[EntityKind::IndustrialCenter, EntityKind::TrainingCentre]
        ));
    }
}
