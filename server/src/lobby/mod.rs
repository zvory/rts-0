//! Lobby & room orchestration. See `docs/design/server-sim.md` §3.2.
//!
//! Concurrency model: there is exactly **one tokio task per room** and that task is the
//! sole owner/writer of its [`Game`]. There are no locks around the simulation. Everything
//! else — connections, the lobby registry — talks to a room only by sending it
//! [`RoomEvent`]s over an `mpsc` channel. A room task multiplexes (via `tokio::select!`)
//! between its fixed-rate tick and that event stream, so a slow or disconnected client can
//! never stall the simulation.
//!
//! Lifecycle:
//! 1. A connection joins a room (creating it if needed). The room task is spawned lazily.
//! 2. In the `Lobby` phase the room broadcasts a `lobby` message on every membership/ready
//!    change. The host may start the match when everyone is ready.
//! 3. In the `InGame` phase the room advances [`Game`] once per tick, fans out a fog-filtered
//!    snapshot to each connected player, and detects game-over. When the match resolves the
//!    room returns to the `Lobby` phase (ready flags reset) so the same players can rematch.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, error, info, warn};

use crate::config;
use crate::db::Db;
use crate::game::command::SimCommand;
use crate::game::{Game, PlayerInit};
use crate::protocol::{
    Event, LobbyPlayer, PlayerScore, ResourceDelta, ServerMessage, Snapshot, StartPayload,
};
use rts_ai::selfplay::{is_safe_artifact_name, LiveSelfPlay, ReplayArtifact, ReplayDriver};

mod connection;
mod crash_replay;
mod dev_replay;
mod room_task;
mod snapshots;

pub use connection::{ConnectionSink, ConnectionWriter};
use dev_replay::room_mode_for;
use room_task::RoomTask;
pub use snapshots::compact_snapshot_for_wire;

/// Player colors, assigned from the head of the palette. MUST match `client/src/config.js`
/// `PLAYER_PALETTE`.
const PLAYER_PALETTE: [&str; 8] = [
    "#4878c8", "#c84848", "#30a090", "#8040c8", "#c83880", "#c87830", "#409840", "#c8b030",
];

/// Hard cap on players in a single match (humans + AI). The hardcoded map has four authored
/// player-start slots, so we never seat more than this.
const MAX_PLAYERS: usize = 4;

/// Bound on a player's reliable outbound message queue. Snapshots do not use this FIFO; each
/// connection has one replaceable latest-snapshot slot so stale world states cannot backlog.
const PLAYER_RELIABLE_CHANNEL_CAP: usize = 64;

/// Bound on a room's inbound event queue. Commands/joins past this are dropped rather than
/// allowed to grow without limit; in practice the room drains this every tick.
const ROOM_EVENT_CHANNEL_CAP: usize = 1024;
const DEV_SELFPLAY_ROOM_PREFIX: &str = "__dev_selfplay__";
const DEV_SCENARIO_ROOM_PREFIX: &str = "__dev_scenario__:";
const MATCH_SEED_ENV: &str = "RTS_MATCH_SEED";

/// Monotonic source of globally-unique player ids (ids are never reused within a process run).
static NEXT_PLAYER_ID: AtomicU32 = AtomicU32::new(1);

/// Allocate a fresh, process-unique player id. Called once per connection.
pub fn next_player_id() -> u32 {
    NEXT_PLAYER_ID.fetch_add(1, Ordering::Relaxed)
}

