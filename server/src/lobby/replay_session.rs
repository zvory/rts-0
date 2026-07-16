use super::replay_validation;
use super::{normalize_start_team_id, ReplayBranchSeed, MAX_PLAYERS};
use crate::protocol::{
    Event, ReplayBranchSeat, ReplayStartMetadata, RoomTimeState, VisionSelectionRequest,
};
use rts_sim::game::command::SimCommand;
use rts_sim::game::map::Map;
use rts_sim::game::replay::{ReplayArtifactV1, ReplayValidationError};
use rts_sim::game::Game;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant as StdInstant};

#[derive(Clone, Debug, PartialEq, Eq)]
enum VisionSelection {
    All,
    Players(Vec<u32>),
}

impl VisionSelection {
    fn from_request(request: VisionSelectionRequest) -> Self {
        match request {
            VisionSelectionRequest::All => VisionSelection::All,
            VisionSelectionRequest::Player { player_id } => {
                VisionSelection::Players(vec![player_id])
            }
            VisionSelectionRequest::Players { player_ids } => VisionSelection::Players(player_ids),
        }
    }

    fn player_ids(&self, all_players: &[u32]) -> Vec<u32> {
        match self {
            VisionSelection::All => all_players.to_vec(),
            VisionSelection::Players(ids) => ids.clone(),
        }
    }
}

pub(super) fn validate_vision_selection_request(
    vision: &VisionSelectionRequest,
    valid_player_ids: &[u32],
) -> Result<(), &'static str> {
    let valid: HashSet<u32> = valid_player_ids.iter().copied().collect();
    match vision {
        VisionSelectionRequest::All => {
            if valid.is_empty() {
                Err("no replay players")
            } else {
                Ok(())
            }
        }
        VisionSelectionRequest::Player { player_id } => {
            if valid.contains(player_id) {
                Ok(())
            } else {
                Err("unknown replay player")
            }
        }
        VisionSelectionRequest::Players { player_ids } => {
            if player_ids.is_empty() {
                return Err("empty replay player subset");
            }
            let mut seen = HashSet::new();
            for player_id in player_ids {
                if !valid.contains(player_id) {
                    return Err("unknown replay player");
                }
                if !seen.insert(*player_id) {
                    return Err("duplicate replay player");
                }
            }
            Ok(())
        }
    }
}

/// Reusable server-side replay runtime. It owns the artifact and a rebuilt simulation, and the
/// room task drives it exactly like a live game.
pub(super) struct ReplaySession {
    pub(super) artifact: ReplayArtifactV1,
    pub(super) game: Box<Game>,
    pub(super) next_command: usize,
    pub(super) keyframes: Vec<ReplayKeyframe>,
    start_tick: u32,
    pub(super) duration_ticks: u32,
    speed: f32,
    viewer_selection: HashMap<u32, VisionSelection>,
    last_controller_id: Option<u32>,
    last_seek_at: Option<StdInstant>,
}

pub(super) struct ReplayKeyframe {
    pub(super) tick: u32,
    pub(super) game: Box<Game>,
    pub(super) next_command: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReplaySeekPlan {
    pub(super) from_tick: u32,
    pub(super) target_tick: u32,
}

impl ReplaySession {
    #[allow(dead_code)]
    pub(super) const DEFAULT_SPEED: f32 = 2.0;
    pub(super) const PAUSED_SPEED: f32 = 0.0;
    const MIN_SPEED: f32 = 0.125;
    pub(super) const MAX_SPEED: f32 = 8.0;
    pub(super) const MAX_DURATION_TICKS: u32 = 30 * 60 * 60;
    const MAX_COMMAND_LOG_ENTRIES: usize = 200_000;
    const SEEK_COOLDOWN: Duration = Duration::from_millis(500);
    const KEYFRAME_INTERVAL_TICKS: u32 = 2_000;

