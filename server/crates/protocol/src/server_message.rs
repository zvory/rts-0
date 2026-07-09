use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerMessage {
    Welcome {
        player_id: u32,
    },
    Lobby {
        room: String,
        kind: LobbyKind,
        host_id: u32,
        players: Vec<LobbyPlayer>,
        can_start: bool,
        team_preset: String,
        /// Currently selected map name.
        map: String,
        /// All available maps (populated from disk at broadcast time).
        maps: Vec<AvailableMap>,
    },
    /// Reliable pre-match countdown shown to every lobby participant before the `start` payload.
    MatchCountdown {
        duration_ms: u32,
        words: Vec<String>,
    },
    /// Match start (flattened: carries StartPayload's fields alongside `"t":"start"`).
    Start(StartPayload),
    /// Per-player, fog-filtered world state.
    Snapshot(Snapshot),
    /// Shared room-controlled time cursor/state. Sent reliably outside snapshot cadence.
    RoomTimeState(RoomTimeState),
    /// Authoritative live-match pause state. Sent reliably after start and on every transition.
    LivePauseState(LivePauseState),
    /// Authoritative observer analysis data for replay viewers and live spectators.
    ObserverAnalysis(ObserverAnalysisPayload),
    /// The requested room is currently replay playback. The client should confirm before retrying
    /// `join` with `replayOk: true`.
    JoinReplayPrompt {
        room: String,
    },
    /// A practice branch room was created from the current replay tick.
    BranchFromTickCreated {
        branch_room: String,
        source_tick: u32,
        seats: Vec<ReplayBranchSeat>,
    },
    /// Current state of a replay branch staging room.
    BranchStaging {
        room: String,
        source_tick: u32,
        host_id: u32,
        seats: Vec<BranchStagingSeat>,
        occupants: Vec<BranchStagingOccupant>,
        can_start: bool,
    },
    /// Reliable lab control-plane state. World state still travels through `snapshot`.
    LabState(LabState),
    /// Reliable result for one lab request.
    LabResult(LabResult),
    /// Server shutdown drain has started. Existing matches may continue until the deadline, but
    /// new match starts are disabled.
    ShutdownWarning {
        deadline_unix_ms: u64,
        seconds_remaining: u64,
    },
    /// A watched all-AI match has resolved; this id retrieves its saved replay and joins logs.
    ObservationReady {
        match_run_id: String,
    },
    GameOver {
        winner_id: Option<u32>,
        winner_team_id: Option<TeamId>,
        /// "won" | "lost" | "draw"
        you: String,
        /// Frozen per-player score snapshot for the score screen.
        scores: Vec<PlayerScore>,
    },
    Pong {
        ts: f64,
    },
    /// Reliable diagnostics-only command receipt. This is not the sim-consumption ack.
    CommandReceipt {
        client_seq: u32,
        server_tick: u32,
        accepted: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Error {
        msg: String,
    },
}
