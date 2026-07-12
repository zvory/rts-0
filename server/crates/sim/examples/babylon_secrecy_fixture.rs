use rts_protocol::terrain;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{LabOp, LabOpOutcome, LabSpawnEntity};
use rts_sim::game::map::{Map, CURRENT_MAP_VERSION};
use rts_sim::game::{Game, MapMetadata, PlayerInit};
use serde_json::json;

fn main() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Recipient One".to_string(),
            color: "#3377aa".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Recipient Two".to_string(),
            color: "#aa4433".to_string(),
            is_ai: false,
        },
    ];
    let size = 64;
    let map = Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(6, 6), (57, 57)],
        base_sites: vec![(6, 6), (57, 57)],
    };
    let mut game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        0xBABA_0005,
        map,
        MapMetadata {
            name: "Babylon secrecy fixture".to_string(),
            schema_version: CURRENT_MAP_VERSION,
            content_hash: "babylon-secrecy-fixture-v1".to_string(),
        },
    );
    let sentinel_x = 48.5 * 32.0;
    let sentinel_y = 50.5 * 32.0;
    let outcome = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 2,
            kind: EntityKind::Tank,
            x: sentinel_x,
            y: sentinel_y,
            completed: true,
        }))
        .expect("sentinel spawn must be valid");
    let LabOpOutcome::Spawned {
        entity_id: sentinel_id,
    } = outcome
    else {
        panic!("sentinel spawn returned the wrong outcome");
    };
    let recipient_one = game.snapshot_for(1);
    let recipient_two = game.snapshot_for(2);
    assert!(
        recipient_one
            .entities
            .iter()
            .all(|entity| entity.id != sentinel_id),
        "recipient one must never receive the sentinel"
    );
    assert!(
        recipient_two
            .entities
            .iter()
            .any(|entity| entity.id == sentinel_id),
        "the owning recipient proves the sentinel really exists"
    );

    println!(
        "{}",
        serde_json::to_string(&json!({
            "sentinelId": sentinel_id,
            "map": {
                "width": size,
                "height": size,
                "tileSize": 32,
                "terrain": vec![terrain::GRASS; (size * size) as usize],
                "resources": [],
            },
            "players": players,
            "recipientOne": recipient_one,
            "recipientTwo": recipient_two,
        }))
        .expect("fixture JSON must serialize")
    );
}
