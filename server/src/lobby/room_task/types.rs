use std::time::{Duration, Instant as StdInstant};

use rts_sim::game::entity::EntityKind;
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::Game;

use super::super::connection::ConnectionSink;
use super::super::projection::ProjectionPolicy;
use super::super::replay_branch::BranchStagingState;
use super::super::replay_session::ReplaySession;
use super::super::session_policy::SessionPolicy;
use super::super::ReplayBranchSeed;
use crate::protocol::{DiagnosticCapabilities, TeamId};

/// A connected player as tracked inside a room.
pub(in crate::lobby) struct RoomPlayer {
    pub(super) name: String,
    pub(in crate::lobby) color: String,
    pub(super) ready: bool,
    pub(in crate::lobby) spectator: bool,
    pub(in crate::lobby) msg_tx: ConnectionSink,
    pub(in crate::lobby) head_of_line_count: u32,
    pub(super) last_received_client_seq: u32,
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
}

#[derive(Clone)]
pub(in crate::lobby) struct DevScenarioConfig {
    pub(in crate::lobby) id: DevScenarioId,
    pub(in crate::lobby) unit: EntityKind,
    pub(in crate::lobby) count: usize,
    pub(in crate::lobby) blocker: Option<EntityKind>,
    pub(in crate::lobby) case: Option<&'static str>,
}

#[derive(Clone)]
pub(in crate::lobby) enum DevScenarioId {
    ScoutCarSnakingCorridor,
    DirectReverseOrder,
    ScoutCarWallChokepoint,
    VehicleCornerWall,
    VehicleSmallBlockBaseline,
    FactoryZeroGapPerpendicular,
    TankTrapLineHorizontal,
    TankTrapLineVertical,
    TankTrapLineDiagonal,
    TankTrapPathingMatrix,
    EntrenchmentInspection,
    PanzerfaustDuel,
    PanzerfaustWindupCancel,
    PanzerfaustTargetDeath,
    PanzerfaustEntrenchedRange,
    PanzerfaustMethamphetamines,
    TankCoaxInspection,
}

impl DevScenarioId {
    pub(in crate::lobby) fn from_room_id(id: &str) -> Option<Self> {
        match id {
            "scout_car_snaking_corridor" => Some(Self::ScoutCarSnakingCorridor),
            "direct_reverse_order" => Some(Self::DirectReverseOrder),
            "scout_car_wall_chokepoint" => Some(Self::ScoutCarWallChokepoint),
            "vehicle_corner_wall" => Some(Self::VehicleCornerWall),
            "vehicle_small_block_baseline" => Some(Self::VehicleSmallBlockBaseline),
            "factory_zero_gap_perpendicular" => Some(Self::FactoryZeroGapPerpendicular),
            "tank_trap_line_horizontal" => Some(Self::TankTrapLineHorizontal),
            "tank_trap_line_vertical" => Some(Self::TankTrapLineVertical),
            "tank_trap_line_diagonal" => Some(Self::TankTrapLineDiagonal),
            "tank_trap_pathing_matrix" => Some(Self::TankTrapPathingMatrix),
            "entrenchment_inspection" => Some(Self::EntrenchmentInspection),
            "panzerfaust_duel" => Some(Self::PanzerfaustDuel),
            "panzerfaust_windup_cancel" => Some(Self::PanzerfaustWindupCancel),
            "panzerfaust_target_death" => Some(Self::PanzerfaustTargetDeath),
            "panzerfaust_entrenched_range" => Some(Self::PanzerfaustEntrenchedRange),
            "panzerfaust_methamphetamines" => Some(Self::PanzerfaustMethamphetamines),
            "tank_coax_inspection" => Some(Self::TankCoaxInspection),
            _ => None,
        }
    }

    pub(in crate::lobby) fn room_id(&self) -> &'static str {
        match self {
            Self::ScoutCarSnakingCorridor => "scout_car_snaking_corridor",
            Self::DirectReverseOrder => "direct_reverse_order",
            Self::ScoutCarWallChokepoint => "scout_car_wall_chokepoint",
            Self::VehicleCornerWall => "vehicle_corner_wall",
            Self::VehicleSmallBlockBaseline => "vehicle_small_block_baseline",
            Self::FactoryZeroGapPerpendicular => "factory_zero_gap_perpendicular",
            Self::TankTrapLineHorizontal => "tank_trap_line_horizontal",
            Self::TankTrapLineVertical => "tank_trap_line_vertical",
            Self::TankTrapLineDiagonal => "tank_trap_line_diagonal",
            Self::TankTrapPathingMatrix => "tank_trap_pathing_matrix",
            Self::EntrenchmentInspection => "entrenchment_inspection",
            Self::PanzerfaustDuel => "panzerfaust_duel",
            Self::PanzerfaustWindupCancel => "panzerfaust_windup_cancel",
            Self::PanzerfaustTargetDeath => "panzerfaust_target_death",
            Self::PanzerfaustEntrenchedRange => "panzerfaust_entrenched_range",
            Self::PanzerfaustMethamphetamines => "panzerfaust_methamphetamines",
            Self::TankCoaxInspection => "tank_coax_inspection",
        }
    }
}