    #[allow(dead_code)]
    pub(super) fn new(artifact: ReplayArtifactV1) -> Result<Self, String> {
        Self::validate_artifact_static_limits(&artifact)?;
        let duration_ticks = artifact.duration_ticks;
        let build_start = StdInstant::now();
        let game = Box::new(Self::build_game_for_build(
            &artifact,
            crate::build_info::build_id(),
            true,
        )?);
        let start_tick = game.tick_count();
        Self::validate_artifact_timeline(&artifact, start_tick)?;
        let keyframes = vec![ReplayKeyframe {
            tick: start_tick,
            game: Box::new(game.clone_for_replay_keyframe()),
            next_command: 0,
        }];
        crate::log_info!(
            map = %artifact.map_name,
            duration_ticks,
            command_count = artifact.command_log.len(),
            player_count = artifact.players.len(),
            build_ms = build_start.elapsed().as_millis(),
            "replay session built"
        );
        Ok(ReplaySession {
            artifact,
            game,
            next_command: 0,
            keyframes,
            start_tick,
            duration_ticks,
            speed: Self::DEFAULT_SPEED,
            viewer_selection: HashMap::new(),
            last_controller_id: None,
            last_seek_at: None,
        })
    }

    pub(super) fn validate_artifact_for_launch(
        artifact: &ReplayArtifactV1,
        expected_build_sha: &str,
    ) -> Result<(), String> {
        Self::validate_artifact_static_limits(artifact)?;
        let game = Self::build_game_for_build(artifact, expected_build_sha, false)?;
        Self::validate_artifact_timeline(artifact, game.tick_count())
    }

    fn validate_artifact_static_limits(artifact: &ReplayArtifactV1) -> Result<(), String> {
        if artifact.players.is_empty() {
            return Err("replay artifact has no players".to_string());
        }
        if artifact.players.len() > MAX_PLAYERS {
            return Err(format!(
                "replay artifact has {} players; maximum is {MAX_PLAYERS}",
                artifact.players.len()
            ));
        }
        replay_validation::validate_faction_loadouts(artifact)?;
        if artifact.duration_ticks > Self::MAX_DURATION_TICKS {
            return Err(format!(
                "replay duration {} exceeds maximum {}",
                artifact.duration_ticks,
                Self::MAX_DURATION_TICKS
            ));
        }
        if artifact.command_log.len() > Self::MAX_COMMAND_LOG_ENTRIES {
            return Err(format!(
                "replay command log has {} entries; maximum is {}",
                artifact.command_log.len(),
                Self::MAX_COMMAND_LOG_ENTRIES
            ));
        }
        Ok(())
    }

    fn validate_artifact_timeline(
        artifact: &ReplayArtifactV1,
        start_tick: u32,
    ) -> Result<(), String> {
        if artifact.duration_ticks < start_tick {
            return Err(format!(
                "replay duration {} is before start tick {}",
                artifact.duration_ticks, start_tick
            ));
        }
        let seen_players: HashSet<u32> = artifact.players.iter().map(|player| player.id).collect();
        let mut previous_tick = 0;
        for (index, entry) in artifact.command_log.iter().enumerate() {
            if !seen_players.contains(&entry.player_id) {
                return Err(format!(
                    "replay command {index} references unknown player {}",
                    entry.player_id
                ));
            }
            if entry.tick <= start_tick {
                return Err(format!(
                    "replay command {index} tick {} is not after start tick {}",
                    entry.tick, start_tick
                ));
            }
            if entry.tick > artifact.duration_ticks {
                return Err(format!(
                    "replay command {index} tick {} exceeds duration {}",
                    entry.tick, artifact.duration_ticks
                ));
            }
            if entry.tick < previous_tick {
                return Err(format!(
                    "replay command {index} is out of order: tick {} before {}",
                    entry.tick, previous_tick
                ));
            }
            previous_tick = entry.tick;
        }
        Ok(())
    }

    fn build_game_for_build(
        artifact: &ReplayArtifactV1,
        expected_build_sha: &str,
        log_build_mismatch: bool,
    ) -> Result<Game, String> {
        let metadata = Map::metadata_for_name(&artifact.map_name)
            .map_err(|err| format!("cannot load replay map metadata: {err}"))?;
        artifact
            .validate_against(expected_build_sha, &metadata)
            .or_else(|err| match err {
                ReplayValidationError::BuildShaMismatch { artifact, running } => {
                    if log_build_mismatch {
                        crate::log_warn!(
                            replay_build_sha = %artifact,
                            server_build_sha = %running,
                            "replay build differs from current server; attempting playback"
                        );
                    }
                    Ok(())
                }
                err => Err(err),
            })
            .map_err(|err| err.to_string())?;
        let replay_start_players: Vec<_> = artifact
            .players
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map = Map::load_for_players(&artifact.map_name, &replay_start_players, artifact.seed)
            .map_err(|err| format!("cannot load replay map: {err}"))?;
        artifact
            .restore_start_game(map, metadata)
            .map_err(|err| err.to_string())
    }

