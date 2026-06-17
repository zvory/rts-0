use serde::Deserialize;
use std::fmt;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitHubPullRequest {
    pub number: u64,
    pub url: String,
    pub state: String,
    pub head_ref_oid: Option<String>,
    pub head_ref_name: Option<String>,
    pub auto_merge_request: Option<serde_json::Value>,
    pub merge_state_status: Option<String>,
    pub is_draft: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrReadiness {
    pub number: u64,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrReadinessError {
    Json(String),
    MissingPr {
        branch: String,
    },
    NotOpen {
        number: u64,
        state: String,
        url: String,
    },
    MissingAutoMerge {
        number: u64,
        url: String,
    },
    Dirty {
        number: u64,
        url: String,
    },
}

pub fn ensure_pr_ready(pr_json: &str, branch: &str) -> Result<PrReadiness, PrReadinessError> {
    let prs: Vec<GitHubPullRequest> =
        serde_json::from_str(pr_json).map_err(|err| PrReadinessError::Json(err.to_string()))?;
    let pr = prs.first().ok_or_else(|| PrReadinessError::MissingPr {
        branch: branch.to_string(),
    })?;
    if pr.state != "OPEN" {
        return Err(PrReadinessError::NotOpen {
            number: pr.number,
            state: pr.state.clone(),
            url: pr.url.clone(),
        });
    }
    if pr.auto_merge_request.is_none() {
        return Err(PrReadinessError::MissingAutoMerge {
            number: pr.number,
            url: pr.url.clone(),
        });
    }
    if pr.merge_state_status.as_deref() == Some("DIRTY") {
        return Err(PrReadinessError::Dirty {
            number: pr.number,
            url: pr.url.clone(),
        });
    }

    Ok(PrReadiness {
        number: pr.number,
        url: pr.url.clone(),
    })
}

impl fmt::Display for PrReadinessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "invalid gh PR JSON: {err}"),
            Self::MissingPr { branch } => {
                write!(f, "agent-pr did not leave an open PR for {branch}")
            }
            Self::NotOpen { number, state, url } => {
                write!(f, "PR #{number} is not open ({state}): {url}")
            }
            Self::MissingAutoMerge { number, url } => {
                write!(f, "PR #{number} is missing auto-merge: {url}")
            }
            Self::Dirty { number, url } => write!(f, "PR #{number} has merge conflicts: {url}"),
        }
    }
}

impl std::error::Error for PrReadinessError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_open_auto_merge_pr_without_conflicts() {
        let ready = ensure_pr_ready(
            r#"[{"number":7,"url":"https://example.test/pr/7","state":"OPEN","headRefOid":"abc","headRefName":"zvorygin/x","autoMergeRequest":{"enabledAt":"now"},"mergeStateStatus":"CLEAN","isDraft":false}]"#,
            "zvorygin/x",
        )
        .unwrap();
        assert_eq!(ready.number, 7);
    }

    #[test]
    fn rejects_missing_pr_non_open_missing_auto_merge_and_dirty() {
        assert!(matches!(
            ensure_pr_ready("[]", "zvorygin/x"),
            Err(PrReadinessError::MissingPr { .. })
        ));
        assert!(matches!(
            ensure_pr_ready(
                r#"[{"number":1,"url":"u","state":"CLOSED","autoMergeRequest":{},"mergeStateStatus":"CLEAN"}]"#,
                "zvorygin/x"
            ),
            Err(PrReadinessError::NotOpen { .. })
        ));
        assert!(matches!(
            ensure_pr_ready(
                r#"[{"number":1,"url":"u","state":"OPEN","autoMergeRequest":null,"mergeStateStatus":"CLEAN"}]"#,
                "zvorygin/x"
            ),
            Err(PrReadinessError::MissingAutoMerge { .. })
        ));
        assert!(matches!(
            ensure_pr_ready(
                r#"[{"number":1,"url":"u","state":"OPEN","autoMergeRequest":{},"mergeStateStatus":"DIRTY"}]"#,
                "zvorygin/x"
            ),
            Err(PrReadinessError::Dirty { .. })
        ));
    }
}
