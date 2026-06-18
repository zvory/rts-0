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
//!    snapshot to each connected player, and detects game-over. When a real match resolves the
//!    room sends `gameOver`, transitions connected humans into post-match replay playback, and
//!    returns to `Lobby` only after every replay viewer has left.
//!
//! `room_task` owns lifecycle and phase changes; `live_tick`, `replay_session`, `replay_branch`,
//! and `snapshot_fanout` hold the extracted tick, replay, branch, and delivery plumbing.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, watch, Mutex, Notify};
use tokio::time::{interval, MissedTickBehavior};

use crate::config;
use crate::db::Db;
use crate::protocol::{
    BranchStagingOccupant, Event, LabClientOp, LobbyPlayer, PlayerScore, ReplayBranchSeat,
    ReplayStartMetadata, ReplayVisionRequest, ResourceDelta, ServerMessage, Snapshot, TeamId,
};
use rts_ai::selfplay::is_safe_artifact_name;
use rts_sim::game::command::SimCommand;
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::{Game, PlayerInit};

mod connection;
mod crash_replay;
mod dev_replay;
mod faction_validation;
mod launch;
mod live_tick;
mod participants;
mod projection;
mod replay_branch;
mod replay_session;
mod replay_validation;
mod room_task;
mod session_policy;
mod snapshot_fanout;
mod snapshots;
mod tick_control;

pub use connection::{ConnectionSink, ConnectionWriter};
use dev_replay::room_mode_for;
use replay_session::{validate_replay_vision_request, ReplaySession};
pub use replay_validation::faction_loadout_incompatibility_reason as replay_faction_loadout_incompatibility_reason;
use room_task::{RoomMode, RoomTask};
pub(crate) use snapshots::compact_snapshot_for_wire;

