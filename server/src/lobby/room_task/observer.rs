use std::collections::HashMap;
use std::time::Instant as StdInstant;

use super::super::connection::send_or_log;
use super::super::live_tick::fanout_current_observer_snapshots;
use super::super::projection::scope_observer_analysis;
use super::super::replay_session::ReplaySession;
use super::super::snapshot_fanout::fanout_replay_snapshots;
use super::types::{Phase, ReplayTickContext};
use super::RoomTask;
use crate::protocol::{Event, ServerMessage};
use rts_sim::game::ObserverView;

impl RoomTask {
    pub(super) fn fanout_current_observer_snapshots_to(
        &mut self,
        recipients: impl IntoIterator<Item = u32>,
    ) {
        let projection_policy = self.projection_policy();
        let tick_budget = self.current_tick_interval();
        let tick_start = StdInstant::now();
        let game = match std::mem::replace(&mut self.phase, Phase::Lobby) {
            Phase::InGame(game) => game,
            other => {
                self.phase = other;
                return;
            }
        };
        fanout_current_observer_snapshots(
            &self.room,
            &mut self.players,
            &self.observer_views,
            projection_policy,
            recipients,
            &mut self.slow_tick_count,
            tick_budget,
            tick_start,
            &game,
        );
        self.phase = Phase::InGame(game);
    }

    pub(super) fn fanout_replay_snapshots_to(
        &mut self,
        session: &ReplaySession,
        recipients: impl IntoIterator<Item = u32>,
        mut per_player_events: HashMap<u32, Vec<Event>>,
        context: ReplayTickContext,
        perf: Option<&mut rts_sim::perf::TickPerf>,
    ) {
        fanout_replay_snapshots(
            &self.room,
            &mut self.players,
            &self.observer_views,
            context.projection_policy,
            session,
            recipients,
            &mut per_player_events,
            context.scheduler_lag,
            context.tick_budget,
            context.tick_start,
            &mut self.slow_tick_count,
            perf,
        );
    }

    pub(super) fn send_scoped_replay_observer_analysis(
        &self,
        session: &ReplaySession,
        recipient_ids: impl IntoIterator<Item = u32>,
    ) {
        let analysis = session.game().observer_analysis();
        for id in recipient_ids {
            let Some(player) = self.players.get(&id) else {
                continue;
            };
            let view = self
                .observer_views
                .get(&id)
                .cloned()
                .unwrap_or_else(|| ObserverView::Players(session.active_player_ids()));
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::ObserverAnalysis(scope_observer_analysis(analysis.clone(), &view)),
            );
        }
    }
}