    pub(super) fn active_player_ids(&self) -> Vec<u32> {
        self.artifact.players.iter().map(|p| p.id).collect()
    }

    pub(super) fn start_metadata(&self) -> ReplayStartMetadata {
        self.artifact.start_metadata()
    }

    pub(super) fn state(&self) -> RoomTimeState {
        RoomTimeState {
            current_tick: self.current_tick(),
            duration_ticks: self.duration_ticks,
            keyframe_ticks: self
                .keyframes
                .iter()
                .map(|keyframe| keyframe.tick)
                .collect(),
            speed: self.speed,
            paused: self.speed == Self::PAUSED_SPEED,
            ended: self.current_tick() >= self.duration_ticks,
            controller_id: self.last_controller_id,
        }
    }

    pub(super) fn speed(&self) -> f32 {
        self.speed
    }

    pub(super) fn is_paused(&self) -> bool {
        self.speed == Self::PAUSED_SPEED
    }

    pub(super) fn has_remaining_ticks(&self) -> bool {
        self.current_tick() < self.duration_ticks
    }

    pub(super) fn current_tick(&self) -> u32 {
        self.game.tick_count()
    }

    pub(super) fn game(&self) -> &Game {
        &self.game
    }

    pub(super) fn remove_viewer(&mut self, viewer_id: u32) {
        self.viewer_selection.remove(&viewer_id);
    }

    pub(super) fn can_create_replay_branch(&self) -> bool {
        !self.artifact.players.iter().any(|player| player.is_ai)
    }

    pub(super) fn branch_seed(&self) -> Result<ReplayBranchSeed, String> {
        if !self.can_create_replay_branch() {
            return Err("Replay branching does not support replays with AI seats yet.".to_string());
        }
        let source_tick = self.current_tick();
        let seats = self
            .artifact
            .players
            .iter()
            .map(|player| ReplayBranchSeat {
                player_id: player.id,
                team_id: normalize_start_team_id(player.id, player.team_id),
                faction_id: player.faction_id.clone(),
                name: player.name.clone(),
                color: player.color.clone(),
                claimable: true,
            })
            .collect();
        Ok(ReplayBranchSeed {
            source_replay: self.artifact.start_metadata(),
            source_tick,
            game: Box::new(self.game.clone_for_replay_keyframe()),
            seats,
        })
    }

    pub(super) fn set_speed(&mut self, controller_id: u32, speed: f32) {
        self.speed = if speed == Self::PAUSED_SPEED {
            Self::PAUSED_SPEED
        } else {
            speed.clamp(Self::MIN_SPEED, Self::MAX_SPEED)
        };
        self.last_controller_id = Some(controller_id);
    }

    pub(super) fn set_vision(&mut self, viewer_id: u32, vision: VisionSelectionRequest) {
        self.viewer_selection
            .insert(viewer_id, VisionSelection::from_request(vision));
    }

    pub(super) fn vision_player_ids_for(&self, viewer_id: u32) -> Vec<u32> {
        let all_players = self.active_player_ids();
        self.viewer_selection
            .get(&viewer_id)
            .unwrap_or(&VisionSelection::All)
            .player_ids(&all_players)
    }

    pub(super) fn enqueue_for_current_tick(&mut self) -> Result<(), String> {
        let tick = self.current_tick().saturating_add(1);
        while let Some(entry) = self.artifact.command_log.get(self.next_command) {
            if entry.tick < tick {
                return Err(format!(
                    "replay command {} is out of order: tick {} before {}",
                    self.next_command, entry.tick, tick
                ));
            }
            if entry.tick != tick {
                break;
            }
            self.game.enqueue(
                entry.player_id,
                SimCommand::from_protocol(entry.command.clone()),
            );
            self.next_command += 1;
        }
        Ok(())
    }

    pub(super) fn tick(
        &mut self,
        perf: Option<&mut rts_sim::perf::TickPerf>,
    ) -> HashMap<u32, Vec<Event>> {
        self.game.tick_with_perf(perf).into_iter().collect()
    }

