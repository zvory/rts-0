use std::time::{Duration, Instant as StdInstant};

use rts_sim::game::entity::EntityKind;
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::Game;

use super::super::connection::ConnectionSink;
use super::super::dev_scenario_id::DevScenarioId;
use super::super::projection::ProjectionPolicy;
use super::super::replay_branch::BranchStagingState;
use super::super::replay_session::ReplaySession;
use super::super::session_policy::SessionPolicy;
use super::super::ReplayBranchSeed;
use crate::protocol::{DiagnosticCapabilities, LabMapDraft, TeamId};

/// A connected player as tracked inside a room.
pub(in crate::lobby) struct RoomPlayer {
    pub(in crate::lobby) name: String,
    pub(in crate::lobby) color: String,
    pub(in crate::lobby) ready: bool,
    pub(in crate::lobby) spectator: bool,
    pub(in crate::lobby) msg_tx: ConnectionSink,
    pub(in crate::lobby) head_of_line_count: u32,
    pub(in crate::lobby) last_received_client_seq: u32,
    pub(in crate::lobby) last_sim_consumed_client_seq: u32,
    pub(in crate::lobby) last_sim_consumed_client_tick: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::lobby) struct PendingClientCommandAck {
    pub(in crate::lobby) connection_id: u32,
    pub(in crate::lobby) client_seq: u32,
    pub(in crate::lobby) received_unix_ms: u64,
    pub(in crate::lobby) family: &'static str,
    pub(in crate::lobby) accepted_at: StdInstant,
}

/// A computer opponent seated in a room. Has an id for the lobby list / removal, but no socket -
/// it is materialized into an AI-driven player only when the match starts.
pub(super) struct AiSlot {
    pub(super) id: u32,
    pub(super) team_id: TeamId,
    pub(super) faction_id: String,
    pub(super) profile_id: &'static str,
}

pub(super) const MAX_LOBBY_TEAMS: TeamId = 4;

/// The room's current mode. `InGame` owns the live simulation outright.
pub(super) enum Phase {
    Lobby,
    InGame(Box<Game>),
    ReplayViewer(Box<ReplaySession>),
    BranchStaging(Box<BranchStagingState>),
}

#[derive(Clone, Copy)]
pub(super) struct ReplayTickContext {
    pub(super) scheduler_lag: Duration,
    pub(super) tick_budget: Duration,
    pub(super) tick_start: StdInstant,
    pub(super) projection_policy: ProjectionPolicy,
}

#[derive(Clone, Copy)]
pub(super) enum LabSeekTarget {
    Relative(u32),
    Absolute(u32),
}

#[derive(Clone, Copy)]
pub(super) struct ReplayStartPayloadStamp {
    pub(super) policy: SessionPolicy,
    pub(super) diagnostics: DiagnosticCapabilities,
}

#[derive(Clone)]
pub(in crate::lobby) enum RoomMode {
    Normal,
    DevScenario(DevScenarioConfig),
    Replay { artifact: ReplayArtifactV1 },
    ReplayArtifact { artifact: String },
    ReplayBranch { seed: ReplayBranchSeed },
    Lab(LabRoomConfig),
}

#[derive(Clone)]
pub(in crate::lobby) struct LabRoomConfig {
    pub(in crate::lobby) public_id: String,
    pub(in crate::lobby) map_name: String,
    pub(in crate::lobby) seed: Option<u32>,
    pub(in crate::lobby) scenario: Option<String>,
    pub(in crate::lobby) map_draft: Option<LabMapDraft>,
}

#[derive(Clone)]
pub(in crate::lobby) struct DevScenarioConfig {
    pub(in crate::lobby) id: DevScenarioId,
    pub(in crate::lobby) unit: EntityKind,
    pub(in crate::lobby) count: usize,
    pub(in crate::lobby) blocker: Option<EntityKind>,
    pub(in crate::lobby) case: Option<&'static str>,
}