/// Player colors, assigned in colorblind-safer order. MUST match `client/src/config.js`
/// `PLAYER_PALETTE`.
const PLAYER_PALETTE: [&str; 8] = [
    "#0072b2", "#d55e00", "#009e73", "#cc79a7", "#56b4e9", "#e69f00", "#f0e442", "#7e57c2",
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
const DEV_SCENARIO_ROOM_PREFIX: &str = "__dev_scenario__:";
const REPLAY_ARTIFACT_ROOM_PREFIX: &str = "__replay_artifact__:";
const MATCH_REPLAY_ROOM_PREFIX: &str = "__match_replay__";
const REPLAY_BRANCH_ROOM_PREFIX: &str = "__replay_branch__";
const LAB_ROOM_PREFIX: &str = "__lab__:";
const MATCH_SEED_ENV: &str = "RTS_MATCH_SEED";

/// Monotonic source of globally-unique player ids (ids are never reused within a process run).
static NEXT_PLAYER_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_MATCH_REPLAY_ROOM_ID: AtomicU32 = AtomicU32::new(1);

/// Allocate a fresh, process-unique player id. Called once per connection.
pub fn next_player_id() -> u32 {
    NEXT_PLAYER_ID.fetch_add(1, Ordering::Relaxed)
}

fn normalize_start_team_id(player_id: u32, team_id: TeamId) -> TeamId {
    if team_id == 0 {
        player_id
    } else {
        team_id
    }
}

/// Frozen server-side seed for a future practice branch staging room.
#[derive(Clone)]
pub struct ReplayBranchSeed {
    pub source_replay: ReplayStartMetadata,
    pub source_tick: u32,
    pub game: Box<Game>,
    pub seats: Vec<ReplayBranchSeat>,
}

/// Internal message from a connection (or the lobby) to a room task. The room task is the
/// only consumer; see module docs.
pub enum RoomEvent {
    /// A player joins this room. `msg_tx` is the connection's outbound sink. `ack` carries the
    /// accept/reject decision back to the connection: `true` once the player is actually in the
    /// room, `false` if the join was rejected (duplicate, or mid-match). The connection must not
    /// mark itself joined until it sees a `true`, so a rejected join doesn't wedge the socket.
    Join {
        player_id: u32,
        name: String,
        spectator: bool,
        replay_ok: bool,
        msg_tx: ConnectionSink,
        ack: tokio::sync::oneshot::Sender<bool>,
    },
    /// A player left (socket closed). During a match this eliminates them so it can resolve.
    Leave { player_id: u32 },
    /// A player toggled their lobby ready flag.
    Ready { player_id: u32, ready: bool },
    /// The host requested the match to begin (honored only from the host when `can_start`).
    StartRequest { player_id: u32 },
    /// Deprecated compatibility command. Team slots are host-managed via `SetTeam`.
    SetTeamPreset { player_id: u32, preset: String },
    /// The host assigned one active lobby seat to a team (lobby phase only; honored only from host).
    SetTeam {
        player_id: u32,
        target: u32,
        team_id: TeamId,
    },
    /// A connected active human selected their own playable faction in the lobby.
    SetFaction { player_id: u32, faction_id: String },
    /// The host asked to add a computer opponent (lobby phase only; honored only from the host).
    AddAi {
        player_id: u32,
        team_id: Option<TeamId>,
        ai_profile_id: Option<String>,
    },
    /// The host selected a live AI profile for one AI opponent.
    SetAiProfile {
        player_id: u32,
        target: u32,
        ai_profile_id: String,
    },
    /// The host asked to remove an AI opponent by id (lobby phase only; honored only from host).
    RemoveAi { player_id: u32, target: u32 },
    /// The host toggled the lobby's start-with-more-money mode.
    SetQuickstart { player_id: u32, enabled: bool },
    /// A connected human switched between active player and spectator role in the lobby. `target`
    /// may differ from `player_id` only for host-managed lobby moves.
    SetSpectator {
        player_id: u32,
        target: u32,
        spectator: bool,
    },
    /// A gameplay command (ignored unless the room is in-game and the sender is in the room).
    Command {
        player_id: u32,
        client_seq: u32,
        cmd: SimCommand,
    },
    /// A connected player intentionally gave up the active match.
    GiveUp { player_id: u32 },
    /// A replay viewer asked to leave playback and return their connection to the lobby screen.
    ReturnToLobby { player_id: u32 },
    /// Set replay/dev-watch playback speed multiplier; ignored outside replay/dev watch rooms.
    SetReplaySpeed { player_id: u32, speed: f32 },
    /// Advance a paused dev-watch room by one simulation tick.
    StepDevTick { player_id: u32 },
    /// Rewind a replay by `ticks_back` simulation ticks (replay rooms only; clamped to start).
    SeekReplay { player_id: u32, ticks_back: u32 },
    /// Seek a replay to an absolute simulation tick (replay rooms only; clamped to duration).
    SeekReplayTo { player_id: u32, tick: u32 },
    /// Select replay vision for this viewer only. Ignored outside replay rooms in phase 1.
    SetReplayVision {
        player_id: u32,
        vision: ReplayVisionRequest,
    },
    /// Privileged lab request routed only by lab rooms.
    Lab {
        player_id: u32,
        request_id: u32,
        op: LabClientOp,
    },
    /// A replay viewer requested a frozen practice branch seed from the current replay tick.
    RequestReplayBranch {
        player_id: u32,
        reply: tokio::sync::oneshot::Sender<Result<ReplayBranchSeed, String>>,
    },
    /// Claim one original replay seat in branch staging.
    ClaimBranchSeat { player_id: u32, seat_player_id: u32 },
    /// Release one original replay seat in branch staging.
    ReleaseBranchSeat { player_id: u32, seat_player_id: u32 },
    /// Host asks to launch the branch from staging.
    StartBranch { player_id: u32 },
    /// Announce a successfully-created branch room to all current replay viewers.
    AnnounceReplayBranch {
        branch_room: String,
        source_tick: u32,
        seats: Vec<ReplayBranchSeat>,
    },
    /// Host selects a map by name (lobby phase only; honored only from the host).
    SelectMap { player_id: u32, map: String },
    /// Process shutdown has begun. Rooms stay alive, but lobby clients should see that starting
    /// another match is disabled while currently-running matches drain.
    DrainStarted(DrainNotice),
}

#[derive(Clone)]
pub(super) struct DrainHandle {
    inner: Arc<DrainState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrainNotice {
    pub deadline_unix_ms: u64,
    pub seconds_remaining: u64,
}

struct DrainState {
    draining: AtomicBool,
    notice: StdMutex<Option<DrainNotice>>,
    active_matches: AtomicUsize,
    active_matches_tx: watch::Sender<usize>,
    connection_shutdown_tx: watch::Sender<bool>,
}

impl Default for DrainHandle {
    fn default() -> Self {
        let (active_matches_tx, _active_matches_rx) = watch::channel(0);
        let (connection_shutdown_tx, _connection_shutdown_rx) = watch::channel(false);
        Self {
            inner: Arc::new(DrainState {
                draining: AtomicBool::new(false),
                notice: StdMutex::new(None),
                active_matches: AtomicUsize::new(0),
                active_matches_tx,
                connection_shutdown_tx,
            }),
        }
    }
}

impl DrainHandle {
    fn begin_draining(&self, timeout: Duration) -> DrainNotice {
        if let Ok(mut stored) = self.inner.notice.lock() {
            if let Some(notice) = *stored {
                self.inner.draining.store(true, Ordering::SeqCst);
                return notice;
            }
            let notice = drain_notice_for(timeout);
            *stored = Some(notice);
            self.inner.draining.store(true, Ordering::SeqCst);
            return notice;
        }

        let notice = drain_notice_for(timeout);
        self.inner.draining.store(true, Ordering::SeqCst);
        notice
    }

    pub(super) fn is_draining(&self) -> bool {
        self.inner.draining.load(Ordering::SeqCst)
    }

    pub(super) fn notice(&self) -> Option<DrainNotice> {
        self.inner.notice.lock().ok().and_then(|stored| *stored)
    }

    fn active_matches(&self) -> usize {
        self.inner.active_matches.load(Ordering::SeqCst)
    }

    pub(super) fn match_started(&self) {
        let count = self.inner.active_matches.fetch_add(1, Ordering::SeqCst) + 1;
        self.inner.active_matches_tx.send_replace(count);
    }

    pub(super) fn match_finished(&self) {
        let count = self
            .inner
            .active_matches
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                Some(current.saturating_sub(1))
            })
            .map(|previous| previous.saturating_sub(1))
            .unwrap_or(0);
        self.inner.active_matches_tx.send_replace(count);
    }

    async fn wait_for_matches_to_drain(&self) {
        let mut active_matches_rx = self.inner.active_matches_tx.subscribe();
        loop {
            if *active_matches_rx.borrow_and_update() == 0 {
                return;
            }
            if active_matches_rx.changed().await.is_err() {
                return;
            }
        }
    }

    fn request_connection_shutdown(&self) {
        self.inner.connection_shutdown_tx.send_replace(true);
    }

    pub(super) fn subscribe_connection_shutdown(&self) -> watch::Receiver<bool> {
        self.inner.connection_shutdown_tx.subscribe()
    }
}

