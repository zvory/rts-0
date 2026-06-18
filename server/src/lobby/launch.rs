use super::connection::{send_or_log, ConnectionSink};
use crate::protocol::{
    DiagnosticCapabilities, LabStartMetadata, ServerMessage, StartPayload,
    PREDICTION_PROTOCOL_VERSION,
};

#[derive(Clone, Copy)]
pub(super) enum LaunchPrediction {
    Enabled,
    Disabled,
}

pub(super) struct LaunchRecipient {
    pub(super) connection_id: u32,
    pub(super) payload_player_id: u32,
    pub(super) spectator: bool,
    pub(super) prediction: LaunchPrediction,
    pub(super) diagnostics: DiagnosticCapabilities,
    pub(super) clear_pending_snapshot: bool,
    pub(super) lab: Option<LabStartMetadata>,
    pub(super) msg_tx: ConnectionSink,
}

pub(super) fn send_start_payloads(
    room: &str,
    base_payload: &StartPayload,
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
            ServerMessage::Start(start_payload_for(base_payload, &recipient)),
        );
    }
}

fn start_payload_for(base_payload: &StartPayload, recipient: &LaunchRecipient) -> StartPayload {
    let (prediction_build_id, prediction_version) = match recipient.prediction {
        LaunchPrediction::Enabled => (
            Some(crate::build_info::build_id().to_string()),
            PREDICTION_PROTOCOL_VERSION,
        ),
        LaunchPrediction::Disabled => (None, 0),
    };
    StartPayload {
        player_id: recipient.payload_player_id,
        spectator: recipient.spectator,
        prediction_build_id,
        prediction_version,
        diagnostics: recipient.diagnostics,
        replay: None,
        lab: recipient.lab.clone().or_else(|| base_payload.lab.clone()),
        ..base_payload.clone()
    }
}
