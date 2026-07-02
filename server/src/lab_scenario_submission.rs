//! Server-side draft PR creation for validated lab checkpoint setups.
//!
//! This module owns the privileged GitHub boundary for setup authoring. Browser clients can
//! request submission only through a lab room; the room exports the authoritative `Game` state and
//! hands a validated preview here. The service is disabled unless explicitly configured with
//! server-side credentials.

use std::collections::HashSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Output, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::process::Command;

use crate::lab_scenarios::{
    load_lab_scenario_catalog, LabScenarioAuthoringPreview, LabScenarioCatalogEntry,
};

pub const SCENARIO_SUBMISSION_CAPABILITY_PATH: &str = "/api/lab-scenarios/submission";
pub const LAB_SCENARIO_SUBMISSION_MANIFEST_PATH: &str = "server/assets/lab-scenarios/manifest.json";
pub const LAB_SCENARIO_SUBMISSION_PATH_PREFIX: &str = "server/assets/lab-scenarios/";

const ENABLED_ENV: &str = "RTS_SCENARIO_PR_ENABLED";
const TOKEN_ENV: &str = "RTS_SCENARIO_PR_GITHUB_TOKEN";
const REPO_ENV: &str = "RTS_SCENARIO_PR_REPO";
const BASE_BRANCH_ENV: &str = "RTS_SCENARIO_PR_BASE_BRANCH";
const BRANCH_PREFIX_ENV: &str = "RTS_SCENARIO_PR_BRANCH_PREFIX";
const DEFAULT_BASE_BRANCH: &str = "main";
const DEFAULT_BRANCH_PREFIX: &str = "zvorygin/lab-scenario-";
const MAX_BRANCH_NAME_LEN: usize = 128;
const MAX_BRANCH_PREFIX_LEN: usize = 80;
const MAX_REPO_SEGMENT_LEN: usize = 100;
const MAX_SCENARIO_FILENAME_LEN: usize = 80;

pub(crate) type ScenarioPrFuture = Pin<
    Box<
        dyn Future<Output = Result<LabScenarioSubmissionSuccess, LabScenarioSubmissionError>>
            + Send,
    >,
>;

pub(crate) trait ScenarioPrBackend: Send + Sync {
    fn create_draft_pr(&self, request: LabScenarioPrRequest) -> ScenarioPrFuture;
}

#[derive(Clone)]
struct GhCliScenarioPrBackend;

impl ScenarioPrBackend for GhCliScenarioPrBackend {
    fn create_draft_pr(&self, request: LabScenarioPrRequest) -> ScenarioPrFuture {
        Box::pin(async move { create_draft_pr_with_git_and_gh(request).await })
    }
}

#[derive(Clone)]
pub struct LabScenarioSubmissionService {
    state: Arc<LabScenarioSubmissionState>,
}

enum LabScenarioSubmissionState {
    Disabled {
        code: LabScenarioSubmissionErrorCode,
        message: String,
    },
    Enabled {
        config: LabScenarioSubmissionConfig,
        backend: Arc<dyn ScenarioPrBackend>,
    },
}

#[derive(Clone)]
struct LabScenarioSubmissionConfig {
    token: String,
    repo: String,
    base_branch: String,
    branch_prefix: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioSubmissionCapability {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub branch_prefix: String,
    pub scenario_path_prefix: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioSubmissionSuccess {
    pub pr_url: String,
    pub branch_name: String,
    pub scenario_path: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabScenarioSubmissionErrorCode {
    CredentialsMissing,
    ConfigurationError,
    ValidationFailure,
    DuplicateSlug,
    BranchCollision,
    GithubApiError,
    RateLimit,
    IoError,
}

impl LabScenarioSubmissionErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CredentialsMissing => "credentialsMissing",
            Self::ConfigurationError => "configurationError",
            Self::ValidationFailure => "validationFailure",
            Self::DuplicateSlug => "duplicateSlug",
            Self::BranchCollision => "branchCollision",
            Self::GithubApiError => "githubApiError",
            Self::RateLimit => "rateLimit",
            Self::IoError => "ioError",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabScenarioSubmissionError {
    pub code: LabScenarioSubmissionErrorCode,
    pub message: String,
}

impl LabScenarioSubmissionError {
    pub fn new(code: LabScenarioSubmissionErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn credentials_missing(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::CredentialsMissing, message)
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::ConfigurationError, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::ValidationFailure, message)
    }

    pub fn duplicate_slug(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::DuplicateSlug, message)
    }

    pub fn branch_collision(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::BranchCollision, message)
    }

    pub fn github(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::GithubApiError, message)
    }

    pub fn rate_limit(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::RateLimit, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(LabScenarioSubmissionErrorCode::IoError, message)
    }
}

