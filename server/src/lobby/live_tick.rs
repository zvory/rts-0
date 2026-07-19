use super::connection::send_or_log;
use super::connection::CommandSimAckSample;
use super::crash_replay::{dump_crash_replay, panic_reason};
use super::participants::Participants;
use super::projection::{
    scope_observer_analysis, ObserverAnalysisAudience, ProjectionPolicy, RecipientRole,
};
use super::room_task::{PendingClientCommandAck, RoomPlayer};
use super::snapshot_fanout::{SnapshotFanout, SnapshotFanoutPayload};
use super::snapshots::union_events;
use crate::protocol::{
    Event, ObserverAnalysisAiDiagnostics, ObserverAnalysisPayload, PlayerScore, ServerMessage,
};
use rts_ai::{AiController, AiThinkContext};
use rts_sim::game::replay::ReplayStartComposition;
use rts_sim::game::Game;
use rts_sim::game::ObserverView;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant as StdInstant};
use tokio::time::Instant as TokioInstant;

pub(super) enum LiveTickResult {
    Continue(Box<Game>),
    EndMatch {
        game: Box<Game>,
        winner_id: Option<u32>,
        scores: Vec<PlayerScore>,
    },
    PanicEnd {
        scores: Vec<PlayerScore>,
    },
}

pub(super) struct LiveTickDriver<'a> {
    pub(super) room: &'a str,
    pub(super) scheduled: TokioInstant,
    pub(super) tick_budget: Duration,
    pub(super) match_run_id: Option<&'a str>,
    pub(super) match_player_count: usize,
    pub(super) ai_player_count: usize,
    pub(super) players: &'a mut HashMap<u32, RoomPlayer>,
    pub(super) order: &'a [u32],
    pub(super) outcome_sent: &'a mut HashSet<u32>,
    pub(super) branch_live_seat_by_connection: &'a HashMap<u32, u32>,
    pub(super) ai_controllers: &'a mut [AiController],
    pub(super) pending_client_command_acks: &'a mut Vec<PendingClientCommandAck>,
    pub(super) pending_recipient_notices: &'a mut HashMap<u32, Vec<Event>>,
    pub(super) slow_tick_count: &'a mut u32,
    pub(super) observer_views: HashMap<u32, ObserverView>,
    pub(super) observer_include_private_notices: bool,
    pub(super) projection_policy: ProjectionPolicy,
    pub(super) replay_start: Option<&'a ReplayStartComposition>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fanout_current_observer_snapshots(
    room: &str,
    players: &mut HashMap<u32, RoomPlayer>,
    observer_views: &HashMap<u32, ObserverView>,
    projection_policy: ProjectionPolicy,
    recipients: impl IntoIterator<Item = u32>,
    slow_tick_count: &mut u32,
    tick_budget: Duration,
    tick_start: StdInstant,
    game: &Game,
) {
    let mut per_player_events = HashMap::new();
    let default_view =
        ObserverView::Players(game.player_inits().iter().map(|player| player.id).collect());
    SnapshotFanout::new(
        room,
        Duration::ZERO,
        tick_budget,
        tick_start,
        slow_tick_count,
        None,
    )
    .send_to_recipients(players, recipients, |id, player| {
        let projection = projection_policy.selected_perspective_snapshot_for(
            observer_views
                .get(&id)
                .cloned()
                .unwrap_or_else(|| default_view.clone()),
        );
        let snapshot = projection.snapshot_with_events(game, &mut per_player_events, &[]);
        Some(SnapshotFanoutPayload::new(snapshot, player.spectator))
    });
}