    pub(super) fn record_keyframe_if_due(&mut self) {
        let tick = self.current_tick();
        if tick == 0 || !tick.is_multiple_of(Self::KEYFRAME_INTERVAL_TICKS) {
            return;
        }
        match self
            .keyframes
            .binary_search_by_key(&tick, |keyframe| keyframe.tick)
        {
            Ok(_) => (),
            Err(index) => self.keyframes.insert(
                index,
                ReplayKeyframe {
                    tick,
                    game: Box::new(self.game.clone_for_replay_keyframe()),
                    next_command: self.next_command,
                },
            ),
        }
    }

    #[cfg(test)]
    pub(super) fn seek_back(
        &mut self,
        room: &str,
        viewer_count: usize,
        controller_id: u32,
        ticks_back: u32,
    ) -> Result<u32, String> {
        let plan = self.plan_seek_back(ticks_back)?;
        self.apply_seek(room, viewer_count, controller_id, plan)
    }

    #[cfg(test)]
    pub(super) fn seek_to(
        &mut self,
        room: &str,
        viewer_count: usize,
        controller_id: u32,
        target_tick: u32,
    ) -> Result<u32, String> {
        let plan = self.plan_seek_to(target_tick)?;
        self.apply_seek(room, viewer_count, controller_id, plan)
    }

    pub(super) fn plan_seek_back(&self, ticks_back: u32) -> Result<ReplaySeekPlan, String> {
        self.plan_seek_to(self.current_tick().saturating_sub(ticks_back))
    }

    pub(super) fn plan_seek_to(&self, target_tick: u32) -> Result<ReplaySeekPlan, String> {
        if self
            .last_seek_at
            .is_some_and(|last_seek| last_seek.elapsed() < Self::SEEK_COOLDOWN)
        {
            return Err("Replay seek ignored; wait before seeking again.".to_string());
        }
        Ok(ReplaySeekPlan {
            from_tick: self.current_tick(),
            target_tick: target_tick.clamp(self.start_tick, self.duration_ticks),
        })
    }

    pub(super) fn apply_seek(
        &mut self,
        room: &str,
        viewer_count: usize,
        controller_id: u32,
        plan: ReplaySeekPlan,
    ) -> Result<u32, String> {
        // Replay reconstruction is synchronous CPU work. In the production multi-thread runtime,
        // mark this section as blocking so Tokio can hand this worker's async tasks (especially
        // connection writers carrying RoomTimeSeekStarted) to a replacement worker. The fallback
        // keeps ordinary unit tests and any current-thread runtime usable.
        if tokio::runtime::Handle::try_current().is_ok_and(|handle| {
            handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
        }) {
            return tokio::task::block_in_place(|| {
                self.apply_seek_blocking(room, viewer_count, controller_id, plan)
            });
        }
        self.apply_seek_blocking(room, viewer_count, controller_id, plan)
    }

    fn apply_seek_blocking(
        &mut self,
        room: &str,
        viewer_count: usize,
        controller_id: u32,
        plan: ReplaySeekPlan,
    ) -> Result<u32, String> {
        debug_assert_eq!(self.current_tick(), plan.from_tick);
        let seek_start = StdInstant::now();
        let keyframe_tick = self.rebuild_to(plan.target_tick)?;
        self.last_seek_at = Some(StdInstant::now());
        self.last_controller_id = Some(controller_id);
        crate::log_info!(
            room = %room,
            controller_id,
            viewer_count,
            from_tick = plan.from_tick,
            to_tick = plan.target_tick,
            keyframe_tick,
            duration_ticks = self.duration_ticks,
            command_count = self.artifact.command_log.len(),
            keyframe_count = self.keyframes.len(),
            rebuild_ms = seek_start.elapsed().as_millis(),
            "replay seek rebuilt"
        );
        Ok(plan.target_tick)
    }

