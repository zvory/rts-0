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
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use tokio::sync::{mpsc, watch, Mutex, Notify};
use tokio::time::{interval, sleep_until, Instant as TokioInstant, MissedTickBehavior};

use crate::config;
use crate::db::Db;
use crate::protocol::{
    Event, LabClientOp, LabMapDraft, LabReplayArtifactV1, LobbyKind, ReplayBranchSeat,
    ReplayStartMetadata, ResourceDelta, ServerMessage, Snapshot, TeamId, VisionSelectionRequest,
};
use rts_ai::selfplay::is_safe_artifact_name;
use rts_sim::game::command::SimCommand;
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::{Game, PlayerInit};

mod connection;
mod crash_replay;
mod dev_replay;
mod dev_scenario_id;
mod faction_validation;
mod lab_replay_operations;
pub(crate) mod lab_scenario_driver;
mod lab_timeline;
mod launch;
mod live_tick;
mod map_catalog;
mod match_history_writes;
mod participants;
mod projection;
mod reconstruction;
mod replay_branch;
mod replay_session;
mod replay_validation;
mod room_task;
mod session_policy;
mod snapshot_fanout;
mod snapshots;
mod tick_control;

pub use connection::{
    CommandLifecycleExemplarStats, CommandLifecycleReportStats, CommandTimingStats,
    ConnectionReportStats, ConnectionSink, ConnectionWriter, ConnectionWriterStats,
    SnapshotLifecycleReportStats, SnapshotPayloadEntityKindReportStats,
    SnapshotPayloadSectionReportStats, SnapshotWindowStats, SnapshotWriterSendStats,
};
use dev_replay::{load_replay_artifact, room_mode_for};
pub use match_history_writes::MatchHistoryWriteWaitResult;
pub use replay_validation::faction_loadout_incompatibility_reason as replay_faction_loadout_incompatibility_reason;
use room_task::{RoomMode, RoomTask};
pub(crate) use snapshots::compact_snapshot_for_wire;

pub fn load_saved_replay_artifact(name: &str) -> Result<ReplayArtifactV1, String> {
    load_replay_artifact(name)
}

pub fn replay_launch_incompatibility_reason(
    artifact: &ReplayArtifactV1,
    expected_build_sha: &str,
) -> Option<String> {
    replay_session::ReplaySession::validate_artifact_for_launch(artifact, expected_build_sha).err()
}

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
const LOBBY_SUMMARY_TIMEOUT: Duration = Duration::from_millis(75);
const PUBLIC_LOBBY_NAME_MAX_BYTES: usize = 64;
const DEFAULT_DEPLOY_SHUTDOWN_ABORT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_DEPLOY_WRITE_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const DEFAULT_DEPLOY_SHUTDOWN_SLACK: Duration = Duration::from_secs(5);
const DEV_SCENARIO_ROOM_PREFIX: &str = "__dev_scenario__:";
const REPLAY_ARTIFACT_ROOM_PREFIX: &str = "__replay_artifact__:";
const MATCH_REPLAY_ROOM_PREFIX: &str = "__match_replay__";
const REPLAY_BRANCH_ROOM_PREFIX: &str = "__replay_branch__";
const LAB_ROOM_PREFIX: &str = "__lab__:";
const MAP_EDITOR_LAB_ROOM_PREFIX: &str = "__lab__:map-editor-";
const MATCH_SEED_ENV: &str = "RTS_MATCH_SEED";

/// Monotonic source of globally-unique player ids (ids are never reused within a process run).
static NEXT_PLAYER_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_MATCH_REPLAY_ROOM_ID: AtomicU32 = AtomicU32::new(1);

fn pending_create_lease_duration() -> Duration {
    #[cfg(test)]
    {
        Duration::from_millis(25)
    }
    #[cfg(not(test))]
    {
        Duration::from_secs(5)
    }
}

