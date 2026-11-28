// replayResourceHistory also carries players[] with id, lifetimeSteel/lifetimeOil and absolute steel/oil, aliveSteel/aliveOil, lostSteel/lostOil, unitSteel/unitOil values for up to four players.
// replayResourceHistory samples carry signed steel/oil trailing-five-second collection totals (client displays totals / 5 per second) and aliveSteel/aliveOil lifetime-minus-loss advantages, lostSteel/lostOil destroyed-value differences, and unitSteel/unitOil current-army-value differences.
//! Lobby color preferences use the shared SetPreferredColor DTO.
//! Server-shell adapter for the extracted protocol crate.
//!
//! Keeps existing `rts_server::protocol` call sites stable while wire protocol DTOs live in
//! `rts_protocol` (including public Rocket Truck rack counts) and rules-aware kind conversion lives in the simulation adapter.

// Escape protection status is projected only to owner/allies and privileged observers.
// ClientNetReport includes the shared performance-summary wire DTO.
pub use rts_protocol::*;
pub use rts_sim::protocol::{kind_from_wire, kind_to_wire};

/// Stamp reliable connection messages with the current server build for the client handshake.
pub fn welcome_message(player_id: u32) -> ServerMessage {
    ServerMessage::Welcome {
        player_id,
        build_id: crate::build_info::build_id().to_string(),
    }
}

pub fn pong_message(ts: f64) -> ServerMessage {
    ServerMessage::Pong {
        ts,
        build_id: crate::build_info::build_id().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::game::entity::EntityKind;

    #[test]
    fn version_handshake_serializes_through_server_adapter() {
        for (message, expected) in [
            (
                welcome_message(7),
                serde_json::json!({"t":"welcome", "playerId":7, "buildId":crate::build_info::build_id()}),
            ),
            (
                pong_message(42.0),
                serde_json::json!({"t":"pong", "ts":42.0, "buildId":crate::build_info::build_id()}),
            ),
        ] {
            assert_eq!(serde_json::to_value(message).unwrap(), expected);
        }
    }

    #[test]
    fn room_notice_serializes_through_server_adapter() {
        let message = ServerMessage::RoomNotice {
            msg: "Scout has joined the replay".to_string(),
        };
        assert_eq!(
            serde_json::to_value(message).unwrap(),
            serde_json::json!({"t": "roomNotice", "msg": "Scout has joined the replay"})
        );
    }

    #[test]
    fn shop_unit_target_round_trips_through_sim_adapter() {
        let value = serde_json::json!({
            "c":"useAbility", "ability":"escapeTreasure", "units":[1], "targetId":2
        });
        let wire: Command = serde_json::from_value(value.clone()).unwrap();
        let sim = rts_sim::game::command::SimCommand::from_protocol(wire);
        assert_eq!(serde_json::to_value(sim.to_protocol()).unwrap(), value);
    }

    #[test]
    fn point_tanks_command_round_trips_through_sim_adapter() {
        let wire: Command =
            serde_json::from_str(r#"{"c":"pointTanks","units":[1,2],"x":300.0,"y":400.0}"#)
                .unwrap();
        let sim = rts_sim::game::command::SimCommand::from_protocol(wire);
        assert_eq!(
            serde_json::to_value(sim.to_protocol()).unwrap(),
            serde_json::json!({"c":"pointTanks","units":[1,2],"x":300.0,"y":400.0})
        );
    }

    #[test]
    fn queued_point_tanks_round_trips_through_sim_adapter() {
        let value =
            serde_json::json!({"c":"pointTanks","units":[1],"x":300.0,"y":400.0,"queued":true});
        let wire: Command = serde_json::from_value(value.clone()).unwrap();
        let sim = rts_sim::game::command::SimCommand::from_protocol(wire);
        assert_eq!(serde_json::to_value(sim.to_protocol()).unwrap(), value);
    }

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
        assert_eq!(terrain::GRAVEL_A, rts_rules::terrain::MAP_TERRAIN_GRAVEL_A);
        assert_eq!(terrain::GRAVEL_B, rts_rules::terrain::MAP_TERRAIN_GRAVEL_B);
        assert_eq!(terrain::GRAVEL_C, rts_rules::terrain::MAP_TERRAIN_GRAVEL_C);
        assert_eq!(terrain::DIRT_A, rts_rules::terrain::MAP_TERRAIN_DIRT_A);
        assert_eq!(terrain::DIRT_B, rts_rules::terrain::MAP_TERRAIN_DIRT_B);
        assert_eq!(terrain::DIRT_C, rts_rules::terrain::MAP_TERRAIN_DIRT_C);
        assert_eq!(terrain::MUD_A, rts_rules::terrain::MAP_TERRAIN_MUD_A);
        assert_eq!(terrain::MUD_B, rts_rules::terrain::MAP_TERRAIN_MUD_B);
        assert_eq!(terrain::MUD_C, rts_rules::terrain::MAP_TERRAIN_MUD_C);
        assert_eq!(
            terrain::FROSTED_GROUND,
            rts_rules::terrain::MAP_TERRAIN_FROSTED_GROUND
        );
    }
}

// MG prototype: Attack.shotOrigin (compact v57, slot 6) fixes the ray origin; toPos fixes its endpoint.

// Disciple EntityView public insight/dao/daoReady/daoAdvanceTicks and team-private chosenDao are re-exported from rts-contract (slots 45–49).

// Tournament Grounds uses the shared kind adapter (tournament_grounds/code 32);
// EntityView tournament_entrants is team-private compact v62 slot 50.

// Proving Grounds uses kind code 33; team-private dao_training_queue appends compact v63 slot 51.
// Existing research commands targeting this building enqueue researched Dao training plans.

// Lab spawnEntity/spawnEntities entries accept optional dao: wind | war | magic (Novice only) and daoMaster: bool (Adepts only).

// Snapshot playerFactions (compact pf) projects public identity independently of private resources.

// Cultivator Shop: wire kind shop, compact kind code 34; no additional snapshot fields.

// Sword Disciple warrior (29) and Bow Novice bow_novice (41) evolve into distinct kinds wind_adept (35), war_adept (36),
// or magic_adept (37). EntityView.dao is derived presentation metadata, not engine identity.

// EntityView daoMaster: public Master rank at compact slot 52 (v65).

// Wind Dash uses windDash (ability 21, order stage 29); no return marker.

// Healing Seal: kind healing_seal (38), ability healingSeal (22), stage 30, compact v69.
// Existing hp/maxHp carry healing budget/capacity, not damageable health.

// Compact v70 entity slot 54: public itemRadianceTier (0–6), fog-gated with its carrier.

// Compact v71: shield ownerState adds maxHp (slot 6) and durationTicks (slot 7), public with the visible field.