/// Internal message from a connection (or the lobby) to a room task. The room task is the
/// only consumer; see module docs.
#[derive(Debug)]
pub enum RoomEvent {
    /// A player joins this room. `msg_tx` is the connection's outbound sink. `ack` carries the
    /// accept/reject decision back to the connection: `true` once the player is actually in the
    /// room, `false` if the join was rejected (duplicate, or mid-match). The connection must not
    /// mark itself joined until it sees a `true`, so a rejected join doesn't wedge the socket.
    Join {
        player_id: u32,
        name: String,
        spectator: bool,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    },
    /// A player left (socket closed). During a match this eliminates them so it can resolve.
    Leave { player_id: u32 },
    /// A player toggled their lobby ready flag.
    Ready { player_id: u32, ready: bool },
    /// The host requested the match to begin (honored only from the host when `can_start`).
    StartRequest { player_id: u32 },
    /// The host asked to add a computer opponent (lobby phase only; honored only from the host).
    AddAi { player_id: u32 },
    /// The host asked to remove an AI opponent by id (lobby phase only; honored only from host).
    RemoveAi { player_id: u32, target: u32 },
    /// The host toggled the lobby's start-with-more-money mode.
    SetQuickstart { player_id: u32, enabled: bool },
    /// A connected human switched between active player and spectator role in the lobby.
    SetSpectator { player_id: u32, spectator: bool },
    /// A gameplay command (ignored unless the room is in-game and the sender is in the room).
    Command { player_id: u32, cmd: SimCommand },
    /// A connected player intentionally gave up the active match.
    GiveUp { player_id: u32 },
    /// Set dev playback speed multiplier (replay/scenario rooms only; ignored elsewhere).
    SetReplaySpeed { speed: f32 },
    /// Rewind a replay by `ticks_back` simulation ticks (replay rooms only; clamped to start).
    SeekReplay { ticks_back: u32 },
    /// Host selects a map by name (lobby phase only; honored only from the host).
    SelectMap { player_id: u32, map: String },
}

/// Handle the lobby keeps for each live room: just the channel into its task.
#[derive(Clone)]
pub struct RoomHandle {
    pub event_tx: mpsc::Sender<RoomEvent>,
}

/// Registry of rooms by name. Cheaply cloneable; share one instance across all connections.
#[derive(Clone)]
pub struct Lobby {
    rooms: Arc<Mutex<HashMap<String, RoomHandle>>>,
    db: Option<Arc<Db>>,
    match_history_local_only: bool,
}

impl Lobby {
    pub fn new() -> Self {
        Lobby {
            rooms: Arc::new(Mutex::new(HashMap::new())),
            db: None,
            match_history_local_only: false,
        }
    }

    /// Attach a database for match-history persistence. New rooms will inherit it; existing rooms
    /// (none at construction time) are unaffected.
    pub fn with_db(mut self, db: Option<Arc<Db>>) -> Self {
        self.db = db;
        self.match_history_local_only = false;
        self
    }

    /// Attach a database for match-history persistence with the desired write visibility. New
    /// rooms inherit both the handle and scope; existing rooms are unaffected.
    pub fn with_match_history(mut self, db: Option<Arc<Db>>, local_only: bool) -> Self {
        self.db = db;
        self.match_history_local_only = local_only;
        self
    }

    /// Get the handle for `room`, spawning the room task on first use. The `Mutex` here only
    /// guards the small name→handle map (cheap, never held across `.await` of game work); it is
    /// emphatically *not* a lock around any `Game`.
    pub async fn get_or_create(&self, room: &str) -> RoomHandle {
        let mut rooms = self.rooms.lock().await;
        if let Some(handle) = rooms.get(room) {
            return handle.clone();
        }
        let (event_tx, event_rx) = mpsc::channel(ROOM_EVENT_CHANNEL_CAP);
        let handle = RoomHandle { event_tx };
        rooms.insert(room.to_string(), handle.clone());

        let name = room.to_string();
        let mode = room_mode_for(&name);
        let db = self.db.clone();
        let match_history_local_only = self.match_history_local_only;
        tokio::spawn(async move {
            let mut task = RoomTask::new(name.clone(), mode, db, match_history_local_only);
            task.run(event_rx).await;
            info!(room = %name, "room task exited");
        });
        info!(room = %room, "room created");
        handle
    }
}

impl Default for Lobby {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