    pub(super) fn rebuild_to(&mut self, target_tick: u32) -> Result<u32, String> {
        let (keyframe_tick, keyframe_game, keyframe_next_command) = self
            .keyframes
            .iter()
            .rev()
            .find(|keyframe| keyframe.tick <= target_tick)
            .map(|keyframe| {
                (
                    keyframe.tick,
                    keyframe.game.clone_for_replay_keyframe(),
                    keyframe.next_command,
                )
            })
            .ok_or_else(|| "replay has no valid keyframe".to_string())?;
        *self.game = keyframe_game;
        self.next_command = keyframe_next_command;
        while self.current_tick() < target_tick {
            self.enqueue_for_current_tick()?;
            self.game.tick();
            self.record_keyframe_if_due();
        }
        Ok(keyframe_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Command;
    use rts_rules::faction::EMPTY_FIXTURE_FACTION_ID;
    use rts_sim::game::PlayerInit;

    fn replay_test_players(count: usize) -> Vec<PlayerInit> {
        (1..=count as u32)
            .map(|id| PlayerInit {
                id,
                team_id: id,
                faction_id: "kriegsia".to_string(),
                name: format!("Player {id}"),
                color: crate::lobby::PLAYER_PALETTE
                    [(id as usize - 1) % crate::lobby::PLAYER_PALETTE.len()]
                .to_string(),
                is_ai: false,
            })
            .collect()
    }

    fn replay_test_game(players: &[PlayerInit], seed: u32) -> Game {
        let metadata = Map::metadata_for_name("Chokes").unwrap();
        let start_players: Vec<_> = players
            .iter()
            .map(|player| {
                (
                    player.id,
                    normalize_start_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map = Map::load_for_players("Chokes", &start_players, seed).unwrap();
        Game::new_with_random_ai_profiles_and_map_metadata(players, seed, map, metadata)
    }

    fn replay_test_artifact(players: &[PlayerInit], ticks: u32) -> (Game, ReplayArtifactV1) {
        let seed = 0x5150_2202;
        let mut game = replay_test_game(players, seed);
        let replay_start = rts_sim::game::replay::ReplayStartComposition::capture(
            &game,
            crate::build_info::build_id(),
        )
        .unwrap();
        for _ in 0..ticks {
            game.tick();
        }
        let artifact = replay_start.finalize(&game, None, game.scores());
        (game, artifact)
    }

    #[test]
    fn vision_selection_validation_rejects_unknown_and_empty_subsets() {
        let valid = [1, 2, 3];

        assert!(validate_vision_selection_request(&VisionSelectionRequest::All, &valid).is_ok());
        assert!(validate_vision_selection_request(
            &VisionSelectionRequest::Player { player_id: 2 },
            &valid,
        )
        .is_ok());
        assert!(validate_vision_selection_request(
            &VisionSelectionRequest::Players {
                player_ids: vec![1, 3],
            },
            &valid,
        )
        .is_ok());

        assert!(validate_vision_selection_request(
            &VisionSelectionRequest::Player { player_id: 99 },
            &valid,
        )
        .is_err());
        assert!(validate_vision_selection_request(
            &VisionSelectionRequest::Players { player_ids: vec![] },
            &valid,
        )
        .is_err());
        assert!(validate_vision_selection_request(
            &VisionSelectionRequest::Players {
                player_ids: vec![1, 99],
            },
            &valid,
        )
        .is_err());
        assert!(validate_vision_selection_request(
            &VisionSelectionRequest::Players {
                player_ids: vec![1, 1],
            },
            &valid,
        )
        .is_err());
    }

    #[test]
    fn replay_session_reaches_live_final_snapshots() {
        let players = replay_test_players(2);
        let (live, artifact) = replay_test_artifact(&players, 5);
        let mut replay = ReplaySession::new(artifact).unwrap();

        while replay.current_tick() < replay.duration_ticks {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }

        for player in &players {
            assert_eq!(
                replay.game.snapshot_for(player.id),
                live.snapshot_for(player.id)
            );
        }
    }

    #[test]
    fn checkpoint_backed_replay_can_start_from_nonzero_tick() {
        let players = replay_test_players(2);
        let seed = 0x5150_3301;
        let mut live = replay_test_game(&players, seed);
        live.enqueue(1, SimCommand::Stop { units: vec![1] });
        live.tick();
        let branch_start = rts_sim::game::replay::ReplayStartComposition::capture(
            &live,
            crate::build_info::build_id(),
        )
        .unwrap();
        let start_tick = live.tick_count();

        live.enqueue(1, SimCommand::Stop { units: vec![2] });
        live.tick();
        let artifact = branch_start.finalize(&live, None, live.scores());

        assert_eq!(start_tick, 1);
        assert_eq!(
            artifact
                .command_log
                .iter()
                .map(|entry| entry.tick)
                .collect::<Vec<_>>(),
            vec![2],
            "checkpoint-backed artifacts should store only commands after the start checkpoint"
        );

        let mut replay = ReplaySession::new(artifact).unwrap();
        assert_eq!(replay.current_tick(), start_tick);
        let clamped = replay.seek_to("test", 1, 42, 0).unwrap();
        assert_eq!(clamped, start_tick);
        assert_eq!(replay.current_tick(), start_tick);
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);

        assert_eq!(replay.current_tick(), live.tick_count());
        for player in &players {
            assert_eq!(
                replay.game.snapshot_for(player.id),
                live.snapshot_for(player.id)
            );
        }
    }

    #[test]
    fn replay_session_records_keyframes_and_restores_nearest_before_seek_target() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        artifact.duration_ticks = 2_001;
        let mut replay = ReplaySession::new(artifact).unwrap();

        while replay.current_tick() < replay.duration_ticks {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
            replay.record_keyframe_if_due();
        }

        assert_eq!(
            replay
                .keyframes
                .iter()
                .map(|keyframe| keyframe.tick)
                .collect::<Vec<_>>(),
            vec![0, 2_000]
        );

        let mut expected = replay
            .keyframes
            .iter()
            .find(|keyframe| keyframe.tick == 2_000)
            .expect("replay should record the first interval keyframe")
            .game
            .clone_for_replay_keyframe();
        expected.tick();

        let restored_from = replay.rebuild_to(2_001).unwrap();

        assert_eq!(restored_from, 2_000);
        assert_eq!(replay.current_tick(), 2_001);

        for player in &players {
            assert_eq!(
                replay.game.snapshot_for(player.id),
                expected.snapshot_for(player.id)
            );
        }
    }

    #[test]
    fn replay_viewer_snapshot_hides_resource_outside_union_fog() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let replay = ReplaySession::new(artifact).unwrap();

        let full = replay.game.snapshot_full_for(players[0].id);
        let union = replay
            .game
            .snapshot_for_spectator(&replay.active_player_ids());

        assert!(
            full.resource_deltas.len() > union.resource_deltas.len(),
            "default replay spectator fog should not expose every resource node"
        );
    }

    #[test]
    fn single_player_replay_fog_matches_player_visibility() {
        let players = replay_test_players(1);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let replay = ReplaySession::new(artifact).unwrap();

        let player = replay.game.snapshot_for(players[0].id);
        let replay_view = replay.game.snapshot_for_spectator(&[players[0].id]);

        assert_eq!(replay_view.visible_tiles, player.visible_tiles);
        assert_eq!(
            replay_view
                .entities
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>(),
            player
                .entities
                .iter()
                .map(|entity| entity.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn vision_selection_is_per_viewer() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 0);
        let mut replay = ReplaySession::new(artifact).unwrap();

        replay.set_vision(
            100,
            VisionSelectionRequest::Player {
                player_id: players[0].id,
            },
        );
        replay.set_vision(
            101,
            VisionSelectionRequest::Player {
                player_id: players[1].id,
            },
        );

        assert_eq!(replay.vision_player_ids_for(100), vec![players[0].id]);
        assert_eq!(replay.vision_player_ids_for(101), vec![players[1].id]);
        assert_eq!(
            replay.vision_player_ids_for(102),
            replay.active_player_ids()
        );
    }

    #[test]
    fn replay_speed_and_seek_are_clamped_in_state() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 4);
        let mut replay = ReplaySession::new(artifact).unwrap();
        for _ in 0..3 {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }

        replay.set_speed(42, 99.0);
        assert_eq!(replay.state().speed, ReplaySession::MAX_SPEED);
        assert_eq!(replay.state().controller_id, Some(42));
        assert_eq!(replay.state().keyframe_ticks, vec![0]);

        replay.set_speed(42, 0.0);
        assert_eq!(replay.state().speed, ReplaySession::PAUSED_SPEED);
        assert!(replay.state().paused);

        let target = replay.seek_back("test", 1, 42, u32::MAX).unwrap();
        assert_eq!(target, 0);
        assert_eq!(replay.state().current_tick, 0);
    }

    #[test]
    fn observer_analysis_restores_from_keyframe_without_accumulating_extra_losses() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 1);
        let mut replay = ReplaySession::new(artifact).unwrap();

