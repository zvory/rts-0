//! Server-shell adapter for the extracted protocol crate.
//!
//! Phase 1 keeps existing `rts_server::protocol` call sites stable while the wire protocol and
//! semantic DTOs live in narrower crates.

pub use rts_protocol::*;

use crate::game::entity::EntityKind;

/// Convert domain entity vocabulary to the current wire string vocabulary.
pub fn kind_to_wire(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Worker => kinds::WORKER,
        EntityKind::Rifleman => kinds::RIFLEMAN,
        EntityKind::MachineGunner => kinds::MACHINE_GUNNER,
        EntityKind::AtTeam => kinds::AT_TEAM,
        EntityKind::MortarTeam => kinds::MORTAR_TEAM,
        EntityKind::ScoutCar => kinds::SCOUT_CAR,
        EntityKind::Tank => kinds::TANK,
        EntityKind::CityCentre => kinds::CITY_CENTRE,
        EntityKind::Depot => kinds::DEPOT,
        EntityKind::Barracks => kinds::BARRACKS,
        EntityKind::TrainingCentre => kinds::TRAINING_CENTRE,
        EntityKind::Factory => kinds::FACTORY,
        EntityKind::Steelworks => kinds::STEELWORKS,
        EntityKind::Steel => kinds::STEEL,
        EntityKind::Oil => kinds::OIL,
    }
}

pub fn kind_from_wire(kind: &str) -> Option<EntityKind> {
    match kind {
        kinds::WORKER => Some(EntityKind::Worker),
        kinds::RIFLEMAN => Some(EntityKind::Rifleman),
        kinds::MACHINE_GUNNER => Some(EntityKind::MachineGunner),
        kinds::AT_TEAM => Some(EntityKind::AtTeam),
        kinds::MORTAR_TEAM => Some(EntityKind::MortarTeam),
        kinds::SCOUT_CAR => Some(EntityKind::ScoutCar),
        kinds::TANK => Some(EntityKind::Tank),
        kinds::CITY_CENTRE => Some(EntityKind::CityCentre),
        kinds::DEPOT => Some(EntityKind::Depot),
        kinds::BARRACKS => Some(EntityKind::Barracks),
        kinds::TRAINING_CENTRE => Some(EntityKind::TrainingCentre),
        kinds::FACTORY => Some(EntityKind::Factory),
        kinds::STEELWORKS => Some(EntityKind::Steelworks),
        kinds::STEEL => Some(EntityKind::Steel),
        kinds::OIL => Some(EntityKind::Oil),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_wire_adapter_round_trips_every_domain_kind() {
        for kind in EntityKind::ALL {
            let wire = kind_to_wire(kind);
            assert_eq!(kind_from_wire(wire), Some(kind));
            assert_eq!(wire.parse::<EntityKind>(), Ok(kind));
        }
    }

    #[test]
    fn terrain_wire_codes_match_rules_domain_codes() {
        assert_eq!(terrain::GRASS, rts_rules::terrain::MAP_TERRAIN_GRASS);
        assert_eq!(terrain::ROCK, rts_rules::terrain::MAP_TERRAIN_ROCK);
        assert_eq!(terrain::WATER, rts_rules::terrain::MAP_TERRAIN_WATER);
    }
}
