//! Wire protocol (JSON over WebSocket). See `docs/design/protocol.md`.
//!
//! This file is the authoritative Rust side of the contract. `client/src/protocol.js`
//! is its JavaScript mirror — change both together.
//!
//! Tag conventions: top-level messages use `"t"`, commands use `"c"`, events use `"e"`.
//! Coordinates are world pixels (floats) unless the field name ends in `Tile`.

use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

pub use rts_contract::{
    AbilityCooldownView, AttackReveal, DebugPathPoint, DebugPathView, EntityView, Event, MapInfo,
    NoticeSeverity, OrderPlanMarker, PlayerResourceSnapshot, PlayerScore, PlayerStart,
    ResourceDelta, ResourceNode, SmokeCloudView, Snapshot, SnapshotNetStatus, StartPayload,
};

fn is_false(value: &bool) -> bool {
    !*value
}

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
    pub const RIFLEMAN: &str = "rifleman";
    pub const MACHINE_GUNNER: &str = "machine_gunner";
    pub const AT_TEAM: &str = "at_team";
    pub const SCOUT_CAR: &str = "scout_car";
    pub const TANK: &str = "tank";
    pub const CITY_CENTRE: &str = "city_centre";
    pub const DEPOT: &str = "depot";
    pub const BARRACKS: &str = "barracks";
    pub const TRAINING_CENTRE: &str = "training_centre";
    pub const FACTORY: &str = "factory";
    pub const STEELWORKS: &str = "steelworks";
    pub const STEEL: &str = "steel";
    pub const OIL: &str = "oil";
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

/// Ability ids used by generic ability commands and owner-only cooldown projection.
pub mod abilities {
    pub const CHARGE: &str = "charge";
    pub const SMOKE: &str = "smoke";
}

// ---------------------------------------------------------------------------
// Client -> Server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ClientMessage {
    /// Join (or create) a room. `room` defaults to "main" when absent. Spectators must join
    /// before the match starts; they observe only and are not seated in the simulation.
    Join {
        name: String,
        #[serde(default)]
        room: Option<String>,
        #[serde(default)]
        spectator: bool,
    },
    /// Toggle ready state in the lobby.
    Ready { ready: bool },
    /// Host requests the match to begin.
    Start,
    /// Host adds a computer-controlled opponent to the room (lobby phase only).
    AddAi,
    /// Host removes a previously-added AI opponent by its player id (lobby phase only).
    RemoveAi { id: u32 },
    /// Host toggles the lobby's debug mode.
    SetQuickstart { enabled: bool },
    /// Switch between player and spectator role while still in the lobby.
    SetSpectator { spectator: bool },
    /// Issue a gameplay command (ignored unless in-game).
    Command { cmd: Command },
    /// Give up the current match, removing this player's army and showing the score screen.
    GiveUp,
    /// Latency probe.
    Ping { ts: f64 },
    /// Set replay playback speed multiplier (replay rooms only; ignored elsewhere).
    SetReplaySpeed { speed: f32 },
    /// Rewind a replay by `ticks_back` simulation ticks (replay rooms only; clamped to start).
    SeekReplay {
        #[serde(rename = "ticksBack")]
        ticks_back: u32,
    },
    /// Host selects a map by name (lobby phase only; ignored from non-hosts).
    SelectMap { map: String },
}

