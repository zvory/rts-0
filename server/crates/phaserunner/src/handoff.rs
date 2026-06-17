use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffStatus {
    Completed,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoMergeState {
    Unknown,
    NotRequested,
    Armed,
    Missing,
    Disabled,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeWaitState {
    NotWaited,
    Waiting,
    Merged,
    Blocked,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorHandoff {
    pub status: HandoffStatus,
    pub summary: String,
    pub files_changed: Vec<String>,
    pub verification: Vec<String>,
    pub gameplay_impact: String,
    pub next_executor_notes: String,
    pub manual_test_notes: String,
    pub blocked_reason: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub head_sha: Option<String>,
    pub auto_merge_state: AutoMergeState,
    pub merge_wait_state: MergeWaitState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffError {
    Json(String),
    MissingBlockedReason,
    CompletedWithBlockedReason,
}

impl ExecutorHandoff {
    pub fn parse_json(input: &str) -> Result<Self, HandoffError> {
        let handoff: Self =
            serde_json::from_str(input).map_err(|err| HandoffError::Json(err.to_string()))?;
        handoff.validate()?;
        Ok(handoff)
    }

    pub fn validate(&self) -> Result<(), HandoffError> {
        match self.status {
            HandoffStatus::Blocked if self.blocked_reason.trim().is_empty() => {
                Err(HandoffError::MissingBlockedReason)
            }
            HandoffStatus::Completed if !self.blocked_reason.trim().is_empty() => {
                Err(HandoffError::CompletedWithBlockedReason)
            }
            _ => Ok(()),
        }
    }

    pub fn verification_summary(&self) -> String {
        let text = self
            .verification
            .iter()
            .filter(|item| !item.trim().is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        if text.is_empty() {
            "Focused verification not recorded by executor.".to_string()
        } else {
            text
        }
    }
}

impl fmt::Display for HandoffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "invalid handoff JSON: {err}"),
            Self::MissingBlockedReason => write!(f, "blocked handoff must include blocked_reason"),
            Self::CompletedWithBlockedReason => {
                write!(f, "completed handoff must not include blocked_reason")
            }
        }
    }
}

impl std::error::Error for HandoffError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_completed_handoff() {
        let handoff = ExecutorHandoff::parse_json(completed_json()).unwrap();
        assert_eq!(handoff.status, HandoffStatus::Completed);
        assert_eq!(
            handoff.verification_summary(),
            "cargo test -p rts-phaserunner"
        );
    }

    #[test]
    fn rejects_missing_fields_and_unknown_fields() {
        assert!(matches!(
            ExecutorHandoff::parse_json(r#"{"status":"completed"}"#),
            Err(HandoffError::Json(_))
        ));

        let with_unknown = completed_json().replace(
            r#""merge_wait_state":"not_waited""#,
            r#""merge_wait_state":"not_waited","extra":true"#,
        );
        assert!(matches!(
            ExecutorHandoff::parse_json(&with_unknown),
            Err(HandoffError::Json(_))
        ));
    }

    #[test]
    fn validates_blocked_reason_by_status() {
        let blocked_missing =
            completed_json().replace(r#""status":"completed""#, r#""status":"blocked""#);
        assert!(matches!(
            ExecutorHandoff::parse_json(&blocked_missing),
            Err(HandoffError::MissingBlockedReason)
        ));

        let completed_with_reason = completed_json().replace(
            r#""blocked_reason":"""#,
            r#""blocked_reason":"still blocked""#,
        );
        assert!(matches!(
            ExecutorHandoff::parse_json(&completed_with_reason),
            Err(HandoffError::CompletedWithBlockedReason)
        ));
    }

    fn completed_json() -> &'static str {
        r#"{
            "status":"completed",
            "summary":"done",
            "files_changed":["server/Cargo.toml"],
            "verification":["cargo test -p rts-phaserunner"],
            "gameplay_impact":"none",
            "next_executor_notes":"next",
            "manual_test_notes":"manual",
            "blocked_reason":"",
            "pr_number":null,
            "pr_url":null,
            "head_sha":null,
            "auto_merge_state":"not_requested",
            "merge_wait_state":"not_waited"
        }"#
    }
}
