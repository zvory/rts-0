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

    const PROTOCOL_KIND_IDS: [&str; 24] = [
        kinds::WORKER,
        kinds::GOLEM,
        kinds::RIFLEMAN,
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
}
