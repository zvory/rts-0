use super::connection::send_or_log;
use super::projection::{
    observer_view_or_all, scope_observer_analysis, ObserverAnalysisAudience, ProjectionPolicy,
};
use super::replay_session::ReplaySession;
use super::room_task::RoomTask;
use crate::protocol::ServerMessage;

impl RoomTask {
    pub(in crate::lobby) fn send_scoped_replay_observer_analysis(
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

    pub(in crate::lobby) fn send_replay_resource_history(
        &self,
        session: &ReplaySession,
        recipient_ids: impl IntoIterator<Item = u32>,
        replace: bool,
    ) {
        if !(2..=4).contains(&session.artifact.players.len()) {
            return;
        }
        let samples = if replace {
            session.resource_history.samples.clone()
        } else {
            session
                .resource_history
                .samples
                .last()
                .cloned()
                .into_iter()
                .collect()
        };
        for id in recipient_ids {
            if let Some(player) = self.players.get(&id) {
                send_or_log(
                    &self.room,
                    id,
                    &player.msg_tx,
                    ServerMessage::ReplayResourceHistory {
                        replace,
                        samples: samples.clone(),
                    },
                );
            }
        }
    }

    pub(in crate::lobby) fn broadcast_observer_analysis_for(
        &self,
        session: &ReplaySession,
        projection_policy: ProjectionPolicy,
    ) {
        if projection_policy.observer_analysis_audience() != ObserverAnalysisAudience::AllRecipients
        {
            return;
        }
        self.send_scoped_replay_observer_analysis(session, self.order.clone());
    }
}
