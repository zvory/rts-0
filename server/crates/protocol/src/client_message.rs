use serde::Deserialize;

use crate::{
    ChatSendPayload, ClientNetReport, Command, LabClientOp, TeamId, VisionSelectionRequest,
};

/// A message accepted from an untrusted connected client.
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
        #[serde(rename = "replayOk")]
        #[serde(default)]
        replay_ok: bool,
    },
    /// Update this connection's display name while it is still in a lobby.
    SetName { name: String },
    /// Toggle ready state in the lobby.
    Ready { ready: bool },
    /// Confirm that this client's renderer finished warming for the active countdown.
    MatchLoadReady {
        #[serde(rename = "countdownId")]
        countdown_id: u32,
    },
    /// Host requests the match to begin.
    Start,
    /// Host selects a lobby team preset (lobby phase only).
    SetTeamPreset { preset: String },
    /// Host assigns one active lobby seat to a nonzero team id (lobby phase only).
    SetTeam {
        id: u32,
        #[serde(rename = "teamId")]
        team_id: TeamId,
    },
    /// A player selects their own playable faction while in the lobby.
    SetFaction {
        #[serde(rename = "factionId")]
        faction_id: String,
    },
    /// Host adds a computer-controlled opponent to the room (lobby phase only).
    AddAi {
        #[serde(rename = "teamId")]
        #[serde(default)]
        team_id: Option<TeamId>,
        #[serde(rename = "aiProfileId")]
        #[serde(default)]
        ai_profile_id: Option<String>,
    },
    /// Host selects the live AI profile for one AI lobby seat.
    SetAiProfile {
        id: u32,
        #[serde(rename = "aiProfileId")]
        ai_profile_id: String,
    },
    /// Host removes a previously-added AI opponent by its player id (lobby phase only).
    RemoveAi { id: u32 },
    /// Switch between player and spectator role while still in the lobby. `id` is optional for
    /// self-targeting compatibility; host-targeted changes include the target human player id.
    SetSpectator {
        spectator: bool,
        #[serde(default)]
        id: Option<u32>,
    },
    /// Send room chat; the room validates and routes its authoritative audience.
    ChatSend(ChatSendPayload),
    /// Issue a gameplay command (ignored unless in-game).
    Command {
        #[serde(rename = "clientSeq")]
        client_seq: u32,
        cmd: Command,
    },
    /// Give up the current match, removing this player's army and showing the score screen.
    GiveUp,
    /// Pause a live match. Honored only from live pause-capable players or spectators with pauses
    /// remaining.
    PauseGame,
    /// Unpause a paused live match. Honored only from live pause-capable players or spectators.
    UnpauseGame,
    /// Leave replay playback and return the room to a clean lobby.
    ReturnToLobby,
    /// Latency probe.
    Ping { ts: f64 },
    /// Client-observed network/render health aggregate for server logs.
    NetReport { report: Box<ClientNetReport> },
    /// Throttled notice that the connected browser received human input. This is distinct from
    /// automatic heartbeat and diagnostics traffic so the server can expire abandoned sessions.
    Activity,
    /// Request durable ground decals learned since this recipient-scoped revision.
    RequestGroundDecals {
        #[serde(rename = "requestId")]
        request_id: u32,
        #[serde(rename = "afterRevision")]
        after_revision: u32,
    },
    /// Set room-controlled time speed. `0` pauses rooms whose clock supports pause.
    SetRoomTimeSpeed { speed: f32 },
    /// Advance room-controlled time by one simulation tick where the clock allows stepping.
    StepRoomTime,
    /// Rewind room-controlled time by `ticks_back` ticks where relative seek is allowed.
    SeekRoomTime {
        #[serde(rename = "ticksBack")]
        ticks_back: u32,
    },
    /// Seek room-controlled time to an absolute simulation tick where absolute seek is allowed.
    SeekRoomTimeTo { tick: u32 },
    /// Select which players' fog to use while viewing a replay. Per-viewer only.
    SetVisionSelection { selection: VisionSelectionRequest },
    /// Privileged lab request envelope. Only lab rooms route these requests.
    Lab {
        #[serde(rename = "requestId")]
        request_id: u32,
        op: Box<LabClientOp>,
    },
    /// Request a new practice branch room from this replay room's current authoritative tick.
    RequestBranchFromTick,
    /// Claim one original player seat in a replay branch staging room.
    ClaimBranchSeat {
        #[serde(rename = "playerId")]
        player_id: u32,
    },
    /// Release one original player seat in a replay branch staging room.
    ReleaseBranchSeat {
        #[serde(rename = "playerId")]
        player_id: u32,
    },
    /// Host asks to launch a replay branch from staging.
    StartBranch,
    /// Host selects a map by name (lobby phase only; ignored from non-hosts).
    SelectMap { map: String },
}