impl LiveTickDriver<'_> {
    pub(super) fn run(mut self, mut game: Box<Game>) -> LiveTickResult {
        let scheduler_lag = self.scheduled.elapsed();
        let tick_start = StdInstant::now();
        let mut perf = rts_sim::perf::TickPerf::maybe_new();

        let tick_result = self.tick_game(&mut game, perf.as_mut());
        let mut per_player_events: HashMap<u32, Vec<Event>> = match tick_result {
            Ok(events) => events.into_iter().collect(),
            Err(payload) => {
                let reason = panic_reason(&payload);
                dump_crash_replay(self.room, &game, self.replay_start, &reason);
                self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
                return LiveTickResult::PanicEnd {
                    scores: game.scores(),
                };
            }
        };

        self.record_consumed_client_sequences(game.tick_count());
        self.fan_out_snapshots(
            &game,
            &mut per_player_events,
            scheduler_lag,
            tick_start,
            perf.as_mut(),
        );
        self.broadcast_observer_analysis(&game);

        let outcome_start = StdInstant::now();
        let alive = self.outcome_alive_players(&game);
        let alive_teams = alive_team_ids_for(&game, &alive);
        if self.match_player_count >= 2 && alive_teams.len() <= 1 {
            self.send_observation_ready();
            if let Some(perf) = perf.as_mut() {
                perf.record_phase("outcome_checks", outcome_start.elapsed());
            }
            self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
            let winner_id = alive_teams
                .first()
                .and_then(|team_id| first_alive_player_on_team(&game, &alive, *team_id));
            return LiveTickResult::EndMatch {
                scores: game.scores(),
                game,
                winner_id,
            };
        }

        // A watched AI-only matchup needs a stable conclusion even when neither strategy can
        // finish its opponent. A decisive base kill on this tick still wins; otherwise the
        // horizon resolves the run as a normal draw and finalizes its replay.
        if match_tick_limit_reached(
            game.tick_count(),
            ai_observation_tick_limit(self.match_player_count, self.ai_player_count),
        ) {
            self.send_observation_ready();
            if let Some(perf) = perf.as_mut() {
                perf.record_phase("outcome_checks", outcome_start.elapsed());
            }
            self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
            return LiveTickResult::EndMatch {
                scores: game.scores(),
                game,
                winner_id: None,
            };
        }

        if self.match_player_count >= 2 {
            self.send_new_defeats(&game, &alive);
        }
        if let Some(perf) = perf.as_mut() {
            perf.record_phase("outcome_checks", outcome_start.elapsed());
        }

        self.finish_perf_tick(perf.as_ref(), &game, scheduler_lag, tick_start);
        LiveTickResult::Continue(game)
    }

    fn tick_game(
        &mut self,
        game: &mut Game,
        perf: Option<&mut rts_sim::perf::TickPerf>,
    ) -> std::thread::Result<Vec<(u32, Vec<Event>)>> {
        let mut perf = perf;
        let game_tick_start = StdInstant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.enqueue_ai_commands(game, perf.as_deref_mut());
            game.tick_with_perf(perf.as_deref_mut())
        }));
        if let Some(perf) = perf {
            perf.record_phase("game_tick", game_tick_start.elapsed());
        }
        result
    }

    fn enqueue_ai_commands(&mut self, game: &mut Game, perf: Option<&mut rts_sim::perf::TickPerf>) {
        rts_sim::perf::timed(perf, "ai_think", || {
            if self.ai_controllers.is_empty() {
                return;
            }
            let start = game.start_payload();
            let alive_player_ids = self.outcome_alive_players(game);
            let mut commands = Vec::new();
            for controller in self.ai_controllers.iter_mut() {
                let player_id = controller.player_id();
                if !alive_player_ids.contains(&player_id) {
                    continue;
                }
                let snapshot = game.snapshot_for(player_id);
                commands.extend(
                    controller
                        .think(AiThinkContext {
                            start: &start,
                            snapshot: &snapshot,
                            alive_player_ids: &alive_player_ids,
                            retreat_commands: game.worker_retreat_commands_for(player_id),
                        })
                        .into_iter()
                        .map(|command| (player_id, command)),
                );
            }
            for (player_id, command) in commands {
                game.enqueue(player_id, command);
            }
        });
    }

    fn outcome_alive_players(&self, game: &Game) -> Vec<u32> {
        if ai_only_match(self.match_player_count, self.ai_player_count) {
            game.primary_base_alive_players()
        } else {
            game.alive_players()
        }
    }

    fn send_observation_ready(&self) {
        if !ai_only_match(self.match_player_count, self.ai_player_count) {
            return;
        }
        let Some(match_run_id) = self.match_run_id else {
            return;
        };
        for id in self.order {
            let Some(player) = self.players.get(id) else {
                continue;
            };
            send_or_log(
                self.room,
                *id,
                &player.msg_tx,
                ServerMessage::ObservationReady {
                    match_run_id: match_run_id.to_string(),
                },
            );
        }
    }

    fn fan_out_snapshots(
        &mut self,
        game: &Game,
        per_player_events: &mut HashMap<u32, Vec<Event>>,
        scheduler_lag: Duration,
        tick_start: StdInstant,
        mut perf: Option<&mut rts_sim::perf::TickPerf>,
    ) {
        let full_vision_events = match perf.as_mut() {
            Some(perf) => rts_sim::perf::timed(Some(&mut **perf), "event_union", || {
                union_events(per_player_events.values())
            }),
            None => rts_sim::perf::timed(None, "event_union", || {
                union_events(per_player_events.values())
            }),
        };
        let recipients: Vec<u32> = self
            .order
            .iter()
            .copied()
            .filter(|id| !self.outcome_sent.contains(id))
            .collect();
        let branch_live_seat_by_connection = self.branch_live_seat_by_connection;
        let observer_views = self.observer_views.clone();
        let default_observer_view =
            ObserverView::Players(game.player_inits().iter().map(|player| player.id).collect());
        let pending_recipient_notices = &*self.pending_recipient_notices;

        let delivered_recipients = SnapshotFanout::new(
            self.room,
            scheduler_lag,
            self.tick_budget,
            tick_start,
            self.slow_tick_count,
            perf,
        )
        .send_to_recipients(self.players, recipients, |id, player| {
            let role = if player.spectator {
                RecipientRole::Spectator
            } else {
                RecipientRole::ActivePlayer
            };
            let projection = if role == RecipientRole::Spectator {
                self.projection_policy.observer_snapshot_for(
                    observer_views
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| default_observer_view.clone()),
                    self.observer_include_private_notices,
                )
            } else {
                self.projection_policy.live_snapshot_for(
                    role,
                    id,
                    branch_live_seat_by_connection.get(&id).copied(),
                    &[],
                )
            };
            let snapshot =
                projection.snapshot_with_events(game, per_player_events, &full_vision_events);
            let mut snapshot = snapshot;
            if let Some(notices) = pending_recipient_notices.get(&id) {
                snapshot.events.extend(notices.iter().cloned());
            }
            Some(SnapshotFanoutPayload::new(snapshot, player.spectator))
        });
        for id in delivered_recipients {
            self.pending_recipient_notices.remove(&id);
        }
    }

    fn broadcast_observer_analysis(&self, game: &Game) {
        if self.projection_policy.observer_analysis_audience()
            != ObserverAnalysisAudience::SpectatorRecipients
        {
            return;
        }
        let spectator_ids: Vec<u32> = self
            .order
            .iter()
            .copied()
            .filter(|id| self.players.get(id).is_some_and(|player| player.spectator))
            .collect();
        if spectator_ids.is_empty() {
            return;
        }

        let full_analysis = self.observer_analysis_with_ai_diagnostics(game);
        let default_view =
            ObserverView::Players(game.player_inits().iter().map(|player| player.id).collect());
        for id in spectator_ids {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            let view = self
                .observer_views
                .get(&id)
                .cloned()
                .unwrap_or_else(|| default_view.clone());
            send_or_log(
                self.room,
                id,
                &player.msg_tx,
                ServerMessage::ObserverAnalysis(scope_observer_analysis(
                    full_analysis.clone(),
                    &view,
                )),
            );
        }
    }

    fn observer_analysis_with_ai_diagnostics(&self, game: &Game) -> ObserverAnalysisPayload {
        let mut analysis = game.observer_analysis();
        analysis.map_analysis = self
            .ai_controllers
            .iter()
            .find_map(|controller| controller.latest_map_analysis_diagnostics());
        if let Some(map_analysis) = analysis.map_analysis.as_mut() {
            for layer in self
                .ai_controllers
                .iter()
                .flat_map(|controller| controller.latest_debug_map_layers())
            {
                map_analysis.layers.push(layer);
            }
        }
        for controller in self.ai_controllers.iter() {
            let Some(trace) = controller.latest_decision_trace() else {
                continue;
            };
            let Some(player) = analysis
                .players
                .iter_mut()
                .find(|player| player.id == trace.player_id)
            else {
                continue;
            };
            player.ai_diagnostics = Some(ObserverAnalysisAiDiagnostics {
                profile_id: trace.profile_id.to_string(),
                trace_tick: trace.trace_tick,
                lines: trace.lines,
            });
        }
        analysis
    }

    fn send_new_defeats(&mut self, game: &Game, alive: &[u32]) {
        let alive: HashSet<u32> = alive.iter().copied().collect();
        let recipients: Vec<u32> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.live_connection_is_player(*id)
                    && self
                        .live_seat_id_for_connection(*id)
                        .map(|seat_id| {
                            !alive.contains(&seat_id) && !game.team_has_alive_player(seat_id)
                        })
                        .unwrap_or(false)
                    && !self.outcome_sent.contains(id)
            })
            .collect();
        if recipients.is_empty() {
            return;
        }

        let scores = game.scores();
        for id in recipients {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            send_or_log(
                self.room,
                id,
                &player.msg_tx,
                ServerMessage::GameOver {
                    winner_id: None,
                    winner_team_id: None,
                    you: "lost".to_string(),
                    scores: scores.clone(),
                },
            );
            self.outcome_sent.insert(id);
        }
    }

    fn live_seat_id_for_connection(&self, connection_id: u32) -> Option<u32> {
        self.participants()
            .live_seat_id_for_connection(connection_id)
    }

    fn live_connection_is_player(&self, connection_id: u32) -> bool {
        self.participants().live_connection_is_player(connection_id)
    }

    fn participants(&self) -> Participants<'_> {
        Participants::new(
            self.order,
            self.players,
            self.branch_live_seat_by_connection,
        )
    }

    fn record_consumed_client_sequences(&mut self, tick: u32) {
        let pending = std::mem::take(self.pending_client_command_acks);
        for ack in pending {
            let Some(player) = self.players.get_mut(&ack.connection_id) else {
                continue;
            };
            if ack.client_seq == player.last_sim_consumed_client_seq.saturating_add(1) {
                player.last_sim_consumed_client_seq = ack.client_seq;
                player.last_sim_consumed_client_tick = Some(tick);
                player.msg_tx.record_command_sim_ack(CommandSimAckSample {
                    received_unix_ms: ack.received_unix_ms,
                    client_seq: ack.client_seq,
                    family: ack.family,
                    accepted_to_sim_ack_ms: duration_ms_u32(ack.accepted_at.elapsed()),
                });
            }
        }
    }

    fn finish_perf_tick(
        &self,
        perf: Option<&rts_sim::perf::TickPerf>,
        game: &Game,
        scheduler_lag: Duration,
        tick_start: StdInstant,
    ) {
        let Some(perf) = perf else {
            return;
        };
        perf.finish(rts_sim::perf::TickContext {
            room: self.room,
            match_run_id: self.match_run_id.unwrap_or(""),
            tick: game.current_tick(),
            scheduler_lag,
            total: tick_start.elapsed(),
            players: self.players.values().filter(|p| !p.spectator).count(),
            spectators: self.players.values().filter(|p| p.spectator).count(),
            ai_players: self.ai_player_count,
            counts: game.perf_entity_counts(),
        });
    }
}

