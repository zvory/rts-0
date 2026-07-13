//! Server-shell adapter for the extracted protocol crate.
//!
//! Keeps existing `rts_server::protocol` call sites stable while wire protocol DTOs live in
//! `rts_protocol` and rules-aware kind conversion lives in the simulation adapter.

pub use rts_protocol::*;
pub use rts_sim::protocol::{kind_from_wire, kind_to_wire};

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::entity::EntityKind;

    #[test]
    fn reexported_kind_wire_adapter_round_trips_every_domain_kind() {
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
        assert_eq!(
            terrain::ROAD_BARE,
            rts_rules::terrain::MAP_TERRAIN_ROAD_BARE
        );
        assert_eq!(
            terrain::ROAD_HORIZONTAL,
            rts_rules::terrain::MAP_TERRAIN_ROAD_HORIZONTAL
        );
        assert_eq!(
            terrain::ROAD_VERTICAL,
            rts_rules::terrain::MAP_TERRAIN_ROAD_VERTICAL
        );
        assert_eq!(
            terrain::ROAD_DIAGONAL_NW_SE,
            rts_rules::terrain::MAP_TERRAIN_ROAD_DIAGONAL_NW_SE
        );
        assert_eq!(
            terrain::ROAD_DIAGONAL_NE_SW,
            rts_rules::terrain::MAP_TERRAIN_ROAD_DIAGONAL_NE_SW
        );
    }
}