/// Allocate a fresh, process-unique player id. Called once per connection.
pub fn next_player_id() -> u32 {
    NEXT_PLAYER_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandLifecycleFamily {
    Move,
    AttackMove,
    Build,
    Train,
    Other,
}

impl CommandLifecycleFamily {
    pub fn from_protocol_command(cmd: &crate::protocol::Command) -> Self {
        match cmd {
            crate::protocol::Command::Move { .. } => Self::Move,
            crate::protocol::Command::AttackMove { .. } => Self::AttackMove,
            crate::protocol::Command::Build { .. } => Self::Build,
            crate::protocol::Command::Train { .. } => Self::Train,
            _ => Self::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::AttackMove => "attackMove",
            Self::Build => "build",
            Self::Train => "train",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CommandLifecycleTiming {
    pub received_unix_ms: u64,
    pub frame_received_at: Instant,
    pub deserialized_at: Instant,
    pub room_event_enqueued_at: Instant,
    pub family: CommandLifecycleFamily,
}

pub async fn send_command_room_event(
    player_id: u32,
    current_room: &Option<RoomHandle>,
    client_seq: u32,
    cmd: crate::protocol::Command,
    received_unix_ms: u64,
    frame_received_at: Instant,
    deserialized_at: Instant,
) {
    let Some(handle) = current_room else {
        crate::log_debug!(player_id, "ignoring event before join");
        return;
    };

    let family = CommandLifecycleFamily::from_protocol_command(&cmd);
    let cmd = SimCommand::from_protocol(cmd);
    match handle.event_tx.reserve().await {
        Ok(permit) => {
            permit.send(RoomEvent::Command {
                player_id,
                client_seq,
                cmd,
                lifecycle: CommandLifecycleTiming {
                    received_unix_ms,
                    frame_received_at,
                    deserialized_at,
                    room_event_enqueued_at: Instant::now(),
                    family,
                },
            });
        }
        Err(_) => {
            crate::log_warn!(player_id, "room task gone; dropping event");
        }
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
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

/// Browser-safe room state for the first-screen lobby list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbySummary {
    pub room: String,
    pub kind: LobbyKind,
    pub host_name: Option<String>,
    pub map: String,
    pub created_at_unix_ms: u64,
    pub occupied_slots: usize,
    pub max_slots: usize,
    pub spectator_count: usize,
    pub phase: LobbySummaryPhase,
    pub join_state: LobbyJoinState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LobbySummaryPhase {
    Lobby,
    Countdown,
    InGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LobbyJoinState {
    Open,
    FullSpectatorOnly,
    Starting,
    InGame,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeployDrainBudget {
    pub natural_match_drain: Duration,
    pub forced_finalization: Duration,
    pub match_history_write_wait: Duration,
    pub shutdown_slack: Duration,
}

impl DeployDrainBudget {
    pub fn from_total(total: Duration) -> Self {
        let default_reserved = DEFAULT_DEPLOY_SHUTDOWN_ABORT_TIMEOUT
            .saturating_add(DEFAULT_DEPLOY_WRITE_WAIT_TIMEOUT)
            .saturating_add(DEFAULT_DEPLOY_SHUTDOWN_SLACK);
        if total > default_reserved {
            return Self {
                natural_match_drain: total.saturating_sub(default_reserved),
                forced_finalization: DEFAULT_DEPLOY_SHUTDOWN_ABORT_TIMEOUT,
                match_history_write_wait: DEFAULT_DEPLOY_WRITE_WAIT_TIMEOUT,
                shutdown_slack: DEFAULT_DEPLOY_SHUTDOWN_SLACK,
            };
        }

        let unit = total / 5;
        let natural_match_drain = unit * 3;
        let forced_finalization = unit;
        let match_history_write_wait =
            total.saturating_sub(natural_match_drain + forced_finalization);
        Self {
            natural_match_drain,
            forced_finalization,
            match_history_write_wait,
            shutdown_slack: Duration::ZERO,
        }
    }

    pub fn total(self) -> Duration {
        self.natural_match_drain
            .saturating_add(self.forced_finalization)
            .saturating_add(self.match_history_write_wait)
            .saturating_add(self.shutdown_slack)
    }

    fn write_wait_after_elapsed(self, elapsed: Duration) -> Duration {
        let remaining_before_slack = self
            .total()
            .saturating_sub(elapsed)
            .saturating_sub(self.shutdown_slack);
        self.match_history_write_wait.min(remaining_before_slack)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShutdownFinalizeResult {
    pub had_active_match: bool,
    pub finalized_match: bool,
    pub match_history_allowed: bool,
    pub record_queued: bool,
    pub replay_captured: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShutdownFinalizeSummary {
    pub active_matches_before: usize,
    pub active_matches_after: usize,
    pub rooms_requested: usize,
    pub rooms_acked: usize,
    pub rooms_unacked: usize,
    pub send_failed: usize,
    pub finalized_matches: usize,
    pub history_allowed_matches: usize,
    pub records_queued: usize,
    pub replays_captured: usize,
    pub timed_out: bool,
}

impl ShutdownFinalizeSummary {
    fn record(&mut self, result: ShutdownFinalizeResult) {
        if result.finalized_match {
            self.finalized_matches += 1;
        }
        if result.match_history_allowed {
            self.history_allowed_matches += 1;
        }
        if result.record_queued {
            self.records_queued += 1;
        }
        if result.replay_captured {
            self.replays_captured += 1;
        }
    }
}

/// Internal message from a connection (or the lobby) to a room task. The room task is the
/// only consumer; see module docs.
pub enum RoomEvent {
    /// Ask the room task to produce a browser-safe public summary. Internal rooms answer `None`.
    Summary {
        reply: tokio::sync::oneshot::Sender<Option<LobbySummary>>,
    },
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
    /// A connected human updated their display name while waiting in the lobby.
    SetName { player_id: u32, name: String },
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
        lifecycle: CommandLifecycleTiming,
    },
    /// A connected player intentionally gave up the active match.
    GiveUp { player_id: u32 },
    /// A connected active live player requested a live match pause.
    PauseGame { player_id: u32 },
    /// A connected active live player requested live match resume.
    UnpauseGame { player_id: u32 },
    /// A replay viewer asked to leave playback and return their connection to the lobby screen.
    ReturnToLobby { player_id: u32 },
    /// Set room-controlled time speed where the session clock capability allows it.
    SetRoomTimeSpeed { player_id: u32, speed: f32 },
    /// Advance room-controlled time by one simulation tick where the clock allows stepping.
    StepRoomTime { player_id: u32 },
    /// Rewind room-controlled time by `ticks_back` ticks where the clock allows relative seek.
    SeekRoomTime { player_id: u32, ticks_back: u32 },
    /// Seek room-controlled time to an absolute tick where the clock allows absolute seek.
    SeekRoomTimeTo { player_id: u32, tick: u32 },
    /// Select replay fog perspective for this viewer only. Ignored outside replay rooms.
    SetVisionSelection {
        player_id: u32,
        selection: VisionSelectionRequest,
    },
    /// Privileged lab request routed only by lab rooms.
    Lab {
        player_id: u32,
        request_id: u32,
        op: LabClientOp,
    },
    /// Local-development-only replay artifact export. The room remains the authority for the
    /// accepted operation stream and serializes it on its single-owner task.
    LabReplayExport {
        name: Option<String>,
        reply: tokio::sync::oneshot::Sender<Result<LabReplayArtifactV1, String>>,
    },
    /// Local-development-only replay artifact import. Validation and destructive replacement run
    /// on the room's single-owner task, never in an HTTP or daemon task.
    LabReplayImport {
        artifact: Box<LabReplayArtifactV1>,
        deadline: std::time::Instant,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// A replay viewer requested a frozen practice branch seed from the current replay tick.
    RequestBranchFromTick {
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
    AnnounceBranchFromTick {
        branch_room: String,
        source_tick: u32,
        seats: Vec<ReplayBranchSeat>,
    },
    /// Host selects a map by name (lobby phase only; honored only from the host).
    SelectMap { player_id: u32, map: String },
    /// Internal lifecycle probe: when the room is empty, ask the registry to remove this exact
    /// room instance. Future lifecycle policy decides when to send this.
    ReportDisposableIfEmpty,
    /// Process shutdown has begun. Rooms stay alive, but lobby clients should see that starting
    /// another match is disabled while currently-running matches drain.
    DrainStarted(DrainNotice),
    /// Process shutdown is past the natural-drain window. Active authoritative rooms must
    /// finalize their current state for shutdown before WebSocket connections close.
    FinalizeForShutdown {
        ack: tokio::sync::oneshot::Sender<ShutdownFinalizeResult>,
    },
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
    match_history_writes: match_history_writes::MatchHistoryWriteTracker,
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
                match_history_writes: match_history_writes::MatchHistoryWriteTracker::default(),
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

    pub(super) fn track_match_history_write<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.inner.match_history_writes.spawn(future);
    }

    fn pending_match_history_writes(&self) -> usize {
        self.inner.match_history_writes.pending_count()
    }

    async fn wait_for_match_history_writes(
        &self,
        timeout: Duration,
    ) -> MatchHistoryWriteWaitResult {
        self.inner
            .match_history_writes
            .wait_for_pending_at_start(timeout)
            .await
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RoomIdentity(u64);

struct RoomDisposalRequest {
    room: String,
    identity: RoomIdentity,
    ack: Option<tokio::sync::oneshot::Sender<bool>>,
}

#[derive(Clone)]
pub(super) struct RoomLifecycle {
    room: String,
    identity: RoomIdentity,
    disposal_tx: mpsc::UnboundedSender<RoomDisposalRequest>,
}

impl RoomLifecycle {
    fn new(
        room: String,
        identity: RoomIdentity,
        disposal_tx: mpsc::UnboundedSender<RoomDisposalRequest>,
    ) -> Self {
        Self {
            room,
            identity,
            disposal_tx,
        }
    }

    pub(super) fn request_disposal(&self) {
        let _ = self.disposal_tx.send(RoomDisposalRequest {
            room: self.room.clone(),
            identity: self.identity,
            ack: None,
        });
    }
}

/// Handle the lobby keeps for each live room: the channel into its task plus identity/lifecycle
/// metadata used by registry-owned cleanup.
#[derive(Clone)]
pub struct RoomHandle {
    pub event_tx: mpsc::Sender<RoomEvent>,
    identity: RoomIdentity,
    shutdown_tx: watch::Sender<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateLobbyError {
    EmptyName,
    NameTooLong { max_bytes: usize },
    InvalidCharacters,
    ReservedName,
    Draining(DrainNotice),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinTargetError {
    Draining(DrainNotice),
    MissingPrivateRoom,
}

impl CreateLobbyError {
    pub fn message(&self) -> &'static str {
        match self {
            CreateLobbyError::EmptyName => "Lobby name is required.",
            CreateLobbyError::NameTooLong { .. } => "Lobby name is too long.",
            CreateLobbyError::InvalidCharacters => "Lobby name contains unsupported characters.",
            CreateLobbyError::ReservedName => "Lobby name is reserved.",
            CreateLobbyError::Draining(_) => {
                "Server is draining for deploy; new lobbies are disabled."
            }
        }
    }
}

/// Registry of rooms by name. Cheaply cloneable; share one instance across all connections.
#[derive(Clone)]
pub struct Lobby {
    rooms: Arc<Mutex<HashMap<String, RoomHandle>>>,
    disposal_tx: mpsc::UnboundedSender<RoomDisposalRequest>,
    next_room_identity: Arc<AtomicU64>,
    match_history_writer: Option<match_history_writes::SharedMatchHistoryWriter>,
    match_history_local_only: bool,
    drain: DrainHandle,
}

impl Lobby {
    pub fn new() -> Self {
        let rooms = Arc::new(Mutex::new(HashMap::new()));
        let (disposal_tx, disposal_rx) = mpsc::unbounded_channel();
        spawn_room_disposal_task(rooms.clone(), disposal_rx);
        Lobby {
            rooms,
            disposal_tx,
            next_room_identity: Arc::new(AtomicU64::new(1)),
            match_history_writer: None,
            match_history_local_only: false,
            drain: DrainHandle::default(),
        }
    }

    /// Attach a database for match-history persistence. New rooms will inherit it; existing rooms
    /// (none at construction time) are unaffected.
    pub fn with_db(mut self, db: Option<Arc<Db>>) -> Self {
        self.match_history_writer = match_history_writes::writer_from_db(db);
        self.match_history_local_only = false;
        self
    }

    /// Attach a database for match-history persistence with the desired write visibility. New
    /// rooms inherit both the handle and scope; existing rooms are unaffected.
    pub fn with_match_history(mut self, db: Option<Arc<Db>>, local_only: bool) -> Self {
        self.match_history_writer = match_history_writes::writer_from_db(db);
        self.match_history_local_only = local_only;
        self
    }

    #[cfg(test)]
    pub(in crate::lobby) fn with_match_history_writer_for_test(
        mut self,
        writer: Option<match_history_writes::SharedMatchHistoryWriter>,
        local_only: bool,
    ) -> Self {
        self.match_history_writer = writer;
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

    /// Export a replay from an existing Lab room for the environment-gated local artifact bridge.
    pub async fn export_lab_replay_artifact(
        &self,
        room: &str,
        name: Option<String>,
    ) -> Result<LabReplayArtifactV1, String> {
        let handle = self
            .rooms
            .lock()
            .await
            .get(room)
            .cloned()
            .ok_or_else(|| "lab room is not running".to_string())?;
        let (reply, response) = tokio::sync::oneshot::channel();
        handle
            .event_tx
            .send(RoomEvent::LabReplayExport { name, reply })
            .await
            .map_err(|_| "lab room is unavailable".to_string())?;
        tokio::time::timeout(Duration::from_secs(5), response)
            .await
            .map_err(|_| "lab replay export timed out".to_string())?
            .map_err(|_| "lab room closed during replay export".to_string())?
    }

    /// Import a validated replay into an existing Lab room through its single-owner task.
    pub async fn import_lab_replay_artifact(
        &self,
        room: &str,
        artifact: LabReplayArtifactV1,
    ) -> Result<(), String> {
        let handle = self
            .rooms
            .lock()
            .await
            .get(room)
            .cloned()
            .ok_or_else(|| "lab room is not running".to_string())?;
        let (reply, response) = tokio::sync::oneshot::channel();
        handle
            .event_tx
            .send(RoomEvent::LabReplayImport {
                artifact: Box::new(artifact),
                // Leave time for the room reply to traverse the channel before the outer
                // request timeout. Rebuild uses temporary state, so expiry can reject the
                // import without partially replacing the live room.
                deadline: std::time::Instant::now() + Duration::from_secs(4),
                reply,
            })
            .await
            .map_err(|_| "lab room is unavailable".to_string())?;
        tokio::time::timeout(Duration::from_secs(5), response)
            .await
            .map_err(|_| "lab replay import timed out".to_string())?
            .map_err(|_| "lab room closed during replay import".to_string())?
    }

    /// Resolve a join target. During deploy drain, existing rooms stay joinable but new rooms are
    /// rejected so fresh lobbies cannot be created while the process is waiting to exit.
    pub async fn get_or_create_join_target(
        &self,
        room: &str,
    ) -> Result<RoomHandle, JoinTargetError> {
        let mut rooms = self.rooms.lock().await;
        if let Some(handle) = rooms.get(room) {
            return Ok(handle.clone());
        }
        if room.starts_with(MAP_EDITOR_LAB_ROOM_PREFIX) {
            return Err(JoinTargetError::MissingPrivateRoom);
        }
        if self.drain.is_draining() {
            return Err(JoinTargetError::Draining(self.drain.notice().unwrap_or(
                DrainNotice {
                    deadline_unix_ms: 0,
                    seconds_remaining: 0,
                },
            )));
        }
        Ok(self.create_room_locked(room, &mut rooms))
    }

    /// Create a public normal lobby, adding the first available numeric suffix when the requested
    /// name is already reserved. Name selection and reservation happen under the same registry
    /// lock so concurrent browser creates cannot race into the same room.
    pub async fn create_lobby(&self, room: &str) -> Result<String, CreateLobbyError> {
        let requested_room = normalize_public_lobby_name(room)?;
        let mut rooms = self.rooms.lock().await;
        if self.drain.is_draining() {
            return Err(CreateLobbyError::Draining(self.drain.notice().unwrap_or(
                DrainNotice {
                    deadline_unix_ms: 0,
                    seconds_remaining: 0,
                },
            )));
        }
        let room = first_available_public_lobby_name(&requested_room, &rooms);
        let handle = self.create_room_locked_with_mode(&room, &mut rooms, RoomMode::Normal);
        schedule_pending_create_disposal_probe(handle.event_tx.clone());
        Ok(room)
    }

    /// Collect browser rows from room tasks without inspecting room internals or waiting forever
    /// on a stuck room. Dead, busy, timed-out, and internal rooms are omitted.
    pub async fn summaries(&self) -> Vec<LobbySummary> {
        let handles: Vec<RoomHandle> = {
            let rooms = self.rooms.lock().await;
            rooms.values().cloned().collect()
        };
        let requests = handles.into_iter().map(|handle| async move {
            let (reply, response) = tokio::sync::oneshot::channel();
            if handle
                .event_tx
                .try_send(RoomEvent::Summary { reply })
                .is_err()
            {
                return None;
            }
            match tokio::time::timeout(LOBBY_SUMMARY_TIMEOUT, response).await {
                Ok(Ok(summary)) => summary,
                Ok(Err(_)) | Err(_) => None,
            }
        });
        let mut summaries: Vec<LobbySummary> = futures_util::future::join_all(requests)
            .await
            .into_iter()
            .flatten()
            .collect();
        summaries.sort_by(|a, b| {
            lobby_join_sort_rank(a.join_state)
                .cmp(&lobby_join_sort_rank(b.join_state))
                .then_with(|| b.created_at_unix_ms.cmp(&a.created_at_unix_ms))
                .then_with(|| a.room.cmp(&b.room))
        });
        summaries
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
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let identity = RoomIdentity(self.next_room_identity.fetch_add(1, Ordering::Relaxed));
        let handle = RoomHandle {
            event_tx,
            identity,
            shutdown_tx,
        };
        rooms.insert(room.to_string(), handle.clone());

        let name = room.to_string();
        let match_history_writer = self.match_history_writer.clone();
        let match_history_local_only = self.match_history_local_only;
        let drain = self.drain.clone();
        let lifecycle = RoomLifecycle::new(name.clone(), identity, self.disposal_tx.clone());
        tokio::spawn(async move {
            let mut task = RoomTask::new_with_lifecycle(
                name.clone(),
                mode,
                match_history_writer,
                match_history_local_only,
                drain,
                lifecycle,
            );
            task.run(event_rx, shutdown_rx).await;
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

    /// Create a private Lab whose first authoritative start payload is materialized from a
    /// validated Map Editor handoff. The room is registered before its opaque name is returned.
    pub async fn create_map_editor_lab_room(
        &self,
        draft: LabMapDraft,
    ) -> Result<String, DrainNotice> {
        let mut rooms = self.rooms.lock().await;
        if self.drain.is_draining() {
            return Err(self.drain.notice().unwrap_or(DrainNotice {
                deadline_unix_ms: 0,
                seconds_remaining: 0,
            }));
        }
        loop {
            let token = rand::random::<u128>();
            let room = format!("__lab__:map-editor-{token:032x}:map=Chokes");
            if rooms.contains_key(&room) {
                continue;
            }
            let handle = self.create_room_locked_with_mode(
                &room,
                &mut rooms,
                RoomMode::Lab(room_task::LabRoomConfig {
                    public_id: format!("map-editor-{token:032x}"),
                    map_name: "Chokes".to_string(),
                    seed: None,
                    scenario: None,
                    map_draft: Some(draft),
                }),
            );
            schedule_pending_create_disposal_probe(handle.event_tx.clone());
            return Ok(room);
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

    pub async fn run_deploy_drain(&self, timeout: Duration) {
        self.run_deploy_drain_with_budget(DeployDrainBudget::from_total(timeout))
            .await;
    }

    pub async fn run_deploy_drain_with_budget(&self, budget: DeployDrainBudget) {
        let drain_started = Instant::now();
        let timeout = budget.total();
        self.begin_draining(timeout).await;
        let active_matches = self.active_match_count();
        if active_matches == 0 {
            crate::log_info!("shutdown drain complete; no active matches");
            self.wait_for_match_history_writes_during_shutdown(
                budget.write_wait_after_elapsed(drain_started.elapsed()),
            )
            .await;
            self.request_connection_shutdown();
            return;
        }

        crate::log_info!(
            active_matches,
            timeout_secs = timeout.as_secs(),
            natural_drain_secs = budget.natural_match_drain.as_secs(),
            forced_finalize_secs = budget.forced_finalization.as_secs(),
            write_wait_secs = budget.match_history_write_wait.as_secs(),
            shutdown_slack_secs = budget.shutdown_slack.as_secs(),
            "shutdown drain started; waiting for active matches"
        );
        if budget.natural_match_drain.is_zero() {
            crate::log_warn!(
                active_matches = self.active_match_count(),
                "shutdown natural drain skipped; no natural-drain budget"
            );
        } else {
            tokio::select! {
                _ = self.wait_for_matches_to_drain() => {
                    crate::log_info!("shutdown drain natural phase complete; all matches finished");
                }
                _ = tokio::time::sleep(budget.natural_match_drain) => {
                    crate::log_warn!(
                        active_matches = self.active_match_count(),
                        timeout_secs = budget.natural_match_drain.as_secs(),
                        "shutdown natural drain timeout reached; forcing remaining matches"
                    );
                }
            }
        }

        if self.active_match_count() > 0 {
            let summary = self
                .finalize_active_matches_for_shutdown(budget.forced_finalization)
                .await;
            if summary.timed_out || summary.send_failed > 0 || summary.rooms_unacked > 0 {
                crate::log_warn!(
                    active_matches_before = summary.active_matches_before,
                    active_matches_after = summary.active_matches_after,
                    rooms_requested = summary.rooms_requested,
                    rooms_acked = summary.rooms_acked,
                    rooms_unacked = summary.rooms_unacked,
                    send_failed = summary.send_failed,
                    finalized_matches = summary.finalized_matches,
                    records_queued = summary.records_queued,
                    replays_captured = summary.replays_captured,
                    timed_out = summary.timed_out,
                    "shutdown forced finalization incomplete"
                );
            } else {
                crate::log_info!(
                    active_matches_before = summary.active_matches_before,
                    active_matches_after = summary.active_matches_after,
                    rooms_requested = summary.rooms_requested,
                    rooms_acked = summary.rooms_acked,
                    finalized_matches = summary.finalized_matches,
                    history_allowed_matches = summary.history_allowed_matches,
                    records_queued = summary.records_queued,
                    replays_captured = summary.replays_captured,
                    "shutdown forced finalization complete"
                );
            }
        }

        if self.active_match_count() > 0 {
            crate::log_warn!(
                active_matches = self.active_match_count(),
                "shutdown continuing with active matches still tracked after finalization"
            );
        }

        self.wait_for_match_history_writes_during_shutdown(
            budget.write_wait_after_elapsed(drain_started.elapsed()),
        )
        .await;
        self.request_connection_shutdown();
    }

    pub async fn finalize_active_matches_for_shutdown(
        &self,
        timeout: Duration,
    ) -> ShutdownFinalizeSummary {
        let handles: Vec<RoomHandle> = {
            let rooms = self.rooms.lock().await;
            rooms.values().cloned().collect()
        };
        let mut summary = ShutdownFinalizeSummary {
            active_matches_before: self.active_match_count(),
            ..ShutdownFinalizeSummary::default()
        };
        if handles.is_empty() || summary.active_matches_before == 0 {
            summary.active_matches_after = self.active_match_count();
            return summary;
        }

        let mut responses = FuturesUnordered::new();
        for handle in handles {
            let (ack, response) = tokio::sync::oneshot::channel();
            match handle
                .event_tx
                .try_send(RoomEvent::FinalizeForShutdown { ack })
            {
                Ok(()) => {
                    summary.rooms_requested += 1;
                    responses.push(response);
                }
                Err(err) => {
                    summary.send_failed += 1;
                    crate::log_warn!(
                        error = %err,
                        "shutdown forced finalization request could not be queued"
                    );
                }
            }
        }

        if responses.is_empty() {
            summary.active_matches_after = self.active_match_count();
            return summary;
        }

        let deadline = TokioInstant::now() + timeout;
        loop {
            if responses.is_empty() {
                break;
            }
            if timeout.is_zero() {
                summary.timed_out = true;
                break;
            }
            tokio::select! {
                maybe_result = responses.next() => {
                    let Some(result) = maybe_result else {
                        break;
                    };
                    match result {
                        Ok(result) => {
                            summary.rooms_acked += 1;
                            summary.record(result);
                        }
                        Err(_) => {
                            summary.rooms_unacked += 1;
                        }
                    }
                }
                _ = sleep_until(deadline) => {
                    summary.timed_out = true;
                    break;
                }
            }
        }

        summary.rooms_unacked += responses.len();
        summary.active_matches_after = self.active_match_count();
        summary
    }

    pub fn pending_match_history_write_count(&self) -> usize {
        self.drain.pending_match_history_writes()
    }

    pub async fn wait_for_match_history_writes(
        &self,
        timeout: Duration,
    ) -> MatchHistoryWriteWaitResult {
        let result = self.drain.wait_for_match_history_writes(timeout).await;
        if result.timed_out {
            crate::log_warn!(
                initial_pending_writes = result.initial_pending,
                remaining_pending_writes = result.remaining_pending,
                timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                "shutdown match-history write wait timed out"
            );
        } else if result.initial_pending > 0 {
            crate::log_info!(
                completed_writes = result.initial_pending,
                "all match-history writes completed during shutdown"
            );
        }
        result
    }

    pub async fn wait_for_match_history_writes_during_shutdown(&self, timeout: Duration) {
        let pending_writes = self.pending_match_history_write_count();
        if pending_writes == 0 {
            return;
        }
        if timeout.is_zero() {
            crate::log_warn!(
                pending_writes,
                "shutdown match-history write wait skipped; drain deadline exhausted"
            );
            return;
        }
        let _ = self.wait_for_match_history_writes(timeout).await;
    }

    pub fn request_connection_shutdown(&self) {
        self.drain.request_connection_shutdown();
    }

    pub fn subscribe_connection_shutdown(&self) -> watch::Receiver<bool> {
        self.drain.subscribe_connection_shutdown()
    }

    #[cfg(test)]
    async fn request_room_disposal_for_test(&self, room: &str, identity: RoomIdentity) -> bool {
        let (ack, response) = tokio::sync::oneshot::channel();
        self.disposal_tx
            .send(RoomDisposalRequest {
                room: room.to_string(),
                identity,
                ack: Some(ack),
            })
            .expect("lobby disposal task should be running");
        response
            .await
            .expect("lobby disposal task should acknowledge request")
    }
}

impl Default for Lobby {
    fn default() -> Self {
        Self::new()
    }
}

fn lobby_join_sort_rank(state: LobbyJoinState) -> u8 {
    match state {
        LobbyJoinState::Open => 0,
        LobbyJoinState::FullSpectatorOnly => 1,
        LobbyJoinState::Starting => 2,
        LobbyJoinState::InGame => 3,
        LobbyJoinState::Stale => 4,
    }
}

fn spawn_room_disposal_task(
    rooms: Arc<Mutex<HashMap<String, RoomHandle>>>,
    mut disposal_rx: mpsc::UnboundedReceiver<RoomDisposalRequest>,
) {
    tokio::spawn(async move {
        while let Some(request) = disposal_rx.recv().await {
            let removed = remove_room_if_matching(&rooms, &request.room, request.identity).await;
            if let Some(ack) = request.ack {
                let _ = ack.send(removed);
            }
        }
    });
}

fn schedule_pending_create_disposal_probe(event_tx: mpsc::Sender<RoomEvent>) {
    tokio::spawn(async move {
        tokio::time::sleep(pending_create_lease_duration()).await;
        let _ = event_tx.send(RoomEvent::ReportDisposableIfEmpty).await;
    });
}

async fn remove_room_if_matching(
    rooms: &Arc<Mutex<HashMap<String, RoomHandle>>>,
    room: &str,
    identity: RoomIdentity,
) -> bool {
    let mut rooms = rooms.lock().await;
    let Some(handle) = rooms.get(room) else {
        return false;
    };
    if handle.identity != identity {
        return false;
    }

    if let Some(handle) = rooms.remove(room) {
        handle.shutdown_tx.send_replace(true);
        crate::log_info!(room = %room, "room disposed from registry");
        return true;
    }
    false
}

fn normalize_public_lobby_name(raw: &str) -> Result<String, CreateLobbyError> {
    let room = raw.trim();
    if room.is_empty() {
        return Err(CreateLobbyError::EmptyName);
    }
    if room.len() > PUBLIC_LOBBY_NAME_MAX_BYTES {
        return Err(CreateLobbyError::NameTooLong {
            max_bytes: PUBLIC_LOBBY_NAME_MAX_BYTES,
        });
    }
    if room.chars().any(char::is_control) {
        return Err(CreateLobbyError::InvalidCharacters);
    }
    if is_reserved_lobby_name(room) {
        return Err(CreateLobbyError::ReservedName);
    }
    Ok(room.to_string())
}

fn first_available_public_lobby_name(
    requested_room: &str,
    rooms: &HashMap<String, RoomHandle>,
) -> String {
    if !rooms.contains_key(requested_room) {
        return requested_room.to_string();
    }

    let mut sequence = 2usize;
    loop {
        let candidate = numbered_public_lobby_name(requested_room, sequence);
        if !rooms.contains_key(&candidate) {
            return candidate;
        }
        sequence = sequence.saturating_add(1);
    }
}

fn numbered_public_lobby_name(requested_room: &str, sequence: usize) -> String {
    let suffix = format!(" {sequence}");
    let max_prefix_bytes = PUBLIC_LOBBY_NAME_MAX_BYTES.saturating_sub(suffix.len());
    let mut prefix_end = requested_room.len().min(max_prefix_bytes);
    while !requested_room.is_char_boundary(prefix_end) {
        prefix_end = prefix_end.saturating_sub(1);
    }
    let prefix = requested_room[..prefix_end].trim_end();
    format!("{prefix}{suffix}")
}

fn is_reserved_lobby_name(room: &str) -> bool {
    const RESERVED_PREFIXES: [&str; 5] = [
        DEV_SCENARIO_ROOM_PREFIX,
        REPLAY_ARTIFACT_ROOM_PREFIX,
        MATCH_REPLAY_ROOM_PREFIX,
        REPLAY_BRANCH_ROOM_PREFIX,
        LAB_ROOM_PREFIX,
    ];
    RESERVED_PREFIXES
        .iter()
        .any(|prefix| room.starts_with(prefix))
        || !matches!(room_mode_for(room), RoomMode::Normal)
}

#[cfg(test)]
mod tests;
