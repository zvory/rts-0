use super::*;

impl ReplaySession {
    pub(in crate::lobby) fn result_messages(&self) -> Vec<ServerMessage> {
        let mut messages: Vec<_> = self
            .integrity_warning()
            .map(|msg| ServerMessage::Error { msg })
            .into_iter()
            .collect();
        messages.push(ServerMessage::GameOver {
            winner_id: self.artifact.winner_id,
            winner_team_id: self.artifact.winner_team_id,
            conclusion: self.artifact.conclusion.clone(),
            you: "draw".to_string(),
            scores: self.artifact.final_scores.clone(),
        });
        messages
    }

    pub(in crate::lobby) fn send_result_messages(
        room: &str,
        id: u32,
        sink: &crate::lobby::connection::ConnectionSink,
        messages: &[ServerMessage],
        include_score: bool,
    ) {
        for message in messages {
            if include_score || matches!(message, ServerMessage::Error { .. }) {
                crate::lobby::connection::send_or_log(room, id, sink, message.clone());
            }
        }
    }

    pub(super) fn validate_eliminations(
        artifact: &ReplayArtifactV1,
        start_tick: u32,
    ) -> Result<(), String> {
        if artifact.eliminations.len() > 1_024 {
            return Err("replay has too many administrative eliminations".into());
        }
        let mut previous = start_tick;
        for entry in &artifact.eliminations {
            if entry.tick < previous
                || entry.tick > artifact.duration_ticks
                || !artifact.players.iter().any(|p| p.id == entry.player_id)
            {
                return Err("invalid replay administrative elimination timeline".into());
            }
            previous = entry.tick;
        }
        Ok(())
    }

    pub(super) fn verify_end_state_if_due(&mut self) {
        if self.has_remaining_ticks() || self.end_state_verification.is_some() {
            return;
        }
        let started = StdInstant::now();
        let result = self.artifact.verify_end_state(&self.game);
        match &result {
            Ok(Some(true)) => {
                crate::log_info!(tick = self.current_tick(), "replay end-state verified")
            }
            Ok(Some(false)) => crate::log_warn!(
                tick = self.current_tick(),
                "replay end-state divergence detected"
            ),
            Ok(None) => crate::log_info!("replay has no recorded end-state hash; unverified"),
            Err(error) => crate::log_warn!(%error, "replay end-state verification unavailable"),
        }
        crate::log_info!(
            hash_us = started.elapsed().as_micros() as u64,
            "replay end-state verification cost"
        );
        self.end_state_verification = Some(result);
    }

    pub(in crate::lobby) fn integrity_warning(&self) -> Option<String> {
        match self.end_state_verification.as_ref()? {
            Ok(Some(false)) => Some("Replay divergence detected: the final game state differs from the recorded match. Build or platform differences can cause this.".into()),
            Err(error) => Some(format!("Replay end-state could not be verified: {error}")),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{replay_test_game, replay_test_players};
    use super::*;
    use rts_sim::game::replay::{ReplayElimination, ReplayStartComposition};

    #[test]
    fn replay_end_state_verifies_give_up_at_zero_and_after_ticks_and_seek() {
        for end_tick in [0, 3] {
            let players = replay_test_players(2);
            let mut live = replay_test_game(&players, 42);
            let mut start =
                ReplayStartComposition::capture(&live, crate::build_info::build_id()).unwrap();
            for _ in 0..end_tick {
                live.tick();
            }
            start.record_elimination(&live, 2);
            live.eliminate(2);
            let artifact = start.finalize(&live, Some(1), live.scores());
            let mut replay = ReplaySession::new(artifact).unwrap();
            while replay.has_remaining_ticks() {
                replay.enqueue_for_current_tick().unwrap();
                replay.tick(None);
                replay.record_keyframe_if_due();
            }
            assert_eq!(replay.end_state_verification, Some(Ok(Some(true))));
            replay
                .begin_seek(
                    99,
                    ReplaySeekPlan {
                        from_tick: end_tick,
                        target_tick: 0,
                    },
                )
                .unwrap();
            replay
                .begin_seek(
                    99,
                    ReplaySeekPlan {
                        from_tick: 0,
                        target_tick: end_tick,
                    },
                )
                .unwrap();
            while replay.is_seeking() {
                replay
                    .advance_seek_slice(10, Duration::MAX, Duration::ZERO)
                    .unwrap();
            }
            assert_eq!(replay.end_state_verification, Some(Ok(Some(true))));
        }
    }

    #[test]
    fn replay_end_state_detects_tampering_and_accepts_unverified_older_artifact() {
        let (_, mut artifact) =
            super::super::tests::replay_test_artifact(&replay_test_players(2), 0);
        artifact.end_state_hash.as_mut().unwrap().digest = "0".repeat(64);
        let replay = ReplaySession::new(artifact.clone()).unwrap();
        assert!(replay.integrity_warning().unwrap().contains("divergence"));
        artifact.end_state_hash = None;
        let replay = ReplaySession::new(artifact).unwrap();
        assert_eq!(replay.end_state_verification, Some(Ok(None)));
        assert!(replay.integrity_warning().is_none());
    }

    #[test]
    fn replay_elimination_timeline_rejects_unknown_players_and_out_of_order_entries() {
        let (_, mut artifact) =
            super::super::tests::replay_test_artifact(&replay_test_players(2), 3);
        artifact.eliminations = vec![ReplayElimination {
            tick: 0,
            player_id: 99,
        }];
        assert!(ReplaySession::new(artifact.clone()).is_err());
        artifact.eliminations = vec![
            ReplayElimination {
                tick: 2,
                player_id: 1,
            },
            ReplayElimination {
                tick: 1,
                player_id: 2,
            },
        ];
        assert!(ReplaySession::new(artifact.clone()).is_err());
        artifact.eliminations = vec![ReplayElimination {
            tick: 4,
            player_id: 2,
        }];
        assert!(ReplaySession::new(artifact).is_err());
    }
}