fn drain_notice_for(timeout: Duration) -> DrainNotice {
    let deadline = SystemTime::now().checked_add(timeout).unwrap_or(UNIX_EPOCH);
    let deadline_unix_ms = deadline
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    DrainNotice {
        deadline_unix_ms,
        seconds_remaining: timeout.as_secs(),
    }
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
    drain: DrainHandle,
}

impl Lobby {
    pub fn new() -> Self {
        Lobby {
            rooms: Arc::new(Mutex::new(HashMap::new())),
            db: None,
            match_history_local_only: false,
            drain: DrainHandle::default(),
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
        self.create_room_locked(room, &mut rooms)
    }

    /// Resolve a join target. During deploy drain, existing rooms stay joinable but new rooms are
    /// rejected so fresh lobbies cannot be created while the process is waiting to exit.
    pub async fn get_or_create_join_target(&self, room: &str) -> Result<RoomHandle, DrainNotice> {
        let mut rooms = self.rooms.lock().await;
        if let Some(handle) = rooms.get(room) {
            return Ok(handle.clone());
        }
        if self.drain.is_draining() {
            return Err(self.drain.notice().unwrap_or(DrainNotice {
                deadline_unix_ms: 0,
                seconds_remaining: 0,
            }));
        }
        Ok(self.create_room_locked(room, &mut rooms))
    }

    fn create_room_locked(
        &self,
        room: &str,
        rooms: &mut HashMap<String, RoomHandle>,
    ) -> RoomHandle {
        let mode = room_mode_for(room);
        self.create_room_locked_with_mode(room, rooms, mode)
    }

    fn create_room_locked_with_mode(
        &self,
        room: &str,
        rooms: &mut HashMap<String, RoomHandle>,
        mode: RoomMode,
    ) -> RoomHandle {
        let (event_tx, event_rx) = mpsc::channel(ROOM_EVENT_CHANNEL_CAP);
        let handle = RoomHandle { event_tx };
        rooms.insert(room.to_string(), handle.clone());

        let name = room.to_string();
        let db = self.db.clone();
        let match_history_local_only = self.match_history_local_only;
        let drain = self.drain.clone();
        tokio::spawn(async move {
            let mut task = RoomTask::new(name.clone(), mode, db, match_history_local_only, drain);
            task.run(event_rx).await;
            crate::log_info!(room = %name, "room task exited");
        });
        crate::log_info!(room = %room, "room created");
        handle
    }

    /// Create an unguessable, spectator-only replay room backed by a persisted artifact.
    pub async fn create_replay_room(&self, artifact: ReplayArtifactV1) -> String {
        let mut rooms = self.rooms.lock().await;
        loop {
            let id = NEXT_MATCH_REPLAY_ROOM_ID.fetch_add(1, Ordering::Relaxed);
            let room = format!("{MATCH_REPLAY_ROOM_PREFIX}:{id:08x}");
            if rooms.contains_key(&room) {
                continue;
            }
            self.create_room_locked_with_mode(
                &room,
                &mut rooms,
                RoomMode::Replay {
                    artifact: artifact.clone(),
                },
            );
            return room;
        }
    }

    /// Create an unguessable room holding frozen state for a future replay practice branch.
    pub async fn create_replay_branch_room(&self, seed: ReplayBranchSeed) -> String {
        let mut rooms = self.rooms.lock().await;
        loop {
            let room = format!(
                "{REPLAY_BRANCH_ROOM_PREFIX}:{:032x}",
                rand::random::<u128>()
            );
            if rooms.contains_key(&room) {
                continue;
            }
            self.create_room_locked_with_mode(
                &room,
                &mut rooms,
                RoomMode::ReplayBranch { seed: seed.clone() },
            );
            return room;
        }
    }

    pub async fn begin_draining(&self, timeout: Duration) {
        let notice = self.drain.begin_draining(timeout);
        let handles: Vec<RoomHandle> = {
            let rooms = self.rooms.lock().await;
            rooms.values().cloned().collect()
        };
        for handle in handles {
            let _ = handle.event_tx.try_send(RoomEvent::DrainStarted(notice));
        }
    }

    pub fn active_match_count(&self) -> usize {
        self.drain.active_matches()
    }

    pub async fn wait_for_matches_to_drain(&self) {
        self.drain.wait_for_matches_to_drain().await;
    }

    pub fn request_connection_shutdown(&self) {
        self.drain.request_connection_shutdown();
    }

    pub fn subscribe_connection_shutdown(&self) -> watch::Receiver<bool> {
        self.drain.subscribe_connection_shutdown()
    }
}

impl Default for Lobby {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
