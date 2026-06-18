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
    DevWatch,
    LabRoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClockPolicy {
    RoomTicker,
    LiveMatch,
    ReplayPlayback,
    BranchStaging,
    DevWatch,
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
pub(super) enum VisionPolicy {
    LobbyState,
    LiveFog,
    ReplayVision,
    BranchStagingState,
    DevFullWorld,
    LabFullWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MutationPolicy {
    LobbyState,
    LiveGame,
    ReplayPlayback,
    BranchStagingClaims,
    BranchLiveGame,
    DevScenarioGame,
    LabReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PersistencePolicy {
    MatchHistoryEligible,
    Suppressed,
    None,
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
pub(super) struct SessionPolicy {
    pub(super) mode: SessionMode,
    pub(super) phase: SessionPhase,
    pub(super) state_source: StateSource,
    pub(super) join: JoinPolicy,
    pub(super) clock: ClockPolicy,
    pub(super) authority: AuthorityPolicy,
    pub(super) vision: VisionPolicy,
    pub(super) mutation: MutationPolicy,
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
                clock: ClockPolicy::RoomTicker,
                authority: AuthorityPolicy::LobbyHost,
                vision: VisionPolicy::LobbyState,
                mutation: MutationPolicy::LobbyState,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::None,
                countdown_eligible: true,
            },
            SessionPhase::LiveMatch => Self {
                mode,
                phase,
                state_source: StateSource::LiveGame,
                join: JoinPolicy::RejectMidMatch,
                clock: ClockPolicy::LiveMatch,
                authority: AuthorityPolicy::LivePlayers,
                vision: VisionPolicy::LiveFog,
                mutation: MutationPolicy::LiveGame,
                persistence: PersistencePolicy::MatchHistoryEligible,
                start_payload: StartPayloadPolicy::LiveMatch,
                countdown_eligible: false,
            },
            SessionPhase::ReplayViewer => Self {
                mode,
                phase,
                state_source: StateSource::PostMatchReplaySession,
                join: JoinPolicy::ReplayPromptOrAttach,
                clock: ClockPolicy::ReplayPlayback,
                authority: AuthorityPolicy::ReplayViewers,
                vision: VisionPolicy::ReplayVision,
                mutation: MutationPolicy::ReplayPlayback,
                persistence: PersistencePolicy::None,
                start_payload: StartPayloadPolicy::ReplayViewer,
                countdown_eligible: false,
            },
            SessionPhase::BranchStaging => Self {
                mode,
                phase,
                state_source: StateSource::ReplayBranchSeed,
                join: JoinPolicy::BranchStaging,
                clock: ClockPolicy::BranchStaging,
                authority: AuthorityPolicy::BranchStagingHost,
                vision: VisionPolicy::BranchStagingState,
                mutation: MutationPolicy::BranchStagingClaims,
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
                policy.clock = ClockPolicy::DevWatch;
                policy.authority = AuthorityPolicy::DevWatchControls;
                policy.vision = VisionPolicy::DevFullWorld;
                policy.mutation = MutationPolicy::DevScenarioGame;
                policy.persistence = PersistencePolicy::Suppressed;
                policy.start_payload = StartPayloadPolicy::DevWatch;
                policy.countdown_eligible = false;
            }
            SessionMode::Replay => {
                if phase == SessionPhase::Lobby {
                    policy.state_source = StateSource::PersistedReplayArtifact;
                    policy.clock = ClockPolicy::RoomTicker;
                }
                policy.join = JoinPolicy::ReplayPromptOrAttach;
                policy.authority = AuthorityPolicy::ReplayViewers;
                policy.vision = VisionPolicy::ReplayVision;
                policy.mutation = MutationPolicy::ReplayPlayback;
                policy.persistence = PersistencePolicy::None;
                policy.start_payload = StartPayloadPolicy::ReplayViewer;
                policy.countdown_eligible = false;
            }
            SessionMode::ReplayArtifact => {
                if phase == SessionPhase::Lobby {
                    policy.state_source = StateSource::SavedReplayArtifact;
                    policy.clock = ClockPolicy::RoomTicker;
                }
                policy.join = JoinPolicy::ReplayPromptOrAttach;
                policy.authority = AuthorityPolicy::ReplayViewers;
                policy.vision = VisionPolicy::ReplayVision;
                policy.mutation = MutationPolicy::ReplayPlayback;
                policy.persistence = PersistencePolicy::None;
                policy.start_payload = StartPayloadPolicy::ReplayViewer;
                policy.countdown_eligible = false;
            }
            SessionMode::ReplayBranch => {
                policy.state_source = match phase {
                    SessionPhase::LiveMatch => StateSource::BranchLiveGame,
                    _ => StateSource::ReplayBranchSeed,
                };
                policy.join = JoinPolicy::BranchStaging;
                policy.clock = match phase {
                    SessionPhase::LiveMatch => ClockPolicy::LiveMatch,
                    _ => ClockPolicy::BranchStaging,
                };
                policy.authority = match phase {
                    SessionPhase::LiveMatch => AuthorityPolicy::BranchLiveSeatAliases,
                    _ => AuthorityPolicy::BranchStagingHost,
                };
                policy.vision = match phase {
                    SessionPhase::LiveMatch => VisionPolicy::LiveFog,
                    _ => VisionPolicy::BranchStagingState,
                };
                policy.mutation = match phase {
                    SessionPhase::LiveMatch => MutationPolicy::BranchLiveGame,
                    _ => MutationPolicy::BranchStagingClaims,
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
                policy.clock = ClockPolicy::LiveMatch;
                policy.authority = AuthorityPolicy::LabOperator;
                policy.vision = VisionPolicy::LabFullWorld;
                policy.mutation = MutationPolicy::LabReadOnly;
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

    pub(super) fn uses_branch_staging_join(self) -> bool {
        self.join == JoinPolicy::BranchStaging
    }

    pub(super) fn uses_lab_room_join(self) -> bool {
        self.join == JoinPolicy::LabRoom
    }

    pub(super) fn allows_match_history(self) -> bool {
        self.mode == SessionMode::Normal
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

    #[test]
    fn session_policy_classifies_normal_lobby_live_and_post_match_replay() {
        let lobby = SessionPolicy::new(SessionMode::Normal, SessionPhase::Lobby);
        assert_eq!(lobby.state_source, StateSource::LobbyState);
        assert_eq!(lobby.join, JoinPolicy::NormalLobby);
        assert_eq!(lobby.clock, ClockPolicy::RoomTicker);
        assert_eq!(lobby.authority, AuthorityPolicy::LobbyHost);
        assert_eq!(lobby.vision, VisionPolicy::LobbyState);
        assert_eq!(lobby.mutation, MutationPolicy::LobbyState);
        assert_eq!(lobby.persistence, PersistencePolicy::None);
        assert_eq!(lobby.start_payload, StartPayloadPolicy::None);
        assert!(lobby.countdown_eligible);

        let live = SessionPolicy::new(SessionMode::Normal, SessionPhase::LiveMatch);
        assert_eq!(live.state_source, StateSource::LiveGame);
        assert_eq!(live.join, JoinPolicy::RejectMidMatch);
        assert_eq!(live.clock, ClockPolicy::LiveMatch);
        assert_eq!(live.authority, AuthorityPolicy::LivePlayers);
        assert_eq!(live.vision, VisionPolicy::LiveFog);
        assert_eq!(live.mutation, MutationPolicy::LiveGame);
        assert_eq!(live.persistence, PersistencePolicy::MatchHistoryEligible);
        assert_eq!(live.start_payload, StartPayloadPolicy::LiveMatch);
        assert!(!live.countdown_eligible);
        assert!(live.allows_match_history());

        let replay = SessionPolicy::new(SessionMode::Normal, SessionPhase::ReplayViewer);
        assert_eq!(replay.state_source, StateSource::PostMatchReplaySession);
        assert_eq!(replay.join, JoinPolicy::ReplayPromptOrAttach);
        assert_eq!(replay.clock, ClockPolicy::ReplayPlayback);
        assert_eq!(replay.authority, AuthorityPolicy::ReplayViewers);
        assert_eq!(replay.vision, VisionPolicy::ReplayVision);
        assert_eq!(replay.mutation, MutationPolicy::ReplayPlayback);
        assert_eq!(replay.persistence, PersistencePolicy::None);
        assert_eq!(replay.start_payload, StartPayloadPolicy::ReplayViewer);
        assert!(!replay.countdown_eligible);
        assert!(replay.allows_match_history());
    }

    #[test]
    fn session_policy_classifies_dedicated_replay_rooms() {
        let persisted = SessionPolicy::new(SessionMode::Replay, SessionPhase::Lobby);
        assert_eq!(persisted.state_source, StateSource::PersistedReplayArtifact);
        assert_eq!(persisted.join, JoinPolicy::ReplayPromptOrAttach);
        assert_eq!(persisted.clock, ClockPolicy::RoomTicker);
        assert_eq!(persisted.authority, AuthorityPolicy::ReplayViewers);
        assert_eq!(persisted.vision, VisionPolicy::ReplayVision);
        assert_eq!(persisted.mutation, MutationPolicy::ReplayPlayback);
        assert_eq!(persisted.persistence, PersistencePolicy::None);
        assert_eq!(persisted.start_payload, StartPayloadPolicy::ReplayViewer);
        assert!(!persisted.countdown_eligible);
        assert!(!persisted.allows_match_history());

        let saved = SessionPolicy::new(SessionMode::ReplayArtifact, SessionPhase::Lobby);
        assert_eq!(saved.state_source, StateSource::SavedReplayArtifact);
        assert_eq!(saved.join, JoinPolicy::ReplayPromptOrAttach);
        assert_eq!(saved.clock, ClockPolicy::RoomTicker);
        assert_eq!(saved.authority, AuthorityPolicy::ReplayViewers);
        assert_eq!(saved.vision, VisionPolicy::ReplayVision);
        assert_eq!(saved.mutation, MutationPolicy::ReplayPlayback);
        assert_eq!(saved.persistence, PersistencePolicy::None);
        assert_eq!(saved.start_payload, StartPayloadPolicy::ReplayViewer);
        assert!(!saved.countdown_eligible);

        let playing = SessionPolicy::new(SessionMode::Replay, SessionPhase::ReplayViewer);
        assert_eq!(playing.state_source, StateSource::PostMatchReplaySession);
        assert_eq!(playing.clock, ClockPolicy::ReplayPlayback);
    }

    #[test]
    fn session_policy_classifies_replay_branch_staging_and_live() {
        let staging = SessionPolicy::new(SessionMode::ReplayBranch, SessionPhase::BranchStaging);
        assert_eq!(staging.state_source, StateSource::ReplayBranchSeed);
        assert_eq!(staging.join, JoinPolicy::BranchStaging);
        assert_eq!(staging.clock, ClockPolicy::BranchStaging);
        assert_eq!(staging.authority, AuthorityPolicy::BranchStagingHost);
        assert_eq!(staging.vision, VisionPolicy::BranchStagingState);
        assert_eq!(staging.mutation, MutationPolicy::BranchStagingClaims);
        assert_eq!(staging.persistence, PersistencePolicy::Suppressed);
        assert_eq!(staging.start_payload, StartPayloadPolicy::None);
        assert!(staging.countdown_eligible);
        assert!(!staging.allows_match_history());

        let live = SessionPolicy::new(SessionMode::ReplayBranch, SessionPhase::LiveMatch);
        assert_eq!(live.state_source, StateSource::BranchLiveGame);
        assert_eq!(live.join, JoinPolicy::BranchStaging);
        assert_eq!(live.clock, ClockPolicy::LiveMatch);
        assert_eq!(live.authority, AuthorityPolicy::BranchLiveSeatAliases);
        assert_eq!(live.vision, VisionPolicy::LiveFog);
        assert_eq!(live.mutation, MutationPolicy::BranchLiveGame);
        assert_eq!(live.persistence, PersistencePolicy::Suppressed);
        assert_eq!(live.start_payload, StartPayloadPolicy::ReplayBranchLive);
        assert!(!live.countdown_eligible);
    }

    #[test]
    fn session_policy_classifies_dev_scenario_as_dev_watch() {
        let dev = SessionPolicy::new(SessionMode::DevScenario, SessionPhase::LiveMatch);
        assert_eq!(dev.state_source, StateSource::DevScenario);
        assert_eq!(dev.join, JoinPolicy::DevWatch);
        assert_eq!(dev.clock, ClockPolicy::DevWatch);
        assert_eq!(dev.authority, AuthorityPolicy::DevWatchControls);
        assert_eq!(dev.vision, VisionPolicy::DevFullWorld);
        assert_eq!(dev.mutation, MutationPolicy::DevScenarioGame);
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
        assert_eq!(lab_lobby.clock, ClockPolicy::LiveMatch);
        assert_eq!(lab_lobby.authority, AuthorityPolicy::LabOperator);
        assert_eq!(lab_lobby.vision, VisionPolicy::LabFullWorld);
        assert_eq!(lab_lobby.mutation, MutationPolicy::LabReadOnly);
        assert_eq!(lab_lobby.persistence, PersistencePolicy::Suppressed);
        assert_eq!(lab_lobby.start_payload, StartPayloadPolicy::Lab);
        assert!(!lab_lobby.countdown_eligible);
        assert!(lab_lobby.uses_lab_room_join());
        assert!(!lab_lobby.allows_match_history());

        let lab_live = SessionPolicy::new(SessionMode::Lab, SessionPhase::LiveMatch);
        assert_eq!(lab_live.state_source, StateSource::LabGame);
        assert_eq!(lab_live.join, JoinPolicy::LabRoom);
        assert_eq!(lab_live.vision, VisionPolicy::LabFullWorld);
        assert_eq!(lab_live.start_payload, StartPayloadPolicy::Lab);
    }
}
