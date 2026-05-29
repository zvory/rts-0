//! Wire protocol (JSON over WebSocket). See `DESIGN.md` §2.
//!
//! This file is the authoritative Rust side of the contract. `client/src/protocol.js`
//! is its JavaScript mirror — change both together.
//!
//! Tag conventions: top-level messages use `"t"`, commands use `"c"`, events use `"e"`.
//! Coordinates are world pixels (floats) unless the field name ends in `Tile`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shared string vocabularies (kept as constants so the simulation never sprays
// magic strings; the JS mirror has the same values).
// ---------------------------------------------------------------------------

/// Terrain codes packed into `MapInfo.terrain` (row-major).
pub mod terrain {
    pub const GRASS: u8 = 0; // passable
    pub const ROCK: u8 = 1; // impassable
    pub const WATER: u8 = 2; // impassable
}

/// `EntityView.kind` values.
pub mod kinds {
    pub const WORKER: &str = "worker";
    pub const SOLDIER: &str = "soldier";
    pub const HEAVY: &str = "heavy";
    pub const HQ: &str = "hq";
    pub const DEPOT: &str = "depot";
    pub const BARRACKS: &str = "barracks";
    pub const TURRET: &str = "turret";
    pub const MINERALS: &str = "minerals";
    pub const GAS: &str = "gas";

    pub const UNITS: [&str; 3] = [WORKER, SOLDIER, HEAVY];
    pub const BUILDINGS: [&str; 4] = [HQ, DEPOT, BARRACKS, TURRET];

    pub fn is_unit(k: &str) -> bool {
        UNITS.contains(&k)
    }
    pub fn is_building(k: &str) -> bool {
        BUILDINGS.contains(&k)
    }
}

/// `EntityView.state` values.
pub mod states {
    pub const IDLE: &str = "idle";
    pub const MOVE: &str = "move";
    pub const ATTACK: &str = "attack";
    pub const GATHER: &str = "gather";
    pub const BUILD: &str = "build";
    pub const TRAIN: &str = "train";
    pub const CONSTRUCT: &str = "construct";
    pub const DEAD: &str = "dead";
}

// ---------------------------------------------------------------------------
// Client -> Server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ClientMessage {
    /// Join (or create) a room. `room` defaults to "main" when absent.
    Join {
        name: String,
        #[serde(default)]
        room: Option<String>,
    },
    /// Toggle ready state in the lobby.
    Ready { ready: bool },
    /// Host requests the match to begin.
    Start,
    /// Issue a gameplay command (ignored unless in-game).
    Command { cmd: Command },
    /// Latency probe.
    Ping { ts: f64 },
}

/// A gameplay command. Validated when applied, not when received.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "c", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
    },
    Build {
        worker: u32,
        building: String,
        tile_x: u32,
        tile_y: u32,
    },
    Train {
        building: u32,
        unit: String,
    },
    Cancel {
        building: u32,
    },
    Stop {
        units: Vec<u32>,
    },
}

// ---------------------------------------------------------------------------
// Server -> Client
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    Welcome {
        player_id: u32,
    },
    Lobby {
        room: String,
        host_id: u32,
        players: Vec<LobbyPlayer>,
        can_start: bool,
    },
    /// Match start (flattened: carries StartPayload's fields alongside `"t":"start"`).
    Start(StartPayload),
    /// Per-player, fog-filtered world state.
    Snapshot(Snapshot),
    GameOver {
        winner_id: Option<u32>,
        /// "won" | "lost" | "draw"
        you: String,
    },
    Pong {
        ts: f64,
    },
    Error {
        msg: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyPlayer {
    pub id: u32,
    pub name: String,
    pub ready: bool,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPayload {
    pub player_id: u32,
    pub tick: u32,
    pub map: MapInfo,
    pub players: Vec<PlayerStart>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    /// Row-major terrain codes, length = width * height. See [`terrain`].
    pub terrain: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStart {
    pub id: u32,
    pub name: String,
    pub color: String,
    pub start_tile_x: u32,
    pub start_tile_y: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tick: u32,
    pub minerals: u32,
    pub gas: u32,
    pub supply_used: u32,
    pub supply_cap: u32,
    pub entities: Vec<EntityView>,
    pub events: Vec<Event>,
}

/// One entity as seen by one player. Optional fields are omitted when not applicable.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityView {
    pub id: u32,
    /// 0 = neutral (resource nodes), otherwise the owning player id.
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub max_hp: u32,
    pub state: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<f32>,

    // Production buildings:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_progress: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prod_queue: Option<u32>,

    // Buildings under construction:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_progress: Option<f32>,

    // Workers carrying resources:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrying: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrying_kind: Option<String>,

    // Resource nodes:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<u32>,

    // Combat feedback:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<u32>,
}

impl EntityView {
    /// Minimal constructor; fill optional fields afterward.
    pub fn new(id: u32, owner: u32, kind: &str, x: f32, y: f32, hp: u32, max_hp: u32, state: &str) -> Self {
        EntityView {
            id,
            owner,
            kind: kind.to_string(),
            x,
            y,
            hp,
            max_hp,
            state: state.to_string(),
            facing: None,
            prod_kind: None,
            prod_progress: None,
            prod_queue: None,
            build_progress: None,
            carrying: None,
            carrying_kind: None,
            remaining: None,
            target_id: None,
        }
    }
}

/// Transient, single-snapshot visual feedback. Clients must not rely on delivery.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "e", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Attack { from: u32, to: u32 },
    Death { id: u32, x: f32, y: f32, kind: String },
    Build { id: u32, kind: String },
    Notice { msg: String },
}
