use super::connection::{send_or_log, ConnectionSink};
use super::projection::RecipientRole;
use super::session_policy::{SessionPolicy, StartPayloadPolicy};
use crate::protocol::{
    DiagnosticCapabilities, LabStartMetadata, ObserverViewSelection, ReplayStartMetadata,
    ServerMessage, StartPayload, PREDICTION_PROTOCOL_VERSION,
};

#[derive(Clone, Copy)]
pub(super) enum LaunchPrediction {
    Enabled,
    Disabled,
}

pub(super) struct LaunchRecipient {
    pub(super) connection_id: u32,
    pub(super) payload_player_id: u32,
    pub(super) role: RecipientRole,
    pub(super) prediction: LaunchPrediction,
    pub(super) diagnostics: DiagnosticCapabilities,
    pub(super) clear_pending_snapshot: bool,
    pub(super) lab: Option<LabStartMetadata>,
    pub(super) observer_view: Option<ObserverViewSelection>,
    pub(super) msg_tx: ConnectionSink,
}

pub(super) struct StartPayloadRecipient {
    pub(super) payload_player_id: u32,
    pub(super) role: RecipientRole,
    pub(super) prediction: LaunchPrediction,
    pub(super) diagnostics: DiagnosticCapabilities,
    pub(super) lab: Option<LabStartMetadata>,
    pub(super) observer_view: Option<ObserverViewSelection>,
}

impl LaunchRecipient {
    pub(super) fn observer(
        connection_id: u32,
        diagnostics: DiagnosticCapabilities,
        clear_pending_snapshot: bool,
        lab: Option<LabStartMetadata>,
        observer_view: ObserverViewSelection,
        msg_tx: ConnectionSink,
    ) -> Self {
        Self {
            connection_id,
            payload_player_id: connection_id,
            role: RecipientRole::Spectator,
            prediction: LaunchPrediction::Disabled,
            diagnostics,
            clear_pending_snapshot,
            lab,
            observer_view: Some(observer_view),
            msg_tx,
        }
    }

    fn start_payload_recipient(&self) -> StartPayloadRecipient {
        StartPayloadRecipient {
            payload_player_id: self.payload_player_id,
            role: self.role,
            prediction: self.prediction,
            diagnostics: self.diagnostics,
            lab: self.lab.clone(),
            observer_view: self.observer_view.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) enum StartPayloadSource {
    Simulation,
    Replay {
        metadata: ReplayStartMetadata,
        branch_available: bool,
    },
}

pub(super) struct StartPayloadBuilder<'a> {
    policy: SessionPolicy,
    source: StartPayloadSource,
    base_payload: &'a StartPayload,
}

impl<'a> StartPayloadBuilder<'a> {
    pub(super) fn simulation(policy: SessionPolicy, base_payload: &'a StartPayload) -> Self {
        Self {
            policy,
            source: StartPayloadSource::Simulation,
            base_payload,
        }
    }

    pub(super) fn replay(
        policy: SessionPolicy,
        base_payload: &'a StartPayload,
        metadata: ReplayStartMetadata,
        branch_available: bool,
    ) -> Self {
        Self {
            policy,
            source: StartPayloadSource::Replay {
                metadata,
                branch_available,
            },
            base_payload,
        }
    }

    pub(super) fn build(&self, recipient: &StartPayloadRecipient) -> StartPayload {
        let active_player = recipient.role == RecipientRole::ActivePlayer;
        let mut capabilities = self.policy.start_capabilities(active_player);
        let replay = match (&self.source, self.policy.start_payload) {
            (
                StartPayloadSource::Replay {
                    metadata,
                    branch_available,
                },
                StartPayloadPolicy::ReplayViewer,
            ) => {
                capabilities.actions.branch_from_tick = *branch_available;
                Some(metadata.clone())
            }
            _ => None,
        };
        let lab = match self.policy.start_payload {
            StartPayloadPolicy::Lab => recipient
                .lab
                .clone()
                .or_else(|| self.base_payload.lab.clone()),
            _ => None,
        };
        let (prediction_build_id, prediction_version) =
            match (self.policy.start_payload, recipient.prediction) {
                (
                    StartPayloadPolicy::LiveMatch
                    | StartPayloadPolicy::ReplayBranchLive
                    | StartPayloadPolicy::DevWatch,
                    LaunchPrediction::Enabled,
                ) => (
                    Some(crate::build_info::build_id().to_string()),
                    PREDICTION_PROTOCOL_VERSION,
                ),
                _ => (None, 0),
            };

        StartPayload {
            player_id: recipient.payload_player_id,
            spectator: !active_player,
            prediction_build_id,
            prediction_version,
            capabilities,
            diagnostics: recipient.diagnostics,
            replay,
            lab,
            observer_view: recipient.observer_view.clone(),
            ..self.base_payload.clone()
        }
    }
}

pub(super) fn send_start_payloads(
    room: &str,
    builder: &StartPayloadBuilder<'_>,
    recipients: impl IntoIterator<Item = LaunchRecipient>,
) {
    for recipient in recipients {
        if recipient.clear_pending_snapshot {
            recipient.msg_tx.clear_pending_snapshot();
        }
        send_or_log(
            room,
            recipient.connection_id,
            &recipient.msg_tx,
            ServerMessage::Start(builder.build(&recipient.start_payload_recipient())),
        );
    }
}