/// A gameplay command. Validated when applied, not when received.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "c", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    SetupAtGuns {
        units: Vec<u32>,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    TearDownAtGuns {
        units: Vec<u32>,
    },
    Charge {
        units: Vec<u32>,
    },
    UseAbility {
        ability: String,
        units: Vec<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        x: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        y: Option<f32>,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
    },
    Build {
        worker: u32,
        building: String,
        tile_x: u32,
        tile_y: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
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
    /// Set a unit-producing building's rally point to a world point. Produced units receive a
    /// move order to it and the building prefers the spawn exit closest to it.
    SetRally {
        building: u32,
        x: f32,
        y: f32,
        #[serde(default, skip_serializing_if = "is_false")]
        queued: bool,
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
        quickstart: bool,
        /// Currently selected map name.
        map: String,
        /// All available map names (populated from disk at broadcast time).
        maps: Vec<String>,
    },
    /// Match start (flattened: carries StartPayload's fields alongside `"t":"start"`).
    Start(StartPayload),
    /// Per-player, fog-filtered world state.
    Snapshot(Snapshot),
    GameOver {
        winner_id: Option<u32>,
        /// "won" | "lost" | "draw"
        you: String,
        /// Frozen per-player score snapshot for the score screen.
        scores: Vec<PlayerScore>,
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
    /// True for computer opponents (no socket). The client uses this to label the row and show a
    /// host-only "remove" control instead of a ready indicator.
    pub is_ai: bool,
    /// True for human observers. Spectators do not count toward match starts or win conditions.
    pub is_spectator: bool,
}

// ---------------------------------------------------------------------------
// Compact snapshot transport encoding
// ---------------------------------------------------------------------------

/// Version for the array-shaped JSON snapshot representation sent over WebSocket.
///
/// [`Snapshot`] remains the semantic source of truth for game code. This format is only a
/// transport-side optimization for `ServerMessage::Snapshot`.
pub const COMPACT_SNAPSHOT_VERSION: u8 = 9;

/// Serialize one semantic snapshot as a compact JSON text frame payload.
pub fn serialize_compact_snapshot(snapshot: &Snapshot) -> serde_json::Result<String> {
    serde_json::to_string(&CompactSnapshot(snapshot))
}

struct CompactSnapshot<'a>(&'a Snapshot);

impl Serialize for CompactSnapshot<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        let snapshot = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("t", "snapshot")?;
        map.serialize_entry("v", &COMPACT_SNAPSHOT_VERSION)?;
        map.serialize_entry(
            "s",
            &[
                snapshot.tick,
                snapshot.steel,
                snapshot.oil,
                snapshot.supply_used,
                snapshot.supply_cap,
            ],
        )?;
        map.serialize_entry(
            "e",
            &snapshot
                .entities
                .iter()
                .map(CompactEntity)
                .collect::<Vec<_>>(),
        )?;
        if !snapshot.resource_deltas.is_empty() {
            map.serialize_entry(
                "r",
                &snapshot
                    .resource_deltas
                    .iter()
                    .map(|delta| [delta.id, delta.remaining])
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.smokes.is_empty() {
            map.serialize_entry(
                "sm",
                &snapshot
                    .smokes
                    .iter()
                    .map(CompactSmokeCloud)
                    .collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.visible_tiles.is_empty() {
            map.serialize_entry("fg", &encode_visibility_runs(&snapshot.visible_tiles))?;
        }
        if !snapshot.events.is_empty() {
            map.serialize_entry(
                "ev",
                &snapshot.events.iter().map(CompactEvent).collect::<Vec<_>>(),
            )?;
        }
        if !snapshot.player_resources.is_empty() {
            map.serialize_entry(
                "pr",
                &snapshot
                    .player_resources
                    .iter()
                    .map(|p| [p.id, p.steel, p.oil, p.supply_used, p.supply_cap])
                    .collect::<Vec<_>>(),
            )?;
        }
        map.serialize_entry("n", &CompactNetStatus(&snapshot.net_status))?;
        map.end()
    }
}

fn encode_visibility_runs(visible_tiles: &[u8]) -> Vec<u32> {
    let Some((&first, rest)) = visible_tiles.split_first() else {
        return Vec::new();
    };
    let mut runs = vec![u32::from(first != 0)];
    let mut current = first != 0;
    let mut len: u32 = 1;
    for &tile in rest {
        let value = tile != 0;
        if value == current && len < u32::MAX {
            len += 1;
        } else {
            runs.push(len);
            current = value;
            len = 1;
        }
    }
    runs.push(len);
    runs
}

struct CompactSmokeCloud<'a>(&'a SmokeCloudView);

impl Serialize for CompactSmokeCloud<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let smoke = self.0;
        let mut seq = serializer.serialize_seq(Some(5))?;
        seq.serialize_element(&smoke.id)?;
        seq.serialize_element(&smoke.x)?;
        seq.serialize_element(&smoke.y)?;
        seq.serialize_element(&smoke.radius_tiles)?;
        seq.serialize_element(&smoke.expires_in)?;
        seq.end()
    }
}

struct CompactNetStatus<'a>(&'a SnapshotNetStatus);

