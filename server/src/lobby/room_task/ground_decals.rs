use std::time::{Duration, Instant};

use super::super::connection::send_or_log;
use super::types::Phase;
use super::RoomTask;
use crate::protocol::ServerMessage;
use rts_sim::game::{Game, ObserverView};

const REQUEST_INTERVAL: Duration = Duration::from_millis(500);

impl RoomTask {
    pub(super) fn on_request_ground_decals(
        &mut self,
        player_id: u32,
        request_id: u32,
        after_revision: u32,
    ) {
        if request_id == 0 || matches!(&self.phase, Phase::Lobby | Phase::BranchStaging(_)) {
            return;
        }
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let now = Instant::now();
        if self
            .ground_decal_request_times
            .get(&player_id)
            .is_some_and(|last| now.duration_since(*last) < REQUEST_INTERVAL)
        {
            return;
        }
        self.ground_decal_request_times.insert(player_id, now);
        let explicit_view = self.observer_views.get(&player_id);
        let active_seat = self
            .live_connection_is_player(player_id)
            .then(|| self.live_seat_id_for_connection(player_id))
            .flatten();
        let full_world = self.is_dev_watch();
        let response = match &self.phase {
            Phase::InGame(game) => {
                projected_delta(game, explicit_view, active_seat, full_world, after_revision)
            }
            Phase::ReplayViewer(session) => projected_delta(
                session.game(),
                explicit_view,
                None,
                full_world,
                after_revision,
            ),
            Phase::Lobby | Phase::BranchStaging(_) => return,
        };
        let (revision, decals, tank_trails) = response;
        send_or_log(
            &self.room,
            player_id,
            &player.msg_tx,
            ServerMessage::GroundDecals {
                request_id,
                revision,
                decals,
                tank_trails,
            },
        );
    }
}

fn projected_delta(
    game: &Game,
    explicit_view: Option<&ObserverView>,
    active_seat: Option<u32>,
    full_world: bool,
    after_revision: u32,
) -> (
    u32,
    Vec<crate::protocol::GroundDecalView>,
    Vec<crate::protocol::TankTrailView>,
) {
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