        replay.game.eliminate(players[1].id);
        let expected = replay.game.observer_analysis();
        replay.keyframes[0] = ReplayKeyframe {
            tick: replay.current_tick(),
            game: Box::new(replay.game.clone_for_replay_keyframe()),
            next_command: replay.next_command,
        };

        replay.game.eliminate(players[0].id);
        replay.rebuild_to(0).unwrap();

        assert_eq!(replay.game.observer_analysis(), expected);
    }

    #[test]
    fn replay_seek_frequency_is_bounded() {
        let players = replay_test_players(2);
        let (_live, artifact) = replay_test_artifact(&players, 4);
        let mut replay = ReplaySession::new(artifact).unwrap();
        for _ in 0..3 {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }

        assert!(replay.seek_back("test", 1, 42, 1).is_ok());
        let err = replay.seek_back("test", 1, 42, 1).unwrap_err();
        assert!(
            err.contains("wait before seeking again"),
            "unexpected seek reject: {err}"
        );
    }

    #[test]
    fn replay_session_allows_build_sha_mismatch() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 1);
        artifact.server_build_sha = "older-build".to_string();

        let replay = ReplaySession::new(artifact).unwrap();

        assert_eq!(replay.current_tick(), 0);
    }

    #[test]
    fn replay_artifact_limits_reject_malformed_command_logs() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 2);
        artifact
            .command_log
            .push(rts_sim::game::replay::CommandLogEntry {
                tick: artifact.duration_ticks + 1,
                player_id: players[0].id,
                command: Command::Stop { units: vec![1] },
            });

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("malformed replay artifact should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("exceeds duration"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_reject_duplicate_players() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        artifact.players.push(artifact.players[0].clone());

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("duplicate-player replay artifact should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("duplicate player id"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_require_matching_player_loadouts() {
        let players = replay_test_players(2);
        let (_live, mut missing_artifact) = replay_test_artifact(&players, 0);
        missing_artifact
            .player_loadouts
            .retain(|loadout| loadout.player_id != players[0].id);

        let err = match ReplaySession::new(missing_artifact) {
            Ok(_) => panic!("missing replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("missing a loadout"),
            "unexpected artifact reject: {err}"
        );

        let (_live, mut mismatched_artifact) = replay_test_artifact(&players, 0);
        mismatched_artifact.player_loadouts[0].faction_id = "ekat".to_string();

        let err = match ReplaySession::new(mismatched_artifact) {
            Ok(_) => panic!("mismatched replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("loadout faction"),
            "unexpected artifact reject: {err}"
        );

        let (_live, mut unknown_loadout_artifact) = replay_test_artifact(&players, 0);
        unknown_loadout_artifact.player_loadouts[0].loadout_id = "kriegsia.missing".to_string();

        let err = match ReplaySession::new(unknown_loadout_artifact) {
            Ok(_) => panic!("unknown replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("unknown loadout"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_reject_unknown_player_loadout() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        let mut extra_loadout = artifact.player_loadouts[0].clone();
        extra_loadout.player_id = 999;
        artifact.player_loadouts.push(extra_loadout);

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("unknown-player replay loadout should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("unknown player 999"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_artifact_limits_reject_oversized_duration() {
        let players = replay_test_players(2);
        let (_live, mut artifact) = replay_test_artifact(&players, 0);
        artifact.duration_ticks = ReplaySession::MAX_DURATION_TICKS + 1;

        let err = match ReplaySession::new(artifact) {
            Ok(_) => panic!("oversized replay artifact should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("exceeds maximum"),
            "unexpected artifact reject: {err}"
        );
    }

    #[test]
    fn replay_session_rejects_unknown_or_fixture_faction_ids() {
        let players = replay_test_players(2);
        let (_live, mut unknown_artifact) = replay_test_artifact(&players, 0);
        unknown_artifact.players[0].faction_id = "unknown-faction".to_string();

        let err = match ReplaySession::new(unknown_artifact) {
            Ok(_) => panic!("unsupported replay faction should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("unknown faction"),
            "unexpected artifact reject: {err}"
        );

        let (_live, mut fixture_artifact) = replay_test_artifact(&players, 0);
        fixture_artifact.players[0].faction_id = EMPTY_FIXTURE_FACTION_ID.to_string();

        let err = match ReplaySession::new(fixture_artifact) {
            Ok(_) => panic!("fixture replay faction should be rejected"),
            Err(err) => err,
        };
        assert!(
            err.contains("fixture-only"),
            "unexpected artifact reject: {err}"
        );
    }
}
