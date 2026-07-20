//! Simulation-facing protocol DTOs and typed entity-kind conversion helpers.

pub use rts_protocol::*;
use rts_rules::EntityKind;

pub fn kind_to_wire(kind: EntityKind) -> &'static str {
    kind.stable_id()
}

pub fn kind_from_wire(kind: &str) -> Option<EntityKind> {
    kind.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_rules::faction::{AbilityKind, UpgradeKind};

    const PROTOCOL_KIND_IDS: [&str; 25] = [
        kinds::WORKER,
        kinds::GOLEM,
        kinds::RIFLEMAN,
        kinds::PANZERFAUST,
        kinds::MACHINE_GUNNER,
        kinds::ANTI_TANK_GUN,
        kinds::MORTAR_TEAM,
        kinds::ARTILLERY,
        kinds::SCOUT_CAR,
        kinds::SCOUT_PLANE,
        kinds::TANK,
        kinds::COMMAND_CAR,
        kinds::EKAT,
        kinds::CITY_CENTRE,
        kinds::ZAMOK,
        kinds::DEPOT,
        kinds::BARRACKS,
        kinds::TRAINING_CENTRE,
        kinds::RESEARCH_COMPLEX,
        kinds::FACTORY,
        kinds::STEELWORKS,
        kinds::TANK_TRAP,
        kinds::PUMP_JACK,
        kinds::STEEL,
        kinds::OIL,
    ];

    #[test]
    fn entity_kind_wire_ids_match_protocol_constants() {
        for kind in EntityKind::ALL {
            let wire = kind_to_wire(kind);
            assert_eq!(kind_from_wire(wire), Some(kind));
            assert_eq!(wire.parse::<EntityKind>(), Ok(kind));
            assert!(
                PROTOCOL_KIND_IDS.contains(&wire),
                "{wire} is missing from rts_protocol::kinds"
            );
        }
    }

    #[test]
    fn protocol_kind_constants_round_trip_to_rules_domain_ids() {
        for wire in PROTOCOL_KIND_IDS {
            let kind = kind_from_wire(wire).expect("protocol kind parses as EntityKind");
            assert_eq!(kind_to_wire(kind), wire);
        }
    }

    #[test]
    fn protocol_kind_constants_are_unique() {
        for (index, wire) in PROTOCOL_KIND_IDS.iter().enumerate() {
            assert!(
                !PROTOCOL_KIND_IDS[..index].contains(wire),
                "duplicate protocol kind constant {wire}"
            );
        }
    }

    #[test]
    fn rules_ability_ids_match_protocol_vocabulary_and_compact_codes() {
        let rule_ids = AbilityKind::ALL
            .iter()
            .map(|kind| kind.stable_id())
            .collect::<Vec<_>>();
        assert_eq!(rule_ids.len(), abilities::ALL.len());
        for id in rule_ids {
            assert!(
                abilities::ALL.contains(&id),
                "protocol is missing ability {id}"
            );
            assert_ne!(ability_code(id), COMPACT_UNKNOWN_CODE);
        }
    }

    #[test]
    fn active_rule_upgrade_ids_match_protocol_codes() {
        let rule_ids = UpgradeKind::ALL
            .iter()
            .map(|kind| kind.stable_id())
            .collect::<Vec<_>>();
        assert_eq!(rule_ids.len(), upgrades::ALL.len());
        for id in rule_ids {
            assert!(
                upgrades::ALL.contains(&id),
                "protocol is missing upgrade {id}"
            );
            assert_ne!(upgrade_code(id), COMPACT_UNKNOWN_CODE);
        }
        assert!(UpgradeKind::ALL.contains(&UpgradeKind::BallisticTables));
        assert!(upgrades::ALL.contains(&upgrades::BALLISTIC_TABLES));
        assert_ne!(
            upgrade_code(upgrades::BALLISTIC_TABLES),
            COMPACT_UNKNOWN_CODE
        );
    }
}