impl Serialize for CompactNetStatus<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let status = self.0;
        let flags = u8::from(status.slow_tick) | (u8::from(status.head_of_line) << 1);
        [
            status.server_lag_ms as u32,
            status.tick_ms as u32,
            flags as u32,
            status.slow_tick_count,
            status.head_of_line_count,
        ]
        .serialize(serializer)
    }
}

struct CompactEntity<'a>(&'a EntityView);

impl Serialize for CompactEntity<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entity = self.0;
        let prod_kind = entity.prod_kind.as_deref().map(kind_code);
        let setup_state = entity.setup_state.as_deref().map(setup_state_code);

        let mut len = 8;
        if entity.facing.is_some() {
            len = 9;
        }
        if entity.weapon_facing.is_some() {
            len = 10;
        }
        if prod_kind.is_some() {
            len = 11;
        }
        if entity.prod_progress.is_some() {
            len = 12;
        }
        if entity.prod_queue.is_some() {
            len = 13;
        }
        if entity.build_progress.is_some() {
            len = 14;
        }
        if entity.latched_node.is_some() {
            len = 15;
        }
        if entity.target_id.is_some() {
            len = 16;
        }
        if setup_state.is_some() {
            len = 17;
        }
        if entity.remaining.is_some() {
            len = 18;
        }
        if entity.rally.is_some() {
            len = 19;
        }
        if entity.oil_used.is_some() {
            len = 20;
        }
        if entity.setup_facing.is_some() {
            len = 21;
        }
        if !entity.order_plan.is_empty() {
            len = 22;
        }
        if entity.charge_cooldown_left.is_some() {
            len = 23;
        }
        if !entity.abilities.is_empty() {
            len = 24;
        }
        if entity.vision_only {
            len = 25;
        }
        if entity.debug_path.is_some() {
            len = 26;
        }

        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&entity.id)?;
        seq.serialize_element(&entity.owner)?;
        seq.serialize_element(&kind_code(&entity.kind))?;
        seq.serialize_element(&entity.x)?;
        seq.serialize_element(&entity.y)?;
        seq.serialize_element(&entity.hp)?;
        seq.serialize_element(&entity.max_hp)?;
        seq.serialize_element(&state_code(&entity.state))?;
        if len > 8 {
            seq.serialize_element(&entity.facing)?;
        }
        if len > 9 {
            seq.serialize_element(&entity.weapon_facing)?;
        }
        if len > 10 {
            seq.serialize_element(&prod_kind)?;
        }
        if len > 11 {
            seq.serialize_element(&entity.prod_progress)?;
        }
        if len > 12 {
            seq.serialize_element(&entity.prod_queue)?;
        }
        if len > 13 {
            seq.serialize_element(&entity.build_progress)?;
        }
        if len > 14 {
            seq.serialize_element(&entity.latched_node)?;
        }
        if len > 15 {
            seq.serialize_element(&entity.target_id)?;
        }
        if len > 16 {
            seq.serialize_element(&setup_state)?;
        }
        if len > 17 {
            seq.serialize_element(&entity.remaining)?;
        }
        if len > 18 {
            seq.serialize_element(&entity.rally)?;
        }
        if len > 19 {
            seq.serialize_element(&entity.oil_used)?;
        }
        if len > 20 {
            seq.serialize_element(&entity.setup_facing)?;
        }
        if len > 21 {
            seq.serialize_element(
                &entity
                    .order_plan
                    .iter()
                    .map(CompactOrderPlanMarker)
                    .collect::<Vec<_>>(),
            )?;
        }
        if len > 22 {
            seq.serialize_element(&entity.charge_cooldown_left)?;
        }
        if len > 23 {
            seq.serialize_element(
                &entity
                    .abilities
                    .iter()
                    .map(CompactAbilityCooldown)
                    .collect::<Vec<_>>(),
            )?;
        }
        if len > 24 {
            seq.serialize_element(&entity.vision_only)?;
        }
        if len > 25 {
            seq.serialize_element(&entity.debug_path.as_ref().map(CompactDebugPath))?;
        }
        seq.end()
    }
}

struct CompactAbilityCooldown<'a>(&'a AbilityCooldownView);

impl Serialize for CompactAbilityCooldown<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let ability = self.0;
        let len = if ability.remaining_uses.is_some() {
            3
        } else {
            2
        };
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&ability_code(&ability.ability))?;
        seq.serialize_element(&ability.cooldown_left)?;
        if ability.remaining_uses.is_some() {
            seq.serialize_element(&ability.remaining_uses)?;
        }
        seq.end()
    }
}