fn duration_ms_u32(duration: Duration) -> u32 {
    duration.as_millis().min(u32::MAX as u128) as u32
}

fn ai_only_match(match_player_count: usize, ai_player_count: usize) -> bool {
    match_player_count >= 2 && match_player_count == ai_player_count
}

pub(super) fn ai_observation_tick_limit(
    match_player_count: usize,
    ai_player_count: usize,
) -> Option<u32> {
    ai_only_match(match_player_count, ai_player_count).then_some(25_000)
}

fn match_tick_limit_reached(tick: u32, tick_limit: Option<u32>) -> bool {
    tick_limit.is_some_and(|limit| tick >= limit)
}

fn alive_team_ids_for(game: &Game, alive: &[u32]) -> Vec<u32> {
    let mut teams = Vec::new();
    for player_id in alive {
        let Some(team_id) = game.team_of_player(*player_id) else {
            continue;
        };
        if team_id != 0 && !teams.contains(&team_id) {
            teams.push(team_id);
        }
    }
    teams
}

fn first_alive_player_on_team(game: &Game, alive: &[u32], team_id: u32) -> Option<u32> {
    if team_id == 0 {
        return None;
    }
    alive
        .iter()
        .copied()
        .find(|player_id| game.team_of_player(*player_id) == Some(team_id))
}

#[cfg(test)]
mod tests {
    use super::{ai_observation_tick_limit, match_tick_limit_reached};

    #[test]
    fn match_tick_limit_resolves_on_its_exact_tick_only() {
        assert!(!match_tick_limit_reached(24_999, Some(25_000)));
        assert!(match_tick_limit_reached(25_000, Some(25_000)));
        assert!(match_tick_limit_reached(25_001, Some(25_000)));
        assert!(!match_tick_limit_reached(25_000, None));
    }

    #[test]
    fn only_all_ai_matchups_get_the_observation_horizon() {
        assert_eq!(ai_observation_tick_limit(2, 2), Some(25_000));
        assert_eq!(ai_observation_tick_limit(2, 1), None);
        assert_eq!(ai_observation_tick_limit(1, 1), None);
    }
}
