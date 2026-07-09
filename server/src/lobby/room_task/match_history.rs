use super::{is_automated_match_history_room, match_history_participants_are_automated, RoomTask};
use crate::protocol::PlayerScore;
use rts_sim::game::replay::ReplayArtifactV1;

pub(super) struct MatchHistoryRecordInput<'a> {
    pub(super) started_at: chrono::DateTime<chrono::Utc>,
    pub(super) ended_at: chrono::DateTime<chrono::Utc>,
    pub(super) duration_ms: i32,
    pub(super) scores: &'a [PlayerScore],
    pub(super) replay_artifact: Option<&'a ReplayArtifactV1>,
    pub(super) outcome: crate::db::MatchOutcome,
    pub(super) winner_name: Option<String>,
}

impl RoomTask {
    pub(super) fn should_persist_match_history(&self) -> bool {
        let match_policy = self.live_session_policy();
        self.match_player_count >= 1
            && match_policy.has_authoritative_mutation()
            && match_policy.allows_match_history()
            && !is_automated_match_history_room(&self.room)
            && !match_history_participants_are_automated(&self.match_participants)
    }

    pub(super) fn match_history_debug_mode(&self) -> bool {
        self.match_player_count == 1 && self.match_human_count == 1
    }

    pub(super) fn should_capture_post_match_replay(&self) -> bool {
        let match_policy = self.live_session_policy();
        match_policy.captures_post_match_replay()
    }

    pub(super) fn should_attach_match_history_replay_artifact(&self) -> bool {
        let match_policy = self.live_session_policy();
        match_policy.attaches_match_history_replay_artifact()
    }

    pub(super) fn match_duration_ms_for(
        &self,
        ended_at: chrono::DateTime<chrono::Utc>,
    ) -> Option<i64> {
        self.match_started_at.map(|started_at| {
            ended_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .clamp(0, i32::MAX as i64)
        })
    }

    pub(super) fn will_record_match_history(&self) -> bool {
        self.match_history_writer.is_some()
            && self.match_started_at.is_some()
            && self.should_persist_match_history()
    }

    pub(super) fn build_match_history_record(
        &self,
        input: MatchHistoryRecordInput<'_>,
    ) -> crate::db::MatchRecord {
        let score_json = serde_json::to_value(input.scores).unwrap_or(serde_json::Value::Null);
        let replay = if self.should_attach_match_history_replay_artifact() {
            input.replay_artifact.and_then(|artifact| {
                match crate::db::MatchReplayRecord::from_artifact(artifact) {
                    Ok(replay) => Some(replay),
                    Err(err) => {
                        crate::log_warn!(
                            room = %self.room,
                            error = %err,
                            "failed to serialize replay artifact for match history"
                        );
                        None
                    }
                }
            })
        } else {
            None
        };
        crate::db::MatchRecord {
            match_run_id: self.match_run_id.clone(),
            started_at: input.started_at,
            ended_at: input.ended_at,
            duration_ms: input.duration_ms,
            map_name: self.match_map_name.clone(),
            winner_name: input.winner_name,
            outcome: input.outcome,
            participants: self.match_participants.clone(),
            score_screen: score_json,
            human_count: i32::try_from(self.match_human_count).unwrap_or(i32::MAX),
            debug_mode: self.match_history_debug_mode(),
            local_only: self.match_history_local_only,
            replay,
        }
    }

    pub(super) fn queue_match_history_write(&self, rec: crate::db::MatchRecord) -> bool {
        let Some(writer) = self.match_history_writer.clone() else {
            return false;
        };
        // Detached: a slow Supabase write must never stall room transitions. The drain-level
        // tracker lets shutdown wait on the task later; errors are logged inside `record_match`.
        self.drain
            .track_match_history_write(writer.record_match(rec));
        true
    }
}