struct CompactOrderPlanMarker<'a>(&'a OrderPlanMarker);

impl Serialize for CompactOrderPlanMarker<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let marker = self.0;
        let mut seq = serializer.serialize_seq(Some(3))?;
        seq.serialize_element(&order_stage_code(&marker.kind))?;
        seq.serialize_element(&marker.x)?;
        seq.serialize_element(&marker.y)?;
        seq.end()
    }
}

struct CompactDebugPath<'a>(&'a DebugPathView);

impl Serialize for CompactDebugPath<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let path = self.0;
        let mut seq = serializer.serialize_seq(Some(6))?;
        seq.serialize_element(
            &path
                .waypoints
                .iter()
                .map(|p| [p.x, p.y])
                .collect::<Vec<_>>(),
        )?;
        seq.serialize_element(&path.goal.map(|p| [p.x, p.y]))?;
        seq.serialize_element(&path.last_repath_tick)?;
        seq.serialize_element(&path.stuck_ticks)?;
        seq.serialize_element(&path.static_blocked_ticks)?;
        seq.serialize_element(&path.total_waypoints)?;
        seq.end()
    }
}

struct CompactEvent<'a>(&'a Event);

impl Serialize for CompactEvent<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Event::Attack {
                from,
                to,
                reveal,
                to_pos,
            } => {
                let len = if to_pos.is_some() {
                    5
                } else if reveal.is_some() {
                    4
                } else {
                    3
                };
                let mut seq = serializer.serialize_seq(Some(len))?;
                seq.serialize_element(&1u8)?;
                seq.serialize_element(from)?;
                seq.serialize_element(to)?;
                if len > 3 {
                    seq.serialize_element(&reveal.as_ref().map(CompactAttackReveal))?;
                }
                if len > 4 {
                    seq.serialize_element(to_pos)?;
                }
                seq.end()
            }
            Event::Death { id, x, y, kind } => {
                let mut seq = serializer.serialize_seq(Some(5))?;
                seq.serialize_element(&2u8)?;
                seq.serialize_element(id)?;
                seq.serialize_element(x)?;
                seq.serialize_element(y)?;
                seq.serialize_element(&kind_code(kind))?;
                seq.end()
            }
            Event::Build { id, kind } => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element(&3u8)?;
                seq.serialize_element(id)?;
                seq.serialize_element(&kind_code(kind))?;
                seq.end()
            }
            Event::SmokeLaunch {
                from_x,
                from_y,
                to_x,
                to_y,
                delay_ticks,
            } => {
                let mut seq = serializer.serialize_seq(Some(4))?;
                seq.serialize_element(&5u8)?;
                seq.serialize_element(&[from_x, from_y])?;
                seq.serialize_element(&[to_x, to_y])?;
                seq.serialize_element(delay_ticks)?;
                seq.end()
            }
            Event::Notice {
                msg,
                x,
                y,
                severity,
            } => {
                let has_position = x.is_some() && y.is_some();
                let len = if has_position {
                    5
                } else if !severity.is_info() {
                    3
                } else {
                    2
                };
                let mut seq = serializer.serialize_seq(Some(len))?;
                seq.serialize_element(&4u8)?;
                seq.serialize_element(msg)?;
                if len > 2 {
                    seq.serialize_element(&notice_severity_code(*severity))?;
                }
                if len > 3 {
                    seq.serialize_element(x)?;
                    seq.serialize_element(y)?;
                }
                seq.end()
            }
        }
    }
}

struct CompactAttackReveal<'a>(&'a AttackReveal);

impl Serialize for CompactAttackReveal<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let reveal = self.0;
        let setup_state = reveal.setup_state.as_deref().map(setup_state_code);

        let mut len = 4;
        if reveal.facing.is_some() {
            len = 5;
        }
        if reveal.weapon_facing.is_some() {
            len = 6;
        }
        if setup_state.is_some() {
            len = 7;
        }

        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&reveal.owner)?;
        seq.serialize_element(&kind_code(&reveal.kind))?;
        seq.serialize_element(&reveal.x)?;
        seq.serialize_element(&reveal.y)?;
        if len > 4 {
            seq.serialize_element(&reveal.facing)?;
        }
        if len > 5 {
            seq.serialize_element(&reveal.weapon_facing)?;
        }
        if len > 6 {
            seq.serialize_element(&setup_state)?;
        }
        seq.end()
    }
}

