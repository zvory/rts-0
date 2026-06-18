use super::room_task::RoomMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionMode {
    Normal,
    DevScenario,
    Replay,
    ReplayArtifact,
    ReplayBranch,
    Lab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionPhase {
    Lobby,
    LiveMatch,
    ReplayViewer,
    BranchStaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StateSource {
    LobbyState,
    LiveGame,
    PostMatchReplaySession,
    PersistedReplayArtifact,
    SavedReplayArtifact,
    ReplayBranchSeed,
    BranchLiveGame,
    DevScenario,
    LabGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum JoinPolicy {
    NormalLobby,
    RejectMidMatch,
    ReplayPromptOrAttach,
    BranchStaging,
    BranchLiveAttach,
    DevWatch,
    LabRoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClockTickSource {
    RoomTicker,
    LiveMatch,
    BranchStaging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RoomTimeSource {
    ReplayPlayback,
    DevScenario,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RoomTimeOperation {
    SetSpeed,
    Step,
    SeekRelative,
    SeekAbsolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RoomTimeOperations {
    set_speed: bool,
    step: bool,
    seek_relative: bool,
    seek_absolute: bool,
}

impl RoomTimeOperations {
    pub(super) const NONE: Self = Self {
        set_speed: false,
        step: false,
        seek_relative: false,
        seek_absolute: false,
    };

    pub(super) const REPLAY_PLAYBACK: Self = Self {
        set_speed: true,
        step: false,
        seek_relative: true,
        seek_absolute: true,
    };

    pub(super) const DEV_SCENARIO: Self = Self {
        set_speed: true,
        step: true,
        seek_relative: false,
        seek_absolute: false,
    };

    pub(super) fn allows(self, operation: RoomTimeOperation) -> bool {
        match operation {
            RoomTimeOperation::SetSpeed => self.set_speed,
            RoomTimeOperation::Step => self.step,
            RoomTimeOperation::SeekRelative => self.seek_relative,
            RoomTimeOperation::SeekAbsolute => self.seek_absolute,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RoomTimeCapability {
    pub(super) source: RoomTimeSource,
    pub(super) operations: RoomTimeOperations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClockCapability {
    FixedRealtime(ClockTickSource),
    RoomControlled(RoomTimeCapability),
}

impl ClockCapability {
    pub(super) const ROOM_TICKER: Self = Self::FixedRealtime(ClockTickSource::RoomTicker);
    pub(super) const LIVE_MATCH: Self = Self::FixedRealtime(ClockTickSource::LiveMatch);
    pub(super) const BRANCH_STAGING: Self = Self::FixedRealtime(ClockTickSource::BranchStaging);
    pub(super) const REPLAY_PLAYBACK: Self = Self::RoomControlled(RoomTimeCapability {
        source: RoomTimeSource::ReplayPlayback,
        operations: RoomTimeOperations::REPLAY_PLAYBACK,
    });
    pub(super) const DEV_SCENARIO: Self = Self::RoomControlled(RoomTimeCapability {
        source: RoomTimeSource::DevScenario,
        operations: RoomTimeOperations::DEV_SCENARIO,
    });

    pub(super) fn room_time_source(self) -> Option<RoomTimeSource> {
        match self {
            ClockCapability::RoomControlled(capability) => Some(capability.source),
            ClockCapability::FixedRealtime(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthorityPolicy {
    LobbyHost,
    LivePlayers,
    ReplayViewers,
    BranchStagingHost,
    BranchLiveSeatAliases,
    DevWatchControls,
    LabOperator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MutationPolicy {
    None,
    LobbyState,
    LiveGameplayCommands,
    ReplayPlaybackCursor,
    ReplayBranchStagingClaims,
    BranchLiveSeatAliasGameplay,
    DevScenarioDriver,
    LabPrivilegedOps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VisibilityPolicy {
    LobbyState,
    LiveFog,
    ReplayVision,
    BranchStagingState,
    DevFullWorld,
    LabFullWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObserverAnalysisPolicy {
    None,
    LiveSpectators,
    ReplayViewers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MovementPathDiagnosticPolicy {
    None,
    OwnerOnly,
    AllProjected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DiagnosticPolicy {
    pub(super) observer_analysis: ObserverAnalysisPolicy,
    pub(super) movement_paths: MovementPathDiagnosticPolicy,
}

impl DiagnosticPolicy {
    pub(super) const NONE: Self = Self {
        observer_analysis: ObserverAnalysisPolicy::None,
        movement_paths: MovementPathDiagnosticPolicy::None,
    };

    pub(super) const LIVE_SPECTATOR_OBSERVER_ANALYSIS: Self = Self {
        observer_analysis: ObserverAnalysisPolicy::LiveSpectators,
        movement_paths: MovementPathDiagnosticPolicy::None,
    };

    pub(super) const REPLAY_OBSERVER_ANALYSIS: Self = Self {
        observer_analysis: ObserverAnalysisPolicy::ReplayViewers,
        movement_paths: MovementPathDiagnosticPolicy::None,
    };

    pub(super) const DEV_MOVEMENT_PATHS: Self = Self {
        observer_analysis: ObserverAnalysisPolicy::None,
        movement_paths: MovementPathDiagnosticPolicy::AllProjected,
    };

    pub(super) fn with_owner_movement_paths(self) -> Self {
        Self {
            movement_paths: MovementPathDiagnosticPolicy::OwnerOnly,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PersistencePolicy {
    MatchHistoryAndReplayArtifacts,
    Suppressed,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExportPolicy {
    None,
    LabScenario,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StartPayloadPolicy {
    None,
    LiveMatch,
    ReplayViewer,
    ReplayBranchLive,
    DevWatch,
    Lab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AffordancePolicy {
    Lobby,
    LiveMatch,
    ReplayViewer,
    BranchStaging,
    BranchLive,
    DevWatch,
    Lab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SessionPolicy {
    pub(super) mode: SessionMode,
    pub(super) phase: SessionPhase,
    pub(super) state_source: StateSource,
    pub(super) join: JoinPolicy,
    pub(super) clock: ClockCapability,
    pub(super) authority: AuthorityPolicy,
    pub(super) mutation: MutationPolicy,
    pub(super) visibility: VisibilityPolicy,
    pub(super) diagnostics: DiagnosticPolicy,
    pub(super) export: ExportPolicy,
    pub(super) affordance: AffordancePolicy,
    pub(super) persistence: PersistencePolicy,
    pub(super) start_payload: StartPayloadPolicy,
    pub(super) countdown_eligible: bool,
}

impl SessionPolicy {
    pub(super) fn new(mode: SessionMode, phase: SessionPhase) -> Self {
        let mut policy = match phase {
            SessionPhase::Lobby => Self {
                mode,
                phase,
                state_source: StateSource::LobbyState,
                join: JoinPolicy::NormalLobby,
                clock: ClockCapability::ROOM_TICKER,
                authority: AuthorityPolicy::LobbyHost,
                mutation: MutationPolicy::LobbyState,
                visibility: VisibilityPolicy::LobbyState,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::Lobby,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::None,
                countdown_eligible: true,
            },
            SessionPhase::LiveMatch => Self {
                mode,
                phase,
                state_source: StateSource::LiveGame,
                join: JoinPolicy::RejectMidMatch,
                clock: ClockCapability::LIVE_MATCH,
                authority: AuthorityPolicy::LivePlayers,
                mutation: MutationPolicy::LiveGameplayCommands,
                visibility: VisibilityPolicy::LiveFog,
                diagnostics: DiagnosticPolicy::LIVE_SPECTATOR_OBSERVER_ANALYSIS,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::LiveMatch,
                persistence: PersistencePolicy::MatchHistoryAndReplayArtifacts,
                start_payload: StartPayloadPolicy::LiveMatch,
                countdown_eligible: false,
            },
            SessionPhase::ReplayViewer => Self {
                mode,
                phase,
                state_source: StateSource::PostMatchReplaySession,
                join: JoinPolicy::ReplayPromptOrAttach,
                clock: ClockCapability::REPLAY_PLAYBACK,
                authority: AuthorityPolicy::ReplayViewers,
                mutation: MutationPolicy::ReplayPlaybackCursor,
                visibility: VisibilityPolicy::ReplayVision,
                diagnostics: DiagnosticPolicy::REPLAY_OBSERVER_ANALYSIS,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::ReplayViewer,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::ReplayViewer,
                countdown_eligible: false,
            },
            SessionPhase::BranchStaging => Self {
                mode,
                phase,
                state_source: StateSource::ReplayBranchSeed,
                join: JoinPolicy::BranchStaging,
                clock: ClockCapability::BRANCH_STAGING,
                authority: AuthorityPolicy::BranchStagingHost,
                mutation: MutationPolicy::ReplayBranchStagingClaims,
                visibility: VisibilityPolicy::BranchStagingState,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::BranchStaging,
                persistence: PersistencePolicy::Suppressed,
                start_payload: StartPayloadPolicy::None,
                countdown_eligible: true,
            },
        };

        match mode {
            SessionMode::Normal => {}
            SessionMode::DevScenario => {
                policy.state_source = StateSource::DevScenario;
                policy.join = JoinPolicy::DevWatch;
                policy.clock = ClockCapability::DEV_SCENARIO;
                policy.authority = AuthorityPolicy::DevWatchControls;
                policy.mutation = MutationPolicy::DevScenarioDriver;
                policy.visibility = VisibilityPolicy::DevFullWorld;
                policy.diagnostics = DiagnosticPolicy::DEV_MOVEMENT_PATHS;
                policy.export = ExportPolicy::None;
                policy.affordance = AffordancePolicy::DevWatch;
                policy.persistence = PersistencePolicy::Suppressed;
                policy.start_payload = StartPayloadPolicy::DevWatch;
                policy.countdown_eligible = false;
            }
            SessionMode::Replay => {
                if phase == SessionPhase::Lobby {
                    policy.state_source = StateSource::PersistedReplayArtifact;
                    policy.clock = ClockCapability::ROOM_TICKER;
                    policy.mutation = MutationPolicy::None;
                    policy.visibility = VisibilityPolicy::LobbyState;
                    policy.diagnostics = DiagnosticPolicy::NONE;
                }
                policy.join = JoinPolicy::ReplayPromptOrAttach;
                policy.authority = AuthorityPolicy::ReplayViewers;
                policy.export = ExportPolicy::None;
                policy.affordance = AffordancePolicy::ReplayViewer;
                policy.persistence = PersistencePolicy::None;
                policy.start_payload = StartPayloadPolicy::ReplayViewer;
                policy.countdown_eligible = false;
            }
            SessionMode::ReplayArtifact => {
                if phase == SessionPhase::Lobby {
                    policy.state_source = StateSource::SavedReplayArtifact;
                    policy.clock = ClockCapability::ROOM_TICKER;
                    policy.mutation = MutationPolicy::None;
                    policy.visibility = VisibilityPolicy::LobbyState;
                    policy.diagnostics = DiagnosticPolicy::NONE;
                }
                policy.join = JoinPolicy::ReplayPromptOrAttach;
                policy.authority = AuthorityPolicy::ReplayViewers;
                policy.export = ExportPolicy::None;
                policy.affordance = AffordancePolicy::ReplayViewer;
                policy.persistence = PersistencePolicy::None;
                policy.start_payload = StartPayloadPolicy::ReplayViewer;
                policy.countdown_eligible = false;
            }
            SessionMode::ReplayBranch => {
                policy.state_source = match phase {
                    SessionPhase::LiveMatch => StateSource::BranchLiveGame,
                    _ => StateSource::ReplayBranchSeed,
                };
                policy.join = match phase {
                    SessionPhase::LiveMatch => JoinPolicy::BranchLiveAttach,
                    _ => JoinPolicy::BranchStaging,
                };
                policy.clock = match phase {
                    SessionPhase::LiveMatch => ClockCapability::LIVE_MATCH,
                    _ => ClockCapability::BRANCH_STAGING,
                };
                policy.authority = match phase {
                    SessionPhase::LiveMatch => AuthorityPolicy::BranchLiveSeatAliases,
                    _ => AuthorityPolicy::BranchStagingHost,
                };
                policy.mutation = match phase {
                    SessionPhase::LiveMatch => MutationPolicy::BranchLiveSeatAliasGameplay,
                    _ => MutationPolicy::ReplayBranchStagingClaims,
                };
                policy.visibility = match phase {
                    SessionPhase::LiveMatch => VisibilityPolicy::LiveFog,
                    _ => VisibilityPolicy::BranchStagingState,
                };
                policy.diagnostics = match phase {
                    SessionPhase::LiveMatch => DiagnosticPolicy::LIVE_SPECTATOR_OBSERVER_ANALYSIS,
                    _ => DiagnosticPolicy::NONE,
                };
                policy.export = ExportPolicy::None;
                policy.affordance = match phase {
                    SessionPhase::LiveMatch => AffordancePolicy::BranchLive,
                    _ => AffordancePolicy::BranchStaging,
                };
                policy.persistence = PersistencePolicy::Suppressed;
                policy.start_payload = match phase {
                    SessionPhase::LiveMatch => StartPayloadPolicy::ReplayBranchLive,
                    _ => StartPayloadPolicy::None,
                };
                policy.countdown_eligible = phase == SessionPhase::BranchStaging;
            }
            SessionMode::Lab => {
                policy.state_source = StateSource::LabGame;
                policy.join = JoinPolicy::LabRoom;
                policy.clock = ClockCapability::LIVE_MATCH;
                policy.authority = AuthorityPolicy::LabOperator;
                policy.mutation = MutationPolicy::LabPrivilegedOps;
                policy.visibility = VisibilityPolicy::LabFullWorld;
                policy.diagnostics = DiagnosticPolicy::NONE;
                policy.export = ExportPolicy::LabScenario;
                policy.affordance = AffordancePolicy::Lab;
                policy.persistence = PersistencePolicy::Suppressed;
                policy.start_payload = StartPayloadPolicy::Lab;
                policy.countdown_eligible = false;
            }
        }

        policy
    }

    pub(super) fn for_room(mode: &RoomMode, phase: SessionPhase) -> Self {
        Self::new(SessionMode::from(mode), phase)
    }

    pub(super) fn is_dev_watch(self) -> bool {
        self.join == JoinPolicy::DevWatch
    }

    pub(super) fn uses_replay_room_join(self) -> bool {
        self.join == JoinPolicy::ReplayPromptOrAttach
    }

    pub(super) fn uses_branch_room_join(self) -> bool {
        matches!(
            self.join,
            JoinPolicy::BranchStaging | JoinPolicy::BranchLiveAttach
        )
    }

    pub(super) fn uses_lab_room_join(self) -> bool {
        self.join == JoinPolicy::LabRoom
    }

    pub(super) fn allows_match_history(self) -> bool {
        self.persistence == PersistencePolicy::MatchHistoryAndReplayArtifacts
    }

    pub(super) fn has_authoritative_mutation(self) -> bool {
        !matches!(self.mutation, MutationPolicy::None)
    }
}

impl From<&RoomMode> for SessionMode {
    fn from(mode: &RoomMode) -> Self {
        match mode {
            RoomMode::Normal => Self::Normal,
            RoomMode::DevScenario(_) => Self::DevScenario,
            RoomMode::Replay { .. } => Self::Replay,
            RoomMode::ReplayArtifact { .. } => Self::ReplayArtifact,
            RoomMode::ReplayBranch { .. } => Self::ReplayBranch,
            RoomMode::Lab(_) => Self::Lab,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CapabilityCase {
        product_path: &'static str,
        mode: SessionMode,
        phase: SessionPhase,
        state_source: StateSource,
        join: JoinPolicy,
        clock: ClockCapability,
        authority: AuthorityPolicy,
        visibility: VisibilityPolicy,
        mutation: MutationPolicy,
        diagnostics: DiagnosticPolicy,
        export: ExportPolicy,
        affordance: AffordancePolicy,
        persistence: PersistencePolicy,
        start_payload: StartPayloadPolicy,
        countdown_eligible: bool,
    }

    fn assert_capability_case(case: CapabilityCase) {
        let policy = SessionPolicy::new(case.mode, case.phase);
        assert_eq!(
            policy.state_source, case.state_source,
            "{} state source",
            case.product_path
        );
        assert_eq!(policy.join, case.join, "{} join", case.product_path);
        assert_eq!(policy.clock, case.clock, "{} clock", case.product_path);
        assert_eq!(
            policy.authority, case.authority,
            "{} authority",
            case.product_path
        );
        assert_eq!(
            policy.visibility, case.visibility,
            "{} visibility",
            case.product_path
        );
        assert_eq!(
            policy.mutation, case.mutation,
            "{} mutation",
            case.product_path
        );
        assert_eq!(
            policy.diagnostics, case.diagnostics,
            "{} diagnostics",
            case.product_path
        );
        assert_eq!(policy.export, case.export, "{} export", case.product_path);
        assert_eq!(
            policy.affordance, case.affordance,
            "{} affordance",
            case.product_path
        );
        assert_eq!(
            policy.persistence, case.persistence,
            "{} persistence",
            case.product_path
        );
        assert_eq!(
            policy.start_payload, case.start_payload,
            "{} start payload",
            case.product_path
        );
        assert_eq!(
            policy.countdown_eligible, case.countdown_eligible,
            "{} countdown eligibility",
            case.product_path
        );
    }

    #[test]
    fn session_policy_capability_baseline_covers_room2_product_paths() {
        for case in [
            CapabilityCase {
                product_path: "normal lobby",
                mode: SessionMode::Normal,
                phase: SessionPhase::Lobby,
                state_source: StateSource::LobbyState,
                join: JoinPolicy::NormalLobby,
                clock: ClockCapability::ROOM_TICKER,
                authority: AuthorityPolicy::LobbyHost,
                visibility: VisibilityPolicy::LobbyState,
                mutation: MutationPolicy::LobbyState,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::Lobby,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::None,
                countdown_eligible: true,
            },
            CapabilityCase {
                product_path: "normal live match",
                mode: SessionMode::Normal,
                phase: SessionPhase::LiveMatch,
                state_source: StateSource::LiveGame,
                join: JoinPolicy::RejectMidMatch,
                clock: ClockCapability::LIVE_MATCH,
                authority: AuthorityPolicy::LivePlayers,
                visibility: VisibilityPolicy::LiveFog,
                mutation: MutationPolicy::LiveGameplayCommands,
                diagnostics: DiagnosticPolicy::LIVE_SPECTATOR_OBSERVER_ANALYSIS,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::LiveMatch,
                persistence: PersistencePolicy::MatchHistoryAndReplayArtifacts,
                start_payload: StartPayloadPolicy::LiveMatch,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "live spectator",
                mode: SessionMode::Normal,
                phase: SessionPhase::LiveMatch,
                state_source: StateSource::LiveGame,
                join: JoinPolicy::RejectMidMatch,
                clock: ClockCapability::LIVE_MATCH,
                authority: AuthorityPolicy::LivePlayers,
                visibility: VisibilityPolicy::LiveFog,
                mutation: MutationPolicy::LiveGameplayCommands,
                diagnostics: DiagnosticPolicy::LIVE_SPECTATOR_OBSERVER_ANALYSIS,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::LiveMatch,
                persistence: PersistencePolicy::MatchHistoryAndReplayArtifacts,
                start_payload: StartPayloadPolicy::LiveMatch,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "post-match replay",
                mode: SessionMode::Normal,
                phase: SessionPhase::ReplayViewer,
                state_source: StateSource::PostMatchReplaySession,
                join: JoinPolicy::ReplayPromptOrAttach,
                clock: ClockCapability::REPLAY_PLAYBACK,
                authority: AuthorityPolicy::ReplayViewers,
                visibility: VisibilityPolicy::ReplayVision,
                mutation: MutationPolicy::ReplayPlaybackCursor,
                diagnostics: DiagnosticPolicy::REPLAY_OBSERVER_ANALYSIS,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::ReplayViewer,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::ReplayViewer,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "persisted replay room",
                mode: SessionMode::Replay,
                phase: SessionPhase::Lobby,
                state_source: StateSource::PersistedReplayArtifact,
                join: JoinPolicy::ReplayPromptOrAttach,
                clock: ClockCapability::ROOM_TICKER,
                authority: AuthorityPolicy::ReplayViewers,
                visibility: VisibilityPolicy::LobbyState,
                mutation: MutationPolicy::None,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::ReplayViewer,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::ReplayViewer,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "saved replay artifact",
                mode: SessionMode::ReplayArtifact,
                phase: SessionPhase::Lobby,
                state_source: StateSource::SavedReplayArtifact,
                join: JoinPolicy::ReplayPromptOrAttach,
                clock: ClockCapability::ROOM_TICKER,
                authority: AuthorityPolicy::ReplayViewers,
                visibility: VisibilityPolicy::LobbyState,
                mutation: MutationPolicy::None,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::ReplayViewer,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::ReplayViewer,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "replay branch staging",
                mode: SessionMode::ReplayBranch,
                phase: SessionPhase::BranchStaging,
                state_source: StateSource::ReplayBranchSeed,
                join: JoinPolicy::BranchStaging,
                clock: ClockCapability::BRANCH_STAGING,
                authority: AuthorityPolicy::BranchStagingHost,
                visibility: VisibilityPolicy::BranchStagingState,
                mutation: MutationPolicy::ReplayBranchStagingClaims,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::BranchStaging,
                persistence: PersistencePolicy::Suppressed,
                start_payload: StartPayloadPolicy::None,
                countdown_eligible: true,
            },
            CapabilityCase {
                product_path: "replay branch live",
                mode: SessionMode::ReplayBranch,
                phase: SessionPhase::LiveMatch,
                state_source: StateSource::BranchLiveGame,
                join: JoinPolicy::BranchLiveAttach,
                clock: ClockCapability::LIVE_MATCH,
                authority: AuthorityPolicy::BranchLiveSeatAliases,
                visibility: VisibilityPolicy::LiveFog,
                mutation: MutationPolicy::BranchLiveSeatAliasGameplay,
                diagnostics: DiagnosticPolicy::LIVE_SPECTATOR_OBSERVER_ANALYSIS,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::BranchLive,
                persistence: PersistencePolicy::Suppressed,
                start_payload: StartPayloadPolicy::ReplayBranchLive,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "dev scenario",
                mode: SessionMode::DevScenario,
                phase: SessionPhase::LiveMatch,
                state_source: StateSource::DevScenario,
                join: JoinPolicy::DevWatch,
                clock: ClockCapability::DEV_SCENARIO,
                authority: AuthorityPolicy::DevWatchControls,
                visibility: VisibilityPolicy::DevFullWorld,
                mutation: MutationPolicy::DevScenarioDriver,
                diagnostics: DiagnosticPolicy::DEV_MOVEMENT_PATHS,
                export: ExportPolicy::None,
                affordance: AffordancePolicy::DevWatch,
                persistence: PersistencePolicy::Suppressed,
                start_payload: StartPayloadPolicy::DevWatch,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "lab operator",
                mode: SessionMode::Lab,
                phase: SessionPhase::LiveMatch,
                state_source: StateSource::LabGame,
                join: JoinPolicy::LabRoom,
                clock: ClockCapability::LIVE_MATCH,
                authority: AuthorityPolicy::LabOperator,
                visibility: VisibilityPolicy::LabFullWorld,
                mutation: MutationPolicy::LabPrivilegedOps,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::LabScenario,
                affordance: AffordancePolicy::Lab,
                persistence: PersistencePolicy::Suppressed,
                start_payload: StartPayloadPolicy::Lab,
                countdown_eligible: false,
            },
            CapabilityCase {
                product_path: "lab read-only viewer",
                mode: SessionMode::Lab,
                phase: SessionPhase::LiveMatch,
                state_source: StateSource::LabGame,
                join: JoinPolicy::LabRoom,
                clock: ClockCapability::LIVE_MATCH,
                authority: AuthorityPolicy::LabOperator,
                visibility: VisibilityPolicy::LabFullWorld,
                mutation: MutationPolicy::LabPrivilegedOps,
                diagnostics: DiagnosticPolicy::NONE,
                export: ExportPolicy::LabScenario,
                affordance: AffordancePolicy::Lab,
                persistence: PersistencePolicy::Suppressed,
                start_payload: StartPayloadPolicy::Lab,
                countdown_eligible: false,
            },
        ] {
            assert_capability_case(case);
        }
    }

    #[test]
    fn session_policy_classifies_normal_lobby_live_and_post_match_replay() {
        let lobby = SessionPolicy::new(SessionMode::Normal, SessionPhase::Lobby);
        assert_eq!(lobby.state_source, StateSource::LobbyState);
        assert_eq!(lobby.join, JoinPolicy::NormalLobby);
        assert_eq!(lobby.clock, ClockCapability::ROOM_TICKER);
        assert_eq!(lobby.authority, AuthorityPolicy::LobbyHost);
        assert_eq!(lobby.visibility, VisibilityPolicy::LobbyState);
        assert_eq!(lobby.mutation, MutationPolicy::LobbyState);
        assert_eq!(lobby.persistence, PersistencePolicy::None);
        assert_eq!(lobby.start_payload, StartPayloadPolicy::None);
        assert!(lobby.countdown_eligible);

        let live = SessionPolicy::new(SessionMode::Normal, SessionPhase::LiveMatch);
        assert_eq!(live.state_source, StateSource::LiveGame);
        assert_eq!(live.join, JoinPolicy::RejectMidMatch);
        assert_eq!(live.clock, ClockCapability::LIVE_MATCH);
        assert_eq!(live.authority, AuthorityPolicy::LivePlayers);
        assert_eq!(live.visibility, VisibilityPolicy::LiveFog);
        assert_eq!(live.mutation, MutationPolicy::LiveGameplayCommands);
        assert_eq!(
            live.persistence,
            PersistencePolicy::MatchHistoryAndReplayArtifacts
        );
        assert_eq!(live.start_payload, StartPayloadPolicy::LiveMatch);
        assert!(!live.countdown_eligible);
        assert!(live.allows_match_history());

        let replay = SessionPolicy::new(SessionMode::Normal, SessionPhase::ReplayViewer);
        assert_eq!(replay.state_source, StateSource::PostMatchReplaySession);
        assert_eq!(replay.join, JoinPolicy::ReplayPromptOrAttach);
        assert_eq!(replay.clock, ClockCapability::REPLAY_PLAYBACK);
        assert_eq!(replay.authority, AuthorityPolicy::ReplayViewers);
        assert_eq!(replay.visibility, VisibilityPolicy::ReplayVision);
        assert_eq!(replay.mutation, MutationPolicy::ReplayPlaybackCursor);
        assert_eq!(replay.persistence, PersistencePolicy::None);
        assert_eq!(replay.start_payload, StartPayloadPolicy::ReplayViewer);
        assert!(!replay.countdown_eligible);
        assert!(!replay.allows_match_history());
        assert!(replay.has_authoritative_mutation());
    }

    #[test]
    fn session_policy_classifies_dedicated_replay_rooms() {
        let persisted = SessionPolicy::new(SessionMode::Replay, SessionPhase::Lobby);
        assert_eq!(persisted.state_source, StateSource::PersistedReplayArtifact);
        assert_eq!(persisted.join, JoinPolicy::ReplayPromptOrAttach);
        assert_eq!(persisted.clock, ClockCapability::ROOM_TICKER);
        assert_eq!(persisted.authority, AuthorityPolicy::ReplayViewers);
        assert_eq!(persisted.visibility, VisibilityPolicy::LobbyState);
        assert_eq!(persisted.mutation, MutationPolicy::None);
        assert_eq!(persisted.persistence, PersistencePolicy::None);
        assert_eq!(persisted.start_payload, StartPayloadPolicy::ReplayViewer);
        assert!(!persisted.countdown_eligible);
        assert!(!persisted.allows_match_history());
        assert!(!persisted.has_authoritative_mutation());

        let saved = SessionPolicy::new(SessionMode::ReplayArtifact, SessionPhase::Lobby);
        assert_eq!(saved.state_source, StateSource::SavedReplayArtifact);
        assert_eq!(saved.join, JoinPolicy::ReplayPromptOrAttach);
        assert_eq!(saved.clock, ClockCapability::ROOM_TICKER);
        assert_eq!(saved.authority, AuthorityPolicy::ReplayViewers);
        assert_eq!(saved.visibility, VisibilityPolicy::LobbyState);
        assert_eq!(saved.mutation, MutationPolicy::None);
        assert_eq!(saved.persistence, PersistencePolicy::None);
        assert_eq!(saved.start_payload, StartPayloadPolicy::ReplayViewer);
        assert!(!saved.countdown_eligible);

        let playing = SessionPolicy::new(SessionMode::Replay, SessionPhase::ReplayViewer);
        assert_eq!(playing.state_source, StateSource::PostMatchReplaySession);
        assert_eq!(playing.clock, ClockCapability::REPLAY_PLAYBACK);
    }

    #[test]
    fn session_policy_classifies_replay_branch_staging_and_live() {
        let staging = SessionPolicy::new(SessionMode::ReplayBranch, SessionPhase::BranchStaging);
        assert_eq!(staging.state_source, StateSource::ReplayBranchSeed);
        assert_eq!(staging.join, JoinPolicy::BranchStaging);
        assert_eq!(staging.clock, ClockCapability::BRANCH_STAGING);
        assert_eq!(staging.authority, AuthorityPolicy::BranchStagingHost);
        assert_eq!(staging.visibility, VisibilityPolicy::BranchStagingState);
        assert_eq!(staging.mutation, MutationPolicy::ReplayBranchStagingClaims);
        assert_eq!(staging.persistence, PersistencePolicy::Suppressed);
        assert_eq!(staging.start_payload, StartPayloadPolicy::None);
        assert!(staging.countdown_eligible);
        assert!(!staging.allows_match_history());

        let live = SessionPolicy::new(SessionMode::ReplayBranch, SessionPhase::LiveMatch);
        assert_eq!(live.state_source, StateSource::BranchLiveGame);
        assert_eq!(live.join, JoinPolicy::BranchLiveAttach);
        assert_eq!(live.clock, ClockCapability::LIVE_MATCH);
        assert_eq!(live.authority, AuthorityPolicy::BranchLiveSeatAliases);
        assert_eq!(live.visibility, VisibilityPolicy::LiveFog);
        assert_eq!(live.mutation, MutationPolicy::BranchLiveSeatAliasGameplay);
        assert_eq!(live.persistence, PersistencePolicy::Suppressed);
        assert_eq!(live.start_payload, StartPayloadPolicy::ReplayBranchLive);
        assert!(!live.countdown_eligible);
    }

    #[test]
    fn session_policy_classifies_dev_scenario_as_dev_watch() {
        let dev = SessionPolicy::new(SessionMode::DevScenario, SessionPhase::LiveMatch);
        assert_eq!(dev.state_source, StateSource::DevScenario);
        assert_eq!(dev.join, JoinPolicy::DevWatch);
        assert_eq!(dev.clock, ClockCapability::DEV_SCENARIO);
        assert_eq!(dev.authority, AuthorityPolicy::DevWatchControls);
        assert_eq!(dev.visibility, VisibilityPolicy::DevFullWorld);
        assert_eq!(dev.mutation, MutationPolicy::DevScenarioDriver);
        assert_eq!(dev.persistence, PersistencePolicy::Suppressed);
        assert_eq!(dev.start_payload, StartPayloadPolicy::DevWatch);
        assert!(!dev.countdown_eligible);
        assert!(dev.is_dev_watch());
        assert!(!dev.allows_match_history());
    }

    #[test]
    fn session_policy_classifies_lab_as_full_world_room_mode() {
        let lab_lobby = SessionPolicy::new(SessionMode::Lab, SessionPhase::Lobby);
        assert_eq!(lab_lobby.state_source, StateSource::LabGame);
        assert_eq!(lab_lobby.join, JoinPolicy::LabRoom);
        assert_eq!(lab_lobby.clock, ClockCapability::LIVE_MATCH);
        assert_eq!(lab_lobby.authority, AuthorityPolicy::LabOperator);
        assert_eq!(lab_lobby.visibility, VisibilityPolicy::LabFullWorld);
        assert_eq!(lab_lobby.mutation, MutationPolicy::LabPrivilegedOps);
        assert_eq!(lab_lobby.persistence, PersistencePolicy::Suppressed);
        assert_eq!(lab_lobby.start_payload, StartPayloadPolicy::Lab);
        assert!(!lab_lobby.countdown_eligible);
        assert!(lab_lobby.uses_lab_room_join());
        assert!(!lab_lobby.allows_match_history());

        let lab_live = SessionPolicy::new(SessionMode::Lab, SessionPhase::LiveMatch);
        assert_eq!(lab_live.state_source, StateSource::LabGame);
        assert_eq!(lab_live.join, JoinPolicy::LabRoom);
        assert_eq!(lab_live.visibility, VisibilityPolicy::LabFullWorld);
        assert_eq!(lab_live.start_payload, StartPayloadPolicy::Lab);
    }
}
