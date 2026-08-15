use super::helpers::server_build_sha;
use super::RoomTask;
use crate::protocol::{MatchConclusion, PlayerScore};
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::Game;

impl RoomTask {
    pub(super) fn capture_replay_start_for(&mut self, game: &Game) {
        match rts_sim::game::replay::ReplayStartComposition::capture(game, server_build_sha()) {
            Ok(start) => self.replay_start = Some(start),
            Err(err) => {
                self.replay_start = None;
                crate::log_warn!(
                    room = %self.room,
                    error = %err,
                    "failed to capture launch-time replay start"
                );
            }
        }
    }

    pub(super) fn finalize_replay_artifact(
        &self,
        game: &Game,
        winner_id: Option<u32>,
        scores: Vec<PlayerScore>,
        conclusion: Option<MatchConclusion>,
    ) -> Option<ReplayArtifactV1> {
        let Some(start) = &self.replay_start else {
            crate::log_warn!(
                room = %self.room,
                "cannot finalize replay artifact without launch-time start checkpoint"
            );
            return None;
        };
        let mut artifact = start.finalize(game, winner_id, scores);
        artifact.conclusion = conclusion;
        artifact.chat_log = self.match_chat_log.clone();
        Some(artifact)
    }
}