fn kind_code(kind: &str) -> u8 {
    match kind {
        kinds::WORKER => 1,
        kinds::RIFLEMAN => 2,
        kinds::MACHINE_GUNNER => 3,
        kinds::AT_TEAM => 4,
        kinds::TANK => 5,
        kinds::SCOUT_CAR => 14,
        kinds::CITY_CENTRE => 6,
        kinds::DEPOT => 7,
        kinds::BARRACKS => 8,
        kinds::TRAINING_CENTRE => 9,
        kinds::FACTORY => 10,
        kinds::STEEL => 11,
        kinds::OIL => 12,
        kinds::STEELWORKS => 13,
        _ => 255,
    }
}

fn state_code(state: &str) -> u8 {
    match state {
        states::IDLE => 1,
        states::MOVE => 2,
        states::ATTACK => 3,
        states::GATHER => 4,
        states::BUILD => 5,
        states::TRAIN => 6,
        states::CONSTRUCT => 7,
        states::DEAD => 8,
        _ => 255,
    }
}

fn setup_state_code(setup_state: &str) -> u8 {
    match setup_state {
        "packed" => 1,
        "setting_up" => 2,
        "deployed" => 3,
        "tearing_down" => 4,
        _ => 255,
    }
}

fn order_stage_code(kind: &str) -> u8 {
    match kind {
        "move" => 1,
        "attackMove" => 2,
        "attack" => 3,
        "gather" => 4,
        "build" => 5,
        abilities::SMOKE => 6,
        "setupAtGuns" => 7,
        _ => 255,
    }
}

fn ability_code(ability: &str) -> u8 {
    match ability {
        abilities::CHARGE => 1,
        abilities::SMOKE => 2,
        _ => 255,
    }
}

