use std::collections::HashMap;
use std::time::Instant as StdInstant;

use super::super::connection::send_or_log;
use super::super::live_tick::fanout_current_observer_snapshots;
use super::super::projection::{observer_view_or_all, scope_observer_analysis};
use super::super::replay_session::ReplaySession;
use super::super::snapshot_fanout::fanout_replay_snapshots;
use super::types::{Phase, ReplayTickContext};
use super::RoomTask;
use crate::protocol::{Event, ServerMessage};
use rts_sim::game::{Game, ObserverView};

const GROUND_DECAL_REQUEST_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

impl RoomTask {
    pub(super) fn on_request_ground_decals(&mut self, player_id: u32, after_revision: u32) {
        let now = StdInstant::now();
        if self
            .ground_decal_request_times
            .get(&player_id)
            .is_some_and(|last| now.duration_since(*last) < GROUND_DECAL_REQUEST_INTERVAL)
        {
            return;
        }
        self.ground_decal_request_times.insert(player_id, now);
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let explicit_view = self.observer_views.get(&player_id);
        let active_seat = self
            .live_connection_is_player(player_id)
            .then(|| self.live_seat_id_for_connection(player_id))
            .flatten();
        let full_world = self.is_dev_watch();
        let response = match &self.phase {
            Phase::InGame(game) => {
                ground_decal_response(game, explicit_view, active_seat, full_world, after_revision)
            }
            Phase::ReplayViewer(session) => ground_decal_response(
                session.game(),
                explicit_view,
                None,
                full_world,
                after_revision,
            ),
            Phase::Lobby | Phase::BranchStaging(_) => return,
        };
        let (revision, decals) = response;
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::GroundDecals { revision, decals },
        );
    }

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
        per_player_events: HashMap<u32, Vec<Event>>,
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
            &per_player_events,
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
            let view = observer_view_or_all(self.observer_views.get(&id), session.game());
            send_or_log(
                &self.room,
                id,
                &player.msg_tx,
                ServerMessage::ObserverAnalysis(scope_observer_analysis(analysis.clone(), &view)),
            );
        }
    }
}

fn ground_decal_response(
    game: &Game,
    explicit_view: Option<&ObserverView>,
    active_seat: Option<u32>,
    full_world: bool,
    after_revision: u32,
) -> (u32, Vec<crate::protocol::GroundDecalView>) {
    if full_world {
        return game.ground_decals_for_observer(&ObserverView::Omniscient, after_revision);
    }
    if let Some(view) = explicit_view {
        return game.ground_decals_for_observer(view, after_revision);
    }
    if let Some(player_id) = active_seat {
        return game.ground_decals_for_player(player_id, after_revision);
    }
    let all_players = ObserverView::Players(
        game.player_inits()
            .into_iter()
            .map(|player| player.id)
            .collect(),
    );
    game.ground_decals_for_observer(&all_players, after_revision)
}