#[derive(Clone)]
pub(crate) struct LabScenarioPrRequest {
    pub repo: String,
    pub base_branch: String,
    pub branch_name: String,
    pub title: String,
    pub body: String,
    pub commit_subject: String,
    pub commit_body: String,
    pub files: Vec<LabScenarioPrFile>,
    token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LabScenarioPrFile {
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LabScenarioSubmissionArtifacts {
    pub scenario_path: String,
    pub manifest_path: String,
    pub scenario_json: String,
    pub manifest_json: String,
}

#[derive(Serialize)]
struct LabScenarioManifestWrite {
    scenarios: Vec<LabScenarioCatalogEntry>,
}

impl Default for LabScenarioSubmissionService {
    fn default() -> Self {
        Self::disabled()
    }
}

impl LabScenarioSubmissionService {
    pub fn disabled() -> Self {
        Self::disabled_with(
            LabScenarioSubmissionErrorCode::CredentialsMissing,
            "Setup PR submission is disabled.",
        )
    }

    pub fn from_env() -> Self {
        Self::from_env_reader(|key| std::env::var(key).ok())
    }

    fn from_env_reader(mut get_env: impl FnMut(&str) -> Option<String>) -> Self {
        if !env_truthy(get_env(ENABLED_ENV).as_deref()) {
            return Self::disabled();
        }

        let Some(token) = nonempty_env_value(get_env(TOKEN_ENV)) else {
            return Self::disabled_with(
                LabScenarioSubmissionErrorCode::CredentialsMissing,
                format!("{TOKEN_ENV} is required when {ENABLED_ENV} is enabled."),
            );
        };
        let Some(repo) = nonempty_env_value(get_env(REPO_ENV)) else {
            return Self::disabled_with(
                LabScenarioSubmissionErrorCode::ConfigurationError,
                format!("{REPO_ENV} must name the target GitHub repository as owner/repo."),
            );
        };
        if !safe_github_repo(&repo) {
            return Self::disabled_with(
                LabScenarioSubmissionErrorCode::ConfigurationError,
                format!("{REPO_ENV} must be a safe owner/repo repository name."),
            );
        }

        let base_branch = nonempty_env_value(get_env(BASE_BRANCH_ENV))
            .unwrap_or_else(|| DEFAULT_BASE_BRANCH.to_string());
        if !safe_branch_name(&base_branch) {
            return Self::disabled_with(
                LabScenarioSubmissionErrorCode::ConfigurationError,
                format!("{BASE_BRANCH_ENV} contains unsupported branch characters."),
            );
        }

        let branch_prefix = nonempty_env_value(get_env(BRANCH_PREFIX_ENV))
            .unwrap_or_else(|| DEFAULT_BRANCH_PREFIX.to_string());
        if !safe_branch_prefix(&branch_prefix) {
            return Self::disabled_with(
                LabScenarioSubmissionErrorCode::ConfigurationError,
                format!("{BRANCH_PREFIX_ENV} contains unsupported branch prefix characters."),
            );
        }

        Self::enabled(
            LabScenarioSubmissionConfig {
                token,
                repo,
                base_branch,
                branch_prefix,
            },
            Arc::new(GhCliScenarioPrBackend),
        )
    }

    #[cfg(test)]
    pub(crate) fn enabled_for_test<B: ScenarioPrBackend + 'static>(backend: B) -> Self {
        Self::enabled(
            LabScenarioSubmissionConfig {
                token: "test-token".to_string(),
                repo: "example/rts".to_string(),
                base_branch: DEFAULT_BASE_BRANCH.to_string(),
                branch_prefix: DEFAULT_BRANCH_PREFIX.to_string(),
            },
            Arc::new(backend),
        )
    }

    fn enabled(config: LabScenarioSubmissionConfig, backend: Arc<dyn ScenarioPrBackend>) -> Self {
        Self {
            state: Arc::new(LabScenarioSubmissionState::Enabled { config, backend }),
        }
    }

    fn disabled_with(code: LabScenarioSubmissionErrorCode, message: impl Into<String>) -> Self {
        Self {
            state: Arc::new(LabScenarioSubmissionState::Disabled {
                code,
                message: message.into(),
            }),
        }
    }

    pub fn capability(&self) -> LabScenarioSubmissionCapability {
        match self.state.as_ref() {
            LabScenarioSubmissionState::Disabled { code, message } => {
                LabScenarioSubmissionCapability {
                    available: false,
                    unavailable_code: Some(code.as_str().to_string()),
                    unavailable_reason: Some(message.clone()),
                    branch_prefix: DEFAULT_BRANCH_PREFIX.to_string(),
                    scenario_path_prefix: LAB_SCENARIO_SUBMISSION_PATH_PREFIX.to_string(),
                    manifest_path: LAB_SCENARIO_SUBMISSION_MANIFEST_PATH.to_string(),
                }
            }
            LabScenarioSubmissionState::Enabled { config, .. } => LabScenarioSubmissionCapability {
                available: true,
                unavailable_code: None,
                unavailable_reason: None,
                branch_prefix: config.branch_prefix.clone(),
                scenario_path_prefix: LAB_SCENARIO_SUBMISSION_PATH_PREFIX.to_string(),
                manifest_path: LAB_SCENARIO_SUBMISSION_MANIFEST_PATH.to_string(),
            },
        }
    }

    pub fn unavailable_error(&self) -> Option<LabScenarioSubmissionError> {
        match self.state.as_ref() {
            LabScenarioSubmissionState::Disabled { code, message } => {
                Some(LabScenarioSubmissionError::new(*code, message.clone()))
            }
            LabScenarioSubmissionState::Enabled { .. } => None,
        }
    }

    pub async fn submit_preview(
        &self,
        preview: LabScenarioAuthoringPreview,
    ) -> Result<LabScenarioSubmissionSuccess, LabScenarioSubmissionError> {
        let request = self.prepare_request(&preview)?;
        match self.state.as_ref() {
            LabScenarioSubmissionState::Disabled { code, message } => {
                Err(LabScenarioSubmissionError::new(*code, message.clone()))
            }
            LabScenarioSubmissionState::Enabled { backend, .. } => {
                backend.create_draft_pr(request).await
            }
        }
    }

    fn prepare_request(
        &self,
        preview: &LabScenarioAuthoringPreview,
    ) -> Result<LabScenarioPrRequest, LabScenarioSubmissionError> {
        let LabScenarioSubmissionState::Enabled { config, .. } = self.state.as_ref() else {
            return Err(self.unavailable_error().unwrap_or_else(|| {
                LabScenarioSubmissionError::credentials_missing(
                    "Setup PR submission is unavailable.",
                )
            }));
        };
        let artifacts = build_submission_artifacts(preview)?;
        let branch_name = scenario_branch_name(&config.branch_prefix, &preview.slug)?;
        let title = scenario_pr_title(preview);
        let body = scenario_pr_body(preview, &artifacts, config);
        let commit_subject = format!("Add lab setup {}", preview.slug);
        let scenario_title = &preview.manifest_entry.title;
        let commit_body = format!(
            "Adds {scenario_title} to the bundled lab setup catalog.\n\nSetup path: {}\nManifest path: {}",
            artifacts.scenario_path, artifacts.manifest_path
        );

        Ok(LabScenarioPrRequest {
            repo: config.repo.clone(),
            base_branch: config.base_branch.clone(),
            branch_name,
            title,
            body,
            commit_subject,
            commit_body,
            files: artifacts.files(),
            token: config.token.clone(),
        })
    }
}

impl LabScenarioSubmissionArtifacts {
    fn files(&self) -> Vec<LabScenarioPrFile> {
        vec![
            LabScenarioPrFile {
                path: self.scenario_path.clone(),
                contents: self.scenario_json.clone(),
            },
            LabScenarioPrFile {
                path: self.manifest_path.clone(),
                contents: self.manifest_json.clone(),
            },
        ]
    }
}

pub(crate) fn build_submission_artifacts(
    preview: &LabScenarioAuthoringPreview,
) -> Result<LabScenarioSubmissionArtifacts, LabScenarioSubmissionError> {
    let expected_scenario_path =
        format!("{LAB_SCENARIO_SUBMISSION_PATH_PREFIX}{}", preview.filename);
    if preview.scenario_path != expected_scenario_path {
        return Err(LabScenarioSubmissionError::validation(format!(
            "setup path must be {expected_scenario_path:?}"
        )));
    }
    if preview.manifest_path != LAB_SCENARIO_SUBMISSION_MANIFEST_PATH {
        return Err(LabScenarioSubmissionError::validation(format!(
            "manifest path must be {LAB_SCENARIO_SUBMISSION_MANIFEST_PATH:?}"
        )));
    }
    if !allowed_submission_path(&preview.scenario_path)
        || !allowed_submission_path(&preview.manifest_path)
    {
        return Err(LabScenarioSubmissionError::validation(
            "setup submission paths are outside the allowlist",
        ));
    }

    let mut entries = load_lab_scenario_catalog().map_err(|err| {
        LabScenarioSubmissionError::validation(format!(
            "cannot load existing lab setup catalog: {err}"
        ))
    })?;
    if entries
        .iter()
        .any(|entry| entry.id == preview.manifest_entry.id)
    {
        return Err(LabScenarioSubmissionError::duplicate_slug(format!(
            "duplicate lab setup id {:?}",
            preview.manifest_entry.id
        )));
    }
    if entries
        .iter()
        .any(|entry| entry.filename == preview.manifest_entry.filename)
    {
        return Err(LabScenarioSubmissionError::duplicate_slug(format!(
            "duplicate lab setup filename {:?}",
            preview.manifest_entry.filename
        )));
    }
    entries.push(preview.manifest_entry.clone());
    entries.sort_by(|a, b| a.id.cmp(&b.id));

    let manifest_json =
        serde_json::to_string_pretty(&LabScenarioManifestWrite { scenarios: entries }).map_err(
            |err| {
                LabScenarioSubmissionError::validation(format!(
                    "failed to format lab setup manifest: {err}"
                ))
            },
        )? + "\n";

    Ok(LabScenarioSubmissionArtifacts {
        scenario_path: preview.scenario_path.clone(),
        manifest_path: preview.manifest_path.clone(),
        scenario_json: ensure_trailing_newline(&preview.scenario_json),
        manifest_json,
    })
}

pub(crate) fn allowed_submission_path(path: &str) -> bool {
    if path == LAB_SCENARIO_SUBMISSION_MANIFEST_PATH {
        return true;
    }
    let Some(filename) = path.strip_prefix(LAB_SCENARIO_SUBMISSION_PATH_PREFIX) else {
        return false;
    };
    safe_scenario_submission_filename(filename)
}

fn safe_scenario_submission_filename(filename: &str) -> bool {
    !filename.is_empty()
        && filename.len() <= MAX_SCENARIO_FILENAME_LEN
        && filename.ends_with(".json")
        && filename != "manifest.json"
        && !filename.contains('/')
        && !filename.contains('\\')
        && !filename.contains("..")
        && filename
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
}

pub(crate) fn scenario_branch_name(
    prefix: &str,
    slug: &str,
) -> Result<String, LabScenarioSubmissionError> {
    if !safe_branch_prefix(prefix) {
        return Err(LabScenarioSubmissionError::configuration(
            "setup branch prefix is not safe",
        ));
    }
    let branch = format!("{prefix}{slug}");
    if branch.len() > MAX_BRANCH_NAME_LEN || !safe_branch_name(&branch) {
        return Err(LabScenarioSubmissionError::configuration(
            "setup branch name is not safe",
        ));
    }
    Ok(branch)
}

fn scenario_pr_title(preview: &LabScenarioAuthoringPreview) -> String {
    format!("Add lab setup: {}", preview.manifest_entry.title)
}

fn scenario_pr_body(
    preview: &LabScenarioAuthoringPreview,
    artifacts: &LabScenarioSubmissionArtifacts,
    config: &LabScenarioSubmissionConfig,
) -> String {
    let entity_count = scenario_entity_count(preview);
    let tags = if preview.manifest_entry.tags.is_empty() {
        "(none)".to_string()
    } else {
        preview.manifest_entry.tags.join(", ")
    };
    let notes = if preview.review_notes.trim().is_empty() {
        "(none)".to_string()
    } else {
        preview.review_notes.trim().to_string()
    };
    format!(
        "\
## Checkpoint Setup

- ID: `{}`
- Title: {}
- Description: {}
- Tags: {}
- Map: {}
- Players: {}
- Entities: {}
- Setup path: `{}`
- Manifest path: `{}`
- Base branch: `{}`

## Validation

Server validation exported the current authoritative lab game as a checkpoint-backed setup, applied authoring metadata, formatted deterministic JSON, checked catalog duplicate/path/size/entity limits, verified map binding metadata, and restored the setup through the lab `Game` API.

## Author Notes

{}

## Manual Review Checklist

- Confirm the setup name, catalog title, and description are review-ready.
- Confirm the map is the intended bundled map.
- Confirm player/faction setup, teams, resources, and completed research are correct.
- Confirm entity count ({}) and entity placement match the author intent.
- Confirm the intended use is clear from the description or author notes.
- Confirm the setup loads from the lab catalog after merge.
- Run manual lab smoke from `/lab`: launch the setup, inspect it, validate controls, and export setup JSON if needed.
- Let normal CI and human setup review pass before merge.
",
        preview.slug,
        preview.manifest_entry.title,
        preview.manifest_entry.description,
        tags,
        preview.manifest_entry.map,
        preview.manifest_entry.player_count,
        entity_count,
        artifacts.scenario_path,
        artifacts.manifest_path,
        config.base_branch,
        notes,
        entity_count
    )
}

fn scenario_entity_count(preview: &LabScenarioAuthoringPreview) -> String {
    serde_json::from_str::<serde_json::Value>(&preview.scenario_json)
        .ok()
        .and_then(|value| {
            if let Some(count) = value
                .get("entities")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
            {
                return Some(count);
            }
            value
                .get("checkpointPayload")
                .and_then(serde_json::Value::as_str)
                .and_then(|payload| serde_json::from_str::<serde_json::Value>(payload).ok())
                .and_then(|payload| {
                    payload
                        .get("entities")
                        .and_then(|entities| entities.get("entities"))
                        .and_then(serde_json::Value::as_array)
                        .map(Vec::len)
                })
        })
        .map(|count| count.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

async fn create_draft_pr_with_git_and_gh(
    request: LabScenarioPrRequest,
) -> Result<LabScenarioSubmissionSuccess, LabScenarioSubmissionError> {
    validate_pr_request_files(&request.files)?;

    let repo_url = format!("https://github.com/{}.git", request.repo);
    if remote_branch_exists(&repo_url, &request.branch_name, &request.token).await? {
        return Err(LabScenarioSubmissionError::branch_collision(format!(
            "branch {:?} already exists",
            request.branch_name
        )));
    }

    let temp_repo = TempScenarioRepo::new()?;
    let workdir = temp_repo.path().to_path_buf();
    let mut clone_cmd = git_command(&request.token);
    clone_cmd
        .arg("clone")
        .arg("--depth=1")
        .arg("--branch")
        .arg(&request.base_branch)
        .arg(&repo_url)
        .arg(&workdir);
    run_git_command(&mut clone_cmd, "clone setup PR repository").await?;

    let mut checkout_cmd = git_command(&request.token);
    checkout_cmd
        .arg("-C")
        .arg(&workdir)
        .arg("checkout")
        .arg("-b")
        .arg(&request.branch_name);
    run_git_command(&mut checkout_cmd, "create setup PR branch").await?;

    for file in &request.files {
        let path = workdir.join(&file.path);
        let Some(parent) = path.parent() else {
            return Err(LabScenarioSubmissionError::validation(format!(
                "path {:?} has no parent directory",
                file.path
            )));
        };
        std::fs::create_dir_all(parent).map_err(|err| {
            LabScenarioSubmissionError::io(format!(
                "failed to create parent directory for {:?}: {err}",
                file.path
            ))
        })?;
        std::fs::write(&path, file.contents.as_bytes()).map_err(|err| {
            LabScenarioSubmissionError::io(format!("failed to write {:?}: {err}", file.path))
        })?;
    }

    let mut add = git_command(&request.token);
    add.arg("-C").arg(&workdir).arg("add").arg("--");
    for file in &request.files {
        add.arg(&file.path);
    }
    run_git_command(&mut add, "stage setup PR files").await?;

    let mut author_name_cmd = git_plain_command();
    author_name_cmd
        .arg("-C")
        .arg(&workdir)
        .arg("config")
        .arg("user.name")
        .arg("Lab Setup Bot");
    run_git_command(
        &mut author_name_cmd,
        "configure setup PR git author name",
    )
    .await?;

    let mut author_email_cmd = git_plain_command();
    author_email_cmd
        .arg("-C")
        .arg(&workdir)
        .arg("config")
        .arg("user.email")
        .arg("lab-setup-bot@users.noreply.github.com");
    run_git_command(
        &mut author_email_cmd,
        "configure setup PR git author email",
    )
    .await?;

    let mut commit_cmd = git_plain_command();
    commit_cmd
        .arg("-C")
        .arg(&workdir)
        .arg("commit")
        .arg("-m")
        .arg(&request.commit_subject)
        .arg("-m")
        .arg(&request.commit_body);
    run_git_command(&mut commit_cmd, "commit setup PR files").await?;

    let mut push_cmd = git_command(&request.token);
    push_cmd
        .arg("-C")
        .arg(&workdir)
        .arg("push")
        .arg("origin")
        .arg(format!("HEAD:refs/heads/{}", request.branch_name));
    run_git_command(&mut push_cmd, "push setup PR branch").await?;

    let mut pr_cmd = gh_command(&request.token);
    pr_cmd
        .arg("pr")
        .arg("create")
        .arg("--repo")
        .arg(&request.repo)
        .arg("--base")
        .arg(&request.base_branch)
        .arg("--head")
        .arg(&request.branch_name)
        .arg("--title")
        .arg(&request.title)
        .arg("--body")
        .arg(&request.body)
        .arg("--draft");
    let pr_url = run_gh_command(&mut pr_cmd, "create draft setup PR").await?;

    let scenario_path = request
        .files
        .iter()
        .find(|file| file.path != LAB_SCENARIO_SUBMISSION_MANIFEST_PATH)
        .map(|file| file.path.clone())
        .unwrap_or_default();

    Ok(LabScenarioSubmissionSuccess {
        pr_url: pr_url.trim().to_string(),
        branch_name: request.branch_name,
        scenario_path,
        manifest_path: LAB_SCENARIO_SUBMISSION_MANIFEST_PATH.to_string(),
    })
}

fn validate_pr_request_files(
    files: &[LabScenarioPrFile],
) -> Result<(), LabScenarioSubmissionError> {
    if files.len() != 2 {
        return Err(LabScenarioSubmissionError::validation(
            "setup PR requests must write exactly one setup file and the manifest",
        ));
    }

    let mut seen = HashSet::new();
    let mut scenario_files = 0;
    let mut manifest_files = 0;
    for file in files {
        if !allowed_submission_path(&file.path) {
            return Err(LabScenarioSubmissionError::validation(format!(
                "refusing to write non-allowlisted path {:?}",
                file.path
            )));
        }
        if !seen.insert(file.path.as_str()) {
            return Err(LabScenarioSubmissionError::validation(format!(
                "duplicate setup PR file path {:?}",
                file.path
            )));
        }
        if file.path == LAB_SCENARIO_SUBMISSION_MANIFEST_PATH {
            manifest_files += 1;
        } else {
            scenario_files += 1;
        }
    }

    if scenario_files != 1 || manifest_files != 1 {
        return Err(LabScenarioSubmissionError::validation(
            "setup PR requests must include one setup JSON file and one manifest file",
        ));
    }
    Ok(())
}

async fn remote_branch_exists(
    repo_url: &str,
    branch_name: &str,
    token: &str,
) -> Result<bool, LabScenarioSubmissionError> {
    let mut command = git_command(token);
    command
        .arg("ls-remote")
        .arg("--exit-code")
        .arg("--heads")
        .arg(repo_url)
        .arg(branch_name);
    let output = command_output(&mut command, "check setup PR branch collision").await?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(2) {
        return Ok(false);
    }
    Err(LabScenarioSubmissionError::github(format!(
        "failed to check branch collision: {}",
        command_error_text(&output)
    )))
}

fn git_plain_command() -> Command {
    let mut command = Command::new("git");
    command.stdin(Stdio::null());
    command
}

fn git_command(token: &str) -> Command {
    let mut command = git_plain_command();
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "http.https://github.com/.extraheader")
        .env(
            "GIT_CONFIG_VALUE_0",
            format!("AUTHORIZATION: bearer {token}"),
        );
    command
}

fn gh_command(token: &str) -> Command {
    let mut command = Command::new("gh");
    command
        .stdin(Stdio::null())
        .env("GH_TOKEN", token)
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}

async fn run_git_command(
    command: &mut Command,
    context: &str,
) -> Result<String, LabScenarioSubmissionError> {
    let output = command_output(command, context).await?;
    if !output.status.success() {
        return Err(LabScenarioSubmissionError::github(format!(
            "{context} failed: {}",
            command_error_text(&output)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn run_gh_command(
    command: &mut Command,
    context: &str,
) -> Result<String, LabScenarioSubmissionError> {
    let output = command_output(command, context).await?;
    if !output.status.success() {
        return Err(LabScenarioSubmissionError::github(format!(
            "{context} failed: {}",
            command_error_text(&output)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn command_output(
    command: &mut Command,
    context: &str,
) -> Result<Output, LabScenarioSubmissionError> {
    command
        .output()
        .await
        .map_err(|err| LabScenarioSubmissionError::io(format!("{context} failed to start: {err}")))
}

fn command_error_text(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = stderr.trim();
    let stdout = stdout.trim();
    if !stderr.is_empty() {
        stderr.to_string()
    } else if !stdout.is_empty() {
        stdout.to_string()
    } else {
        format!("process exited with status {}", output.status)
    }
}

struct TempScenarioRepo {
    path: PathBuf,
}

impl TempScenarioRepo {
    fn new() -> Result<Self, LabScenarioSubmissionError> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "rts-lab-setup-pr-{}-{unique}",
            std::process::id()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).map_err(|err| {
                LabScenarioSubmissionError::io(format!(
                    "failed to clear existing temp setup PR directory: {err}"
                ))
            })?;
        }
        std::fs::create_dir_all(&path).map_err(|err| {
            LabScenarioSubmissionError::io(format!(
                "failed to create temp setup PR directory: {err}"
            ))
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempScenarioRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

fn nonempty_env_value(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_truthy(value: Option<&str>) -> bool {
    value
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "" | "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(false)
}

fn safe_github_repo(value: &str) -> bool {
    let mut parts = value.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(repo) = parts.next() else {
        return false;
    };
    parts.next().is_none() && safe_repo_segment(owner) && safe_repo_segment(repo)
}

fn safe_repo_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REPO_SEGMENT_LEN
        && !value.contains("..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
}

fn safe_branch_prefix(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_BRANCH_PREFIX_LEN
        && !value.starts_with('/')
        && !value.contains("..")
        && !value.contains("//")
        && value.bytes().all(safe_branch_byte)
}

fn safe_branch_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_BRANCH_NAME_LEN
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.ends_with('.')
        && !value.contains("..")
        && !value.contains("//")
        && value.bytes().all(safe_branch_byte)
}

fn safe_branch_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab_scenarios::{load_lab_scenario_by_id, validate_lab_scenario_authoring};
    use crate::protocol::LabScenarioAuthoringMetadata;
    use std::sync::Mutex as StdMutex;

    #[derive(Clone)]
    struct RecordingBackend {
        captured: Arc<StdMutex<Vec<LabScenarioPrRequest>>>,
        result: Result<LabScenarioSubmissionSuccess, LabScenarioSubmissionError>,
    }

    impl ScenarioPrBackend for RecordingBackend {
        fn create_draft_pr(&self, request: LabScenarioPrRequest) -> ScenarioPrFuture {
            let captured = self.captured.clone();
            let result = self.result.clone();
            Box::pin(async move {
                captured.lock().unwrap().push(request);
                result
            })
        }
    }

    fn preview_for(slug: &str) -> LabScenarioAuthoringPreview {
        let loaded =
            load_lab_scenario_by_id("lategame").expect("bundled lategame setup should load");
        validate_lab_scenario_authoring(
            LabScenarioAuthoringMetadata {
                slug: slug.to_string(),
                name: "Submission Test".to_string(),
                title: "Submission Test".to_string(),
                description: "A setup used to test submission plumbing.".to_string(),
                tags: vec!["test".to_string()],
                review_notes: Some("Check the generated PR body.".to_string()),
            },
            loaded.scenario,
        )
        .expect("setup authoring metadata should validate")
    }

    #[test]
    fn service_is_disabled_by_default_and_requires_credentials() {
        let disabled = LabScenarioSubmissionService::from_env_reader(|_| None);
        let capability = disabled.capability();
        assert!(!capability.available);
        assert_eq!(
            capability.unavailable_code.as_deref(),
            Some("credentialsMissing")
        );

        let enabled_without_token =
            LabScenarioSubmissionService::from_env_reader(|key| match key {
                ENABLED_ENV => Some("1".to_string()),
                REPO_ENV => Some("example/rts".to_string()),
                _ => None,
            });
        let capability = enabled_without_token.capability();
        assert!(!capability.available);
        assert_eq!(
            capability.unavailable_code.as_deref(),
            Some("credentialsMissing")
        );
    }

    #[test]
    fn submission_artifacts_write_only_allowlisted_paths() {
        let preview = preview_for("submission-artifact-test");
        let artifacts = build_submission_artifacts(&preview).expect("artifacts should build");

        assert_eq!(
            artifacts.scenario_path,
            "server/assets/lab-scenarios/submission-artifact-test.json"
        );
        assert_eq!(
            artifacts.manifest_path,
            LAB_SCENARIO_SUBMISSION_MANIFEST_PATH
        );
        assert!(artifacts.scenario_json.ends_with('\n'));
        assert!(artifacts
            .manifest_json
            .contains("\"id\": \"submission-artifact-test\""));
        assert!(artifacts
            .files()
            .iter()
            .all(|file| allowed_submission_path(&file.path)));
        assert!(!allowed_submission_path(
            "server/assets/lab-scenarios/../bad.json"
        ));
        assert!(!allowed_submission_path("client/src/main.js"));
    }

    #[test]
    fn pr_request_files_must_be_exact_scenario_and_manifest_pair() {
        let preview = preview_for("file-pair-test");
        let artifacts = build_submission_artifacts(&preview).expect("artifacts should build");
        let files = artifacts.files();
        validate_pr_request_files(&files).expect("valid setup plus manifest pair should pass");

        let mut duplicate = files.clone();
        duplicate[1] = duplicate[0].clone();
        let err = validate_pr_request_files(&duplicate).expect_err("duplicate path should reject");
        assert_eq!(err.code, LabScenarioSubmissionErrorCode::ValidationFailure);

        let err = validate_pr_request_files(&[files[0].clone()])
            .expect_err("missing manifest should reject");
        assert_eq!(err.code, LabScenarioSubmissionErrorCode::ValidationFailure);

        let err = validate_pr_request_files(&[
            files[0].clone(),
            LabScenarioPrFile {
                path: "client/src/main.js".to_string(),
                contents: String::new(),
            },
        ])
        .expect_err("non-allowlisted path should reject");
        assert_eq!(err.code, LabScenarioSubmissionErrorCode::ValidationFailure);
    }

    #[test]
    fn submission_artifacts_reject_tampered_preview_paths() {
        let mut preview = preview_for("tampered-path-test");
        preview.scenario_path =
            "server/assets/lab-scenarios/../tampered-path-test.json".to_string();

        let err = build_submission_artifacts(&preview).expect_err("tampered path should reject");
        assert_eq!(err.code, LabScenarioSubmissionErrorCode::ValidationFailure);
    }

    #[test]
    fn submission_artifacts_recheck_duplicate_catalog_slug() {
        let mut preview = preview_for("duplicate-race-test");
        preview.manifest_entry.id = "lategame".to_string();

        let err = build_submission_artifacts(&preview).expect_err("duplicate id should reject");
        assert_eq!(err.code, LabScenarioSubmissionErrorCode::DuplicateSlug);
    }

    #[test]
    fn branch_names_are_derived_from_safe_prefix_and_slug() {
        assert_eq!(
            scenario_branch_name("scenario/", "safe_slug-1").unwrap(),
            "scenario/safe_slug-1"
        );
        assert!(scenario_branch_name("../bad/", "safe").is_err());
        assert!(scenario_branch_name("scenario/", "../bad").is_err());
    }

    #[test]
    fn pr_body_contains_metadata_notes_and_manual_review_checklist() {
        let preview = preview_for("body-test");
        let artifacts = build_submission_artifacts(&preview).expect("artifacts should build");
        let body = scenario_pr_body(
            &preview,
            &artifacts,
            &LabScenarioSubmissionConfig {
                token: "test".to_string(),
                repo: "example/rts".to_string(),
                base_branch: "main".to_string(),
                branch_prefix: "scenario/".to_string(),
            },
        );

        assert!(body.contains("ID: `body-test`"));
        assert!(body.contains("Entities: 227"));
        assert!(body.contains("Check the generated PR body."));
        assert!(body.contains("Manual Review Checklist"));
        assert!(body.contains("setup name"));
        assert!(body.contains("map is the intended bundled map"));
        assert!(body.contains("player/faction setup"));
        assert!(body.contains("entity count (227)"));
        assert!(body.contains("intended use"));
        assert!(body.contains("manual lab smoke"));
        assert!(body.contains("server/assets/lab-scenarios/body-test.json"));
    }

    #[tokio::test]
    async fn submit_preview_uses_mocked_backend_with_deterministic_request() {
        let captured = Arc::new(StdMutex::new(Vec::new()));
        let service = LabScenarioSubmissionService::enabled_for_test(RecordingBackend {
            captured: captured.clone(),
            result: Ok(LabScenarioSubmissionSuccess {
                pr_url: "https://github.com/example/rts/pull/7".to_string(),
                branch_name: "zvorygin/lab-scenario-submit-test".to_string(),
                scenario_path: "server/assets/lab-scenarios/submit-test.json".to_string(),
                manifest_path: LAB_SCENARIO_SUBMISSION_MANIFEST_PATH.to_string(),
            }),
        });

        let success = service
            .submit_preview(preview_for("submit-test"))
            .await
            .expect("mocked submission should succeed");
        assert_eq!(success.pr_url, "https://github.com/example/rts/pull/7");

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 1);
        let request = &captured[0];
        assert_eq!(request.branch_name, "zvorygin/lab-scenario-submit-test");
        assert_eq!(request.title, "Add lab setup: Submission Test");
        assert_eq!(request.files.len(), 2);
        assert!(request
            .files
            .iter()
            .any(|file| file.path == "server/assets/lab-scenarios/submit-test.json"));
        assert!(request.body.contains("Server validation exported"));
    }

    #[tokio::test]
    async fn submit_preview_surfaces_mocked_github_errors() {
        let service = LabScenarioSubmissionService::enabled_for_test(RecordingBackend {
            captured: Arc::new(StdMutex::new(Vec::new())),
            result: Err(LabScenarioSubmissionError::github(
                "GitHub API rejected the PR",
            )),
        });

        let err = service
            .submit_preview(preview_for("github-error-test"))
            .await
            .expect_err("mocked GitHub failure should return an error");
        assert_eq!(err.code, LabScenarioSubmissionErrorCode::GithubApiError);
        assert!(err.message.contains("GitHub API rejected"));
    }
}