fn notice_severity_code(severity: NoticeSeverity) -> u8 {
    match severity {
        NoticeSeverity::Info => 1,
        NoticeSeverity::Warn => 2,
        NoticeSeverity::Alert => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn representative_snapshot() -> Snapshot {
        let mut worker = EntityView::new(1, 1, kinds::WORKER, 10.0, 20.0, 40, 40, states::GATHER);
        worker.facing = Some(1.5);
        worker.weapon_facing = Some(1.75);
        worker.latched_node = Some(200);
        worker.target_id = Some(9);
        worker.order_plan = vec![
            OrderPlanMarker {
                kind: "move".to_string(),
                x: 96.0,
                y: 112.0,
            },
            OrderPlanMarker {
                kind: "setupAtGuns".to_string(),
                x: 128.0,
                y: 160.0,
            },
            OrderPlanMarker {
                kind: abilities::SMOKE.to_string(),
                x: 192.0,
                y: 224.0,
            },
        ];
        worker.charge_cooldown_left = Some(87);
        worker.abilities = vec![AbilityCooldownView {
            ability: abilities::CHARGE.to_string(),
            cooldown_left: 87,
            remaining_uses: Some(2),
        }];
        worker.vision_only = true;

        let mut gunner = EntityView::new(
            2,
            1,
            kinds::MACHINE_GUNNER,
            30.0,
            40.0,
            55,
            55,
            states::ATTACK,
        );
        gunner.target_id = Some(7);
        gunner.setup_state = Some("deployed".to_string());

        let mut center = EntityView::new(
            3,
            1,
            kinds::CITY_CENTRE,
            100.0,
            120.0,
            450,
            500,
            states::TRAIN,
        );
        center.prod_kind = Some(kinds::WORKER.to_string());
        center.prod_progress = Some(0.25);
        center.prod_queue = Some(2);
        center.build_progress = Some(0.75);
        center.rally = Some([256.0, 512.0]);

        Snapshot {
            tick: 42,
            steel: 100,
            oil: 25,
            supply_used: 3,
            supply_cap: 10,
            entities: vec![worker, gunner, center],
            resource_deltas: vec![ResourceDelta {
                id: 200,
                remaining: 1498,
            }],
            smokes: vec![SmokeCloudView {
                id: 50,
                x: 320.0,
                y: 352.0,
                radius_tiles: 2.0,
                expires_in: 120,
            }],
            visible_tiles: vec![1, 1, 0, 0, 0, 1],
            events: vec![
                Event::Attack {
                    from: 1,
                    to: 7,
                    reveal: Some(AttackReveal {
                        owner: 1,
                        kind: kinds::AT_TEAM.to_string(),
                        x: 12.0,
                        y: 24.0,
                        facing: Some(0.5),
                        weapon_facing: Some(0.75),
                        setup_state: Some("deployed".to_string()),
                    }),
                    to_pos: Some([48.0, 96.0]),
                },
                Event::Death {
                    id: 200,
                    x: 64.0,
                    y: 96.0,
                    kind: kinds::STEEL.to_string(),
                },
                Event::Build {
                    id: 3,
                    kind: kinds::CITY_CENTRE.to_string(),
                },
                Event::Notice {
                    msg: "Not enough steel".to_string(),
                    x: None,
                    y: None,
                    severity: NoticeSeverity::Info,
                },
            ],
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus {
                server_lag_ms: 4,
                tick_ms: 17,
                slow_tick: false,
                slow_tick_count: 2,
                head_of_line: true,
                head_of_line_count: 3,
            },
        }
    }

    #[test]
    fn compact_snapshot_is_versioned_and_smaller_than_object_json() {
        let snapshot = representative_snapshot();
        let compact = serialize_compact_snapshot(&snapshot).unwrap();
        let object = serde_json::to_string(&ServerMessage::Snapshot(snapshot.clone())).unwrap();

        assert!(
            compact.len() < object.len(),
            "compact={} object={}",
            compact.len(),
            object.len()
        );

        let value: serde_json::Value = serde_json::from_str(&compact).unwrap();
        assert_eq!(value["t"], "snapshot");
        assert_eq!(value["v"], COMPACT_SNAPSHOT_VERSION);
        assert_eq!(value["s"], serde_json::json!([42, 100, 25, 3, 10]));
        assert_eq!(value["e"].as_array().unwrap().len(), 3);
        assert_eq!(value["e"][0][8], serde_json::json!(1.5));
        assert_eq!(value["e"][0][9], serde_json::json!(1.75));
        assert_eq!(value["e"][0][14], serde_json::json!(200));
        assert_eq!(value["e"][0][15], serde_json::json!(9));
        assert_eq!(
            value["e"][0][21],
            serde_json::json!([[1, 96.0, 112.0], [7, 128.0, 160.0], [6, 192.0, 224.0]])
        );
        assert_eq!(value["e"][0][22], serde_json::json!(87));
        assert_eq!(value["e"][0][23], serde_json::json!([[1, 87, 2]]));
        assert_eq!(value["e"][0][24], serde_json::json!(true));
        // Rally point rides in slot 18 of the producing building's record.
        assert_eq!(value["e"][2][18], serde_json::json!([256.0, 512.0]));
        assert_eq!(value["r"], serde_json::json!([[200, 1498]]));
        assert_eq!(
            value["sm"],
            serde_json::json!([[50, 320.0, 352.0, 2.0, 120]])
        );
        assert_eq!(value["fg"], serde_json::json!([1, 2, 3, 1]));
        assert_eq!(value["ev"].as_array().unwrap().len(), 4);
        assert_eq!(value["n"], serde_json::json!([4, 17, 2, 2, 3]));
        assert_eq!(
            value["ev"][0][3],
            serde_json::json!([1, 4, 12.0, 24.0, 0.5, 0.75, 3])
        );
        assert_eq!(value["ev"][0][4], serde_json::json!([48.0, 96.0]));
    }

    #[test]
    fn compact_entity_trims_trailing_optional_nulls() {
        let snapshot = Snapshot {
            tick: 1,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            entities: vec![EntityView::new(
                1,
                1,
                kinds::WORKER,
                10.0,
                20.0,
                40,
                40,
                states::IDLE,
            )],
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            visible_tiles: Vec::new(),
            events: Vec::new(),
            player_resources: Vec::new(),
            net_status: SnapshotNetStatus::default(),
        };

        let compact = serialize_compact_snapshot(&snapshot).unwrap();
        let value: serde_json::Value = serde_json::from_str(&compact).unwrap();
        let entity = value["e"][0].as_array().unwrap();
        assert_eq!(entity.len(), 8);
        assert!(value.get("r").is_none());
        assert!(value.get("ev").is_none());
    }
}
