use crate::phase::{discover_phases, PhaseDiscoveryError, PhaseId, PhaseIdParseError};
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerConfig {
    pub plan: PlanRef,
    pub base_branch: String,
    pub worktree_root: PathBuf,
    pub run_mode: RunMode,
    pub pr_lifecycle: PrLifecycle,
    pub model: Option<String>,
    pub phases: Vec<PhaseId>,
    pub phase_selection: PhaseSelection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanRef {
    pub name: String,
    pub plan_file: PathBuf,
    pub plan_dir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunMode {
    DryRun,
    Execute,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrLifecycle {
    OpenAndStop,
    OpenAndWait,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhaseSelection {
    Explicit,
    DiscoveredRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhaseRun {
    pub plan_name: String,
    pub phase: PhaseId,
    pub branch: String,
    pub layout: WorktreeLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeLayout {
    pub worktree_path: PathBuf,
    pub log_dir: PathBuf,
    pub handoff_file: PathBuf,
    pub pr_body_file: PathBuf,
    pub codex_log: PathBuf,
    pub timing_file: PathBuf,
    pub active_marker_dir: PathBuf,
    pub active_marker: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DryRunPlan {
    pub config: RunnerConfig,
    pub runs: Vec<PhaseRun>,
    pub base_commit: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CliOptions {
    plan_name: String,
    base_branch: String,
    model: Option<String>,
    explicit_phases: Vec<String>,
    from_phase: Option<String>,
    to_phase: Option<String>,
    dry_run: bool,
    pr_mode: bool,
    wait_for_pr: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliAction {
    Help,
    DryRun(DryRunPlan),
}

#[derive(Debug)]
pub enum CliError {
    MissingValue { option: String },
    UnknownOption(String),
    MissingPlan,
    PrRequired,
    WaitRequiresPr,
    RangePairRequired,
    MixedExplicitAndRange,
    InvalidPlanName(String),
    InvalidBase(String),
    MissingPlanFile(PathBuf),
    MissingSchemaFile(PathBuf),
    InvalidPhase(PhaseIdParseError),
    MissingPhaseFile(PathBuf),
    PhaseDiscovery(PhaseDiscoveryError),
    ExecuteUnsupported,
    CurrentDir(String),
    Git(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptSection {
    name: &'static str,
    body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutorPrompt {
    sections: Vec<PromptSection>,
}

impl PhaseRun {
    pub fn new(plan_name: impl Into<String>, phase: PhaseId, worktree_root: PathBuf) -> Self {
        let plan_name = plan_name.into();
        let phase_label = phase.to_string();
        let branch = format!("zvorygin/{plan_name}-{phase_label}");
        let worktree_path = worktree_root.join(format!("{plan_name}-{phase_label}"));
        let log_dir = worktree_root.join("phase-runner-logs").join(&plan_name);
        let handoff_dir = log_dir.join("handoffs");
        let active_marker_dir = worktree_root.join("phase-runner-active");
        let active_marker = active_marker_dir.join(branch.replace('/', "__"));

        Self {
            plan_name,
            phase,
            branch,
            layout: WorktreeLayout {
                worktree_path,
                handoff_file: handoff_dir.join(format!("{phase_label}.json")),
                pr_body_file: log_dir.join(format!("{phase_label}.pr-body.md")),
                codex_log: log_dir.join(format!("{phase_label}.codex.log")),
                timing_file: log_dir.join(format!("{phase_label}.timing.json")),
                log_dir,
                active_marker_dir,
                active_marker,
            },
        }
    }
}

impl RunnerConfig {
    pub fn phase_runs(&self) -> Vec<PhaseRun> {
        self.phases
            .iter()
            .cloned()
            .map(|phase| PhaseRun::new(&self.plan.name, phase, self.worktree_root.clone()))
            .collect()
    }
}

impl PlanRef {
    pub fn new(repo_root: &Path, name: impl Into<String>) -> Self {
        let name = name.into();
        let plan_dir = repo_root.join("plans").join(&name);
        let plan_file = plan_dir.join("plan.md");
        Self {
            name,
            plan_file,
            plan_dir,
        }
    }
}

impl ExecutorPrompt {
    pub fn for_phase(plan_name: &str, phase: &PhaseId, branch: &str) -> Self {
        let phase_path = format!("plans/{plan_name}/{phase}.md");
        let sections = vec![
            PromptSection {
                name: "skill",
                body: "$phase-runner".to_string(),
            },
            PromptSection {
                name: "objective",
                body: "Execute exactly one planned phase in this RTS repository.".to_string(),
            },
            PromptSection {
                name: "context",
                body: format!(
                    "Plan: plans/{plan_name}/plan.md\nPhase: {phase_path}\nCurrent branch: {branch}"
                ),
            },
            PromptSection {
                name: "executor_rules",
                body: format!(
                    concat!(
                        "This is an executor pass only:\n",
                        "- You are already running inside the assigned clean worktree for this phase. This satisfies the\n",
                        "  repository worktree requirement; do not create another worktree or switch to another checkout.\n",
                        "- Do not create or revise the overall plan.\n",
                        "- Do not run a final review pass.\n",
                        "- Do not merge, push, or open a PR; the outer phase runner handles branch push and PR automation after you commit.\n",
                        "- Implement only this phase.\n",
                        "- Stage and commit only files belonging to this phase.\n",
                        "- The phase is not completed until your task changes are committed successfully on {branch}.\n",
                        "- Mark {phase_path} done if and only if the phase is committed successfully.\n",
                        "- Run the smallest targeted verification appropriate for the changed files.\n",
                        "- Commit with the normal git commit hook. Do not run the broad full local gate unless the phase\n",
                        "  explicitly requires it; GitHub Actions is the authoritative full gate after the PR opens.\n",
                        "- If the commit hook fails, do not return completed. Inspect the failure, keep working, run focused\n",
                        "  checks, and retry the commit until it succeeds.\n",
                        "- You may commit with --no-verify only for pure documentation changes or when you have conclusively\n",
                        "  confirmed the only failing hook check is unrelated to this phase. Document that evidence in the\n",
                        "  JSON handoff verification or notes.\n",
                        "- Avoid broad formatting commands such as workspace-wide cargo fmt unless they are required for the\n",
                        "  phase diff. If formatting is needed, keep any formatter drift outside the phase scope out of the\n",
                        "  final diff.\n",
                        "- Prefer plain filesystem renames/moves over git mv inside this sandboxed executor session.\n",
                        "- If the phase is ambiguous, too broad, blocked by failing verification or commit-hook failure you\n",
                        "  cannot repair, or needs human design/product input, stop and report status \"blocked\".\n",
                        "- Include focused verification, next-step notes, and manual-test notes detailed enough for the\n",
                        "  outer phase runner to write an owned PR body."
                    ),
                    branch = branch,
                    phase_path = phase_path
                ),
            },
            PromptSection {
                name: "handoff",
                body: "Return a compact JSON handoff matching the requested schema.".to_string(),
            },
        ];
        Self { sections }
    }

    pub fn render(&self) -> String {
        self.sections
            .iter()
            .map(|section| section.body.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

pub fn usage() -> &'static str {
    "\
Usage:
  rts-phaserunner --plan NAME PHASE [PHASE ...] [options]

Examples:
  rts-phaserunner --plan faction 4 --pr
  rts-phaserunner --plan faction 5.5 --pr
  rts-phaserunner --plan faction phase-4 phase-5 --pr --wait
  rts-phaserunner --plan faction --from 5 --to 6 --pr --wait
  rts-phaserunner --plan ai 2 --model gpt-5.4-mini --pr

Options:
  --plan NAME       Plan directory name under plans/. Required.
  --base BRANCH     Must be main. Kept for compatibility with existing calls.
  --model MODEL     Optional Codex model override for executor passes.
  --from PHASE      Discover phases after PHASE, up to --to. Example: --from 5.
  --to PHASE        Discover phases through PHASE. Requires --from.
  --pr              Push the phase branch, open/update an owned PR, arm auto-merge, and stop pending merge.
  --wait            With --pr, wait for each phase PR to merge before reporting success or continuing.
  --dry-run         Print worktrees, branches, and prompts without running Codex.
  -h, --help        Show this help.
"
}

pub fn plan_from_args<I, S>(args: I, repo_root: &Path) -> Result<CliAction, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(CliAction::Help);
    }

    let options = parse_options(args)?;
    validate_plan_name(&options.plan_name)?;

    if !options.pr_mode {
        return Err(CliError::PrRequired);
    }
    if options.wait_for_pr && !options.pr_mode {
        return Err(CliError::WaitRequiresPr);
    }
    if (options.from_phase.is_some() && options.to_phase.is_none())
        || (options.from_phase.is_none() && options.to_phase.is_some())
    {
        return Err(CliError::RangePairRequired);
    }
    if options.from_phase.is_some() && !options.explicit_phases.is_empty() {
        return Err(CliError::MixedExplicitAndRange);
    }
    if options.base_branch != "main" {
        return Err(CliError::InvalidBase(options.base_branch));
    }

    let plan = PlanRef::new(repo_root, options.plan_name);
    if !plan.plan_file.is_file() {
        return Err(CliError::MissingPlanFile(plan.plan_file));
    }
    let schema_file = repo_root.join("scripts/phase-runner-result.schema.json");
    if !schema_file.is_file() {
        return Err(CliError::MissingSchemaFile(schema_file));
    }

    let discovered_range = options.from_phase.is_some();
    let phases = if let (Some(from), Some(to)) = (&options.from_phase, &options.to_phase) {
        discover_phases(&plan.plan_dir, from, to).map_err(CliError::PhaseDiscovery)?
    } else {
        options
            .explicit_phases
            .iter()
            .map(|raw| PhaseId::parse(raw).map_err(CliError::InvalidPhase))
            .collect::<Result<Vec<_>, _>>()?
    };
    if phases.is_empty() {
        return Err(CliError::MissingPlan);
    }

    for phase in &phases {
        let phase_file = plan.plan_dir.join(phase.file_name());
        if !phase_file.is_file() {
            return Err(CliError::MissingPhaseFile(phase_file));
        }
    }

    let worktree_root = env::var_os("RTS_WORKTREE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/rts-worktrees"));
    let config = RunnerConfig {
        plan,
        base_branch: options.base_branch,
        worktree_root,
        run_mode: if options.dry_run {
            RunMode::DryRun
        } else {
            RunMode::Execute
        },
        pr_lifecycle: if options.wait_for_pr {
            PrLifecycle::OpenAndWait
        } else {
            PrLifecycle::OpenAndStop
        },
        model: options.model,
        phases,
        phase_selection: if discovered_range {
            PhaseSelection::DiscoveredRange
        } else {
            PhaseSelection::Explicit
        },
    };

    if config.run_mode != RunMode::DryRun {
        return Err(CliError::ExecuteUnsupported);
    }

    Ok(CliAction::DryRun(DryRunPlan {
        runs: config.phase_runs(),
        config,
        base_commit: None,
    }))
}

pub fn plan_from_env() -> Result<CliAction, CliError> {
    let repo_root = env::current_dir().map_err(|err| CliError::CurrentDir(err.to_string()))?;
    let mut action = plan_from_args(env::args().skip(1), &repo_root)?;
    if let CliAction::DryRun(plan) = &mut action {
        plan.base_commit = Some(git_rev_parse(&repo_root, &plan.config.base_branch)?);
    }
    Ok(action)
}

pub fn render_dry_run(plan: &DryRunPlan) -> String {
    let mut output = String::new();
    let base_commit = plan.base_commit.as_deref().unwrap_or("<dry-run-base>");
    if plan.config.phase_selection == PhaseSelection::DiscoveredRange {
        output.push_str(&format!(
            "phase-runner: discovered phases: {}\n",
            plan.config
                .phases
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    for run in &plan.runs {
        output.push_str(&format!(
            "phase-runner: creating {} from {} ({}) on {}\n",
            run.layout.worktree_path.display(),
            plan.config.base_branch,
            base_commit,
            run.branch
        ));
        output.push_str(&format!(
            "phase-runner: would run Codex in {}\n",
            run.layout.worktree_path.display()
        ));
        output.push_str(&format!(
            "phase-runner: would push {} to origin\n",
            run.branch
        ));
        output.push_str(&format!(
            "phase-runner: would run scripts/agent-pr.sh --base {} --head {} --verification <executor verification>\n",
            plan.config.base_branch, run.branch
        ));
        match plan.config.pr_lifecycle {
            PrLifecycle::OpenAndWait => {
                output.push_str("phase-runner: would run scripts/wait-pr.sh <opened-pr> before reporting success or continuing\n");
                output.push_str(&format!(
                    "phase-runner: would fetch origin/{} and verify the phase head is reachable from origin/{}\n",
                    plan.config.base_branch, plan.config.base_branch
                ));
            }
            PrLifecycle::OpenAndStop => {
                output.push_str(&format!(
                    "phase-runner: would stop with a pending handoff after arming auto-merge for {}\n",
                    run.branch
                ));
            }
        }
        output.push_str(
            &ExecutorPrompt::for_phase(&plan.config.plan.name, &run.phase, &run.branch).render(),
        );
        output.push('\n');
        if plan.config.pr_lifecycle == PrLifecycle::OpenAndStop {
            break;
        }
    }
    output.push_str(
        "phase-runner: dry run finished. No worktrees were created and no PRs were opened.\n",
    );
    output
}

fn git_rev_parse(repo_root: &Path, rev: &str) -> Result<String, CliError> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg(rev)
        .current_dir(repo_root)
        .output()
        .map_err(|err| CliError::Git(err.to_string()))?;
    if !output.status.success() {
        return Err(CliError::Git(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_options(args: Vec<String>) -> Result<CliOptions, CliError> {
    let mut options = CliOptions {
        plan_name: String::new(),
        base_branch: "main".to_string(),
        model: None,
        explicit_phases: Vec::new(),
        from_phase: None,
        to_phase: None,
        dry_run: false,
        pr_mode: false,
        wait_for_pr: false,
    };

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--plan" => {
                options.plan_name = take_value(&args, &mut index, "--plan")?;
            }
            "--base" => {
                options.base_branch = take_value(&args, &mut index, "--base")?;
            }
            "--model" => {
                options.model = Some(take_value(&args, &mut index, "--model")?);
            }
            "--from" => {
                options.from_phase = Some(take_value(&args, &mut index, "--from")?);
            }
            "--to" => {
                options.to_phase = Some(take_value(&args, &mut index, "--to")?);
            }
            "--dry-run" => {
                options.dry_run = true;
                index += 1;
            }
            "--pr" => {
                options.pr_mode = true;
                index += 1;
            }
            "--wait" => {
                options.wait_for_pr = true;
                index += 1;
            }
            option if option.starts_with("--") => {
                return Err(CliError::UnknownOption(option.to_string()));
            }
            phase => {
                options.explicit_phases.push(phase.to_string());
                index += 1;
            }
        }
    }

    if options.plan_name.is_empty() {
        return Err(CliError::MissingPlan);
    }
    Ok(options)
}

fn take_value(args: &[String], index: &mut usize, option: &str) -> Result<String, CliError> {
    let value_index = *index + 1;
    let value = args
        .get(value_index)
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| CliError::MissingValue {
            option: option.to_string(),
        })?;
    *index += 2;
    Ok(value)
}

fn validate_plan_name(name: &str) -> Result<(), CliError> {
    let simple = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && name.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '.' | '-')
        });
    if simple {
        Ok(())
    } else {
        Err(CliError::InvalidPlanName(name.to_string()))
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue { option } => write!(f, "missing value for {option}"),
            Self::UnknownOption(option) => write!(f, "unknown option: {option}"),
            Self::MissingPlan => write!(f, "--plan is required and at least one phase must be selected"),
            Self::PrRequired => write!(f, "phase-runner is PR-first now; pass --pr, optionally with --wait"),
            Self::WaitRequiresPr => write!(f, "--wait requires --pr"),
            Self::RangePairRequired => write!(f, "--from and --to must be used together"),
            Self::MixedExplicitAndRange => write!(
                f,
                "pass either explicit phases or --from/--to discovery, not both"
            ),
            Self::InvalidPlanName(name) => {
                write!(f, "plan name must be a simple plans/ directory name: {name}")
            }
            Self::InvalidBase(base) => write!(f, "phase-runner opens PRs against main; --base must be main, got {base}"),
            Self::MissingPlanFile(path) => write!(f, "missing plan entry point: {}", path.display()),
            Self::MissingSchemaFile(path) => write!(f, "missing result schema: {}", path.display()),
            Self::InvalidPhase(err) => write!(f, "{err}"),
            Self::MissingPhaseFile(path) => write!(f, "missing phase file: {}", path.display()),
            Self::PhaseDiscovery(err) => write!(f, "{err}"),
            Self::ExecuteUnsupported => write!(
                f,
                "the Rust runner currently supports --dry-run only; scripts/phase-runner.sh remains the active executor"
            ),
            Self::CurrentDir(err) => write!(f, "failed to read current directory: {err}"),
            Self::Git(err) => write!(f, "git command failed: {err}"),
        }
    }
}

impl std::error::Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_run_matches_shell_path_conventions() {
        let run = PhaseRun::new(
            "phaserunner",
            PhaseId::parse("1").unwrap(),
            PathBuf::from("/tmp/rts-worktrees"),
        );
        assert_eq!(run.branch, "zvorygin/phaserunner-phase-1");
        assert_eq!(
            run.layout.worktree_path,
            PathBuf::from("/tmp/rts-worktrees/phaserunner-phase-1")
        );
        assert_eq!(
            run.layout.handoff_file,
            PathBuf::from("/tmp/rts-worktrees/phase-runner-logs/phaserunner/handoffs/phase-1.json")
        );
        assert_eq!(
            run.layout.active_marker,
            PathBuf::from("/tmp/rts-worktrees/phase-runner-active/zvorygin__phaserunner-phase-1")
        );
    }

    #[test]
    fn parses_explicit_dry_run_cli_and_stops_after_first_phase_without_wait() {
        let repo = temp_repo("phaserunner-cli-explicit");
        write_plan(&repo, "phaserunner", &["phase-1.md", "phase-2.md"]);

        let action = plan_from_args(
            ["--plan", "phaserunner", "1", "2", "--pr", "--dry-run"],
            &repo,
        )
        .unwrap();
        let CliAction::DryRun(plan) = action else {
            panic!("expected dry run");
        };
        assert_eq!(plan.config.pr_lifecycle, PrLifecycle::OpenAndStop);
        assert_eq!(plan.runs.len(), 2);

        let output = render_dry_run(&plan);
        assert!(output
            .contains("phase-runner: would run Codex in /tmp/rts-worktrees/phaserunner-phase-1"));
        assert!(output.contains("phase-runner: would stop with a pending handoff after arming auto-merge for zvorygin/phaserunner-phase-1"));
        assert!(!output.contains("phaserunner-phase-2"));

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn discovers_range_and_continues_when_waiting() {
        let repo = temp_repo("phaserunner-cli-range");
        write_plan(
            &repo,
            "phaserunner",
            &["phase-1.md", "phase-2.md", "phase-2a.md", "phase-3.md"],
        );

        let action = plan_from_args(
            [
                "--plan",
                "phaserunner",
                "--from",
                "1",
                "--to",
                "3",
                "--pr",
                "--wait",
                "--dry-run",
            ],
            &repo,
        )
        .unwrap();
        let CliAction::DryRun(plan) = action else {
            panic!("expected dry run");
        };
        assert_eq!(
            plan.config
                .phases
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["phase-2", "phase-2a", "phase-3"]
        );
        let output = render_dry_run(&plan);
        assert!(output.contains("phaserunner-phase-2"));
        assert!(output.contains("phaserunner-phase-2a"));
        assert!(output.contains("phaserunner-phase-3"));
        assert!(output.contains("would run scripts/wait-pr.sh <opened-pr>"));

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn validates_preserved_option_rules() {
        let repo = temp_repo("phaserunner-cli-validation");
        write_plan(&repo, "phaserunner", &["phase-1.md"]);

        assert!(matches!(
            plan_from_args(["--plan", "phaserunner", "1", "--dry-run"], &repo),
            Err(CliError::PrRequired)
        ));
        assert!(matches!(
            plan_from_args(
                [
                    "--plan",
                    "phaserunner",
                    "1",
                    "--from",
                    "1",
                    "--to",
                    "2",
                    "--pr",
                    "--dry-run"
                ],
                &repo
            ),
            Err(CliError::MixedExplicitAndRange)
        ));
        assert!(matches!(
            plan_from_args(["--plan", "../bad", "1", "--pr", "--dry-run"], &repo),
            Err(CliError::InvalidPlanName(_))
        ));
        assert!(matches!(
            plan_from_args(
                [
                    "--plan",
                    "phaserunner",
                    "1",
                    "--base",
                    "develop",
                    "--pr",
                    "--dry-run"
                ],
                &repo
            ),
            Err(CliError::InvalidBase(_))
        ));

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn prompt_preserves_executor_contract_text() {
        let phase = PhaseId::parse("2").unwrap();
        let prompt =
            ExecutorPrompt::for_phase("phaserunner", &phase, "zvorygin/phaserunner-phase-2")
                .render();

        assert!(prompt.starts_with("$phase-runner\n\nExecute exactly one planned phase"));
        assert!(prompt.contains("Plan: plans/phaserunner/plan.md\nPhase: plans/phaserunner/phase-2.md\nCurrent branch: zvorygin/phaserunner-phase-2"));
        assert!(prompt.contains("Do not merge, push, or open a PR"));
        assert!(prompt.ends_with("Return a compact JSON handoff matching the requested schema."));
    }

    fn temp_repo(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("scripts")).unwrap();
        std::fs::write(path.join("scripts/phase-runner-result.schema.json"), "{}").unwrap();
        path
    }

    fn write_plan(repo: &Path, name: &str, phase_files: &[&str]) {
        let plan_dir = repo.join("plans").join(name);
        std::fs::create_dir_all(&plan_dir).unwrap();
        std::fs::write(plan_dir.join("plan.md"), "# Plan\n").unwrap();
        for phase_file in phase_files {
            std::fs::write(plan_dir.join(phase_file), "# Phase\n").unwrap();
        }
    }
}
