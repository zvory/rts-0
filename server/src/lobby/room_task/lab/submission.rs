use crate::lab_scenario_submission::{LabScenarioSubmissionError, LabScenarioSubmissionSuccess};
use crate::lab_scenarios::validate_lab_scenario_authoring;
use crate::protocol::{LabResult, LabScenarioAuthoringMetadata, ServerMessage};

use super::RoomTask;

const MAX_LAB_SCENARIO_PR_SUBMISSIONS_PER_ROOM: u8 = 1;

fn lab_submission_result_error(request_id: u32, error: LabScenarioSubmissionError) -> LabResult {
    LabResult {
        request_id,
        ok: false,
        op: "submitScenario".to_string(),
        error: Some(error.message.clone()),
        failed_index: None,
        details: None,
        outcome: Some(serde_json::json!({
            "code": error.code.as_str(),
            "message": error.message,
        })),
    }
}

fn lab_submission_result_success(
    request_id: u32,
    success: LabScenarioSubmissionSuccess,
) -> LabResult {
    LabResult {
        request_id,
        ok: true,
        op: "submitScenario".to_string(),
        error: None,
        failed_index: None,
        details: None,
        outcome: Some(serde_json::json!({
            "status": "submitted",
            "prUrl": success.pr_url,
            "branchName": success.branch_name,
            "scenarioPath": success.scenario_path,
            "manifestPath": success.manifest_path,
        })),
    }
}

fn lab_submission_validation_error(error: String) -> LabScenarioSubmissionError {
    if error.contains("duplicate lab scenario id")
        || error.contains("duplicate lab scenario filename")
        || error.contains("duplicate lab setup id")
        || error.contains("duplicate lab setup filename")
    {
        LabScenarioSubmissionError::duplicate_slug(error)
    } else {
        LabScenarioSubmissionError::validation(error)
    }
}

impl RoomTask {
    pub(super) fn submit_lab_scenario(
        &mut self,
        operator_id: u32,
        request_id: u32,
        metadata: LabScenarioAuthoringMetadata,
    ) -> Option<LabResult> {
        if !self.session_policy().allows_lab_scenario_io() {
            return Some(lab_submission_result_error(
                request_id,
                LabScenarioSubmissionError::validation(
                    "lab setup submission is not enabled in this room",
                ),
            ));
        }
        if let Some(error) = self.lab_scenario_submission.unavailable_error() {
            return Some(lab_submission_result_error(request_id, error));
        }
        if self
            .lab_session
            .as_ref()
            .map(|session| {
                session.scenario_submission_jobs_started >= MAX_LAB_SCENARIO_PR_SUBMISSIONS_PER_ROOM
            })
            .unwrap_or(true)
        {
            return Some(lab_submission_result_error(
                request_id,
                LabScenarioSubmissionError::rate_limit(
                    "this lab room has already started a setup PR submission",
                ),
            ));
        }

        let scenario_value = match self.export_lab_scenario_value(operator_id, None) {
            Ok(value) => value,
            Err(err) => {
                return Some(lab_submission_result_error(
                    request_id,
                    LabScenarioSubmissionError::validation(err),
                ));
            }
        };
        let scenario = match serde_json::from_value(scenario_value) {
            Ok(scenario) => scenario,
            Err(err) => {
                return Some(lab_submission_result_error(
                    request_id,
                    LabScenarioSubmissionError::validation(format!(
                        "setup export did not produce a lab setup payload: {err}"
                    )),
                ));
            }
        };
        let preview = match validate_lab_scenario_authoring(metadata, scenario) {
            Ok(preview) => preview,
            Err(err) => {
                return Some(lab_submission_result_error(
                    request_id,
                    lab_submission_validation_error(err),
                ));
            }
        };
        let Some(player) = self.players.get(&operator_id) else {
            return Some(lab_submission_result_error(
                request_id,
                LabScenarioSubmissionError::validation("lab operator is no longer connected"),
            ));
        };
        if let Some(session) = &mut self.lab_session {
            session.scenario_submission_jobs_started =
                session.scenario_submission_jobs_started.saturating_add(1);
        }

        let service = self.lab_scenario_submission.clone();
        let msg_tx = player.msg_tx.clone();
        let room = self.room.clone();
        tokio::spawn(async move {
            let result = match service.submit_preview(preview).await {
                Ok(success) => lab_submission_result_success(request_id, success),
                Err(error) => lab_submission_result_error(request_id, error),
            };
            if let Err(err) = msg_tx.send_reliable(ServerMessage::LabResult(result)).await {
                crate::log_debug!(
                    room = %room,
                    player_id = operator_id,
                    %err,
                    "failed to send lab setup submission result"
                );
            }
        });

        None
    }
}
