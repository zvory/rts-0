use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LiveResumeCountdown {
    pub duration_ms: u32,
    pub remaining_ms: u32,
    pub words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LivePauseState {
    pub paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_by: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pauses_remaining: Option<u8>,
    pub pause_limit: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_pause: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_unpause: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_countdown: Option<LiveResumeCountdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MatchConclusion {
    pub defeated_player_ids: Vec<u32>,
    pub reason: MatchConclusionReason,
}

impl MatchConclusion {
    pub fn gave_up(player_id: u32) -> Self {
        Self {
            defeated_player_ids: vec![player_id],
            reason: MatchConclusionReason::GaveUp,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MatchConclusionReason {
    GaveUp,
    Eliminated,
    LostAllBuildings,
    LostPrimaryBase,
}

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
        countdown_id: u32,
        duration_ms: u32,
        words: Vec<String>,
    },
    /// Match start (flattened: carries StartPayload's fields alongside `"t":"start"`).
    Start(StartPayload),
    /// Per-player, fog-filtered world state.
    Snapshot(Snapshot),
    /// Reliable response to `requestGroundDecals`, scoped to the requester's current vision.
    GroundDecals {
        request_id: u32,
        revision: u32,
        decals: Vec<GroundDecalView>,
        tank_trails: Vec<TankTrailView>,
    },
    /// Shared room-controlled time cursor/state. Sent latest-only outside snapshot cadence.
    RoomTimeState(RoomTimeState),
    /// An accepted replay seek is about to reset and incrementally advance shared room time.
    /// Broadcast first so every viewer can present immediate progress feedback.
    RoomTimeSeekStarted {
        controller_id: u32,
        from_tick: u32,
        target_tick: u32,
    },
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
    /// One reliable, server-authored chat delivery. `tick` is present only for live/replay game
    /// chat and is the authoritative replay presentation time.
    Chat {
        scope: ChatScope,
        channel: ChatChannel,
        #[serde(skip_serializing_if = "Option::is_none")]
        tick: Option<u32>,
        sender_id: u32,
        sender_name: String,
        text: String,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        conclusion: Option<MatchConclusion>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_ground_decal_delta_response() {
        let message = ServerMessage::GroundDecals {
            request_id: 4,
            revision: 9,
            decals: vec![GroundDecalView {
                id: 3,
                decal_class: "mortarBlast".to_string(),
                source_kind: "mortarTeam".to_string(),
                x: 48.0,
                y: 64.0,
                owner: 1,
                seed: 77,
                facing: None,
                weapon_facing: None,
                radius_tiles: Some(1.5),
            }],
            tank_trails: vec![TankTrailView {
                id: 8,
                poses: vec![[128, 256, 0], [160, 256, 1024]],
            }],
        };
        let wire = serde_json::to_value(message).unwrap();
        assert_eq!(wire["t"], "groundDecals");
        assert_eq!(wire["requestId"], 4);
        assert_eq!(wire["revision"], 9);
        assert_eq!(wire["decals"][0]["decalClass"], "mortarBlast");
        assert_eq!(wire["decals"][0]["radiusTiles"], 1.5);
        assert_eq!(wire["tankTrails"][0]["poses"][1][0], 160);
    }

    #[test]
    fn serializes_match_conclusion_for_score_explanation() {
        let message = ServerMessage::GameOver {
            winner_id: Some(1),
            winner_team_id: Some(1),
            conclusion: Some(MatchConclusion {
                defeated_player_ids: vec![2],
                reason: MatchConclusionReason::GaveUp,
            }),
            you: "won".to_string(),
            scores: Vec::new(),
        };
        let wire = serde_json::to_value(message).unwrap();
        assert_eq!(wire["t"], "gameOver");
        assert_eq!(
            wire["conclusion"]["defeatedPlayerIds"],
            serde_json::json!([2])
        );
        assert_eq!(wire["conclusion"]["reason"], "gaveUp");
    }

    #[test]
    fn serializes_every_automatic_match_conclusion_reason() {
        assert_eq!(
            serde_json::to_value(MatchConclusionReason::Eliminated).unwrap(),
            "eliminated"
        );
        assert_eq!(
            serde_json::to_value(MatchConclusionReason::LostAllBuildings).unwrap(),
            "lostAllBuildings"
        );
        assert_eq!(
            serde_json::to_value(MatchConclusionReason::LostPrimaryBase).unwrap(),
            "lostPrimaryBase"
        );
    }
}
