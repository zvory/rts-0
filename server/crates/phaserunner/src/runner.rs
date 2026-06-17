use crate::completion::phase_marked_done;
use crate::handoff::{ExecutorHandoff, HandoffError, HandoffStatus};
use crate::phase::{discover_phases, PhaseDiscoveryError, PhaseId, PhaseIdParseError};
use crate::timing::{PhaseTiming, PhaseTimingSeconds};
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

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
pub struct ExecutionPlan {
    pub config: RunnerConfig,
    pub runs: Vec<PhaseRun>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalPhaseResult {
    pub phase: PhaseId,
    pub branch: String,
    pub base_commit: String,
    pub phase_head: String,
    pub timing_file: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionReport {
    pub completed: Vec<LocalPhaseResult>,
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
    Execute(ExecutionPlan),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub current_dir: PathBuf,
    pub combined_output_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub trait CommandRunner {
    fn run(&mut self, invocation: &CommandInvocation) -> Result<CommandResult, io::Error>;
}

#[derive(Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&mut self, invocation: &CommandInvocation) -> Result<CommandResult, io::Error> {
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.args)
            .current_dir(&invocation.current_dir);
        if let Some(path) = &invocation.combined_output_file {
            let file = fs::File::create(path)?;
            let stderr_file = file.try_clone()?;
            let status = command
                .stdout(Stdio::from(file))
                .stderr(Stdio::from(stderr_file))
                .status()?;
            Ok(CommandResult {
                success: status.success(),
                stdout: String::new(),
                stderr: String::new(),
            })
        } else {
            let output = command.output()?;
            Ok(CommandResult {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            })
        }
    }
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
    ExecuteRequiresSinglePhase,
    CurrentDir(String),
    Git(String),
}

#[derive(Debug)]
pub enum ExecutionError {
    Command {
        command: String,
        message: String,
    },
    CommandFailed {
        command: String,
        stderr: String,
    },
    MissingTool(String),
    NotOnBase {
        expected: String,
        actual: String,
    },
    DirtyBase,
    MissingOrigin,
    ExistingBranch(String),
    ExistingWorktree(PathBuf),
    CodexFailed {
        phase: PhaseId,
        worktree: PathBuf,
        log: PathBuf,
        tail: String,
    },
    Handoff(HandoffError),
    HandoffBlocked {
        phase: PhaseId,
        status: HandoffStatus,
        worktree: PathBuf,
        log: PathBuf,
        tail: String,
    },
    DirtyWorktree {
        phase: PhaseId,
        worktree: PathBuf,
        status: String,
        log_tail: String,
    },
    MissingCommit {
        phase: PhaseId,
        base_commit: String,
        worktree: PathBuf,
        log_tail: String,
    },
    MissingDoneMarker {
        phase: PhaseId,
        phase_file: PathBuf,
    },
    Io {
        path: PathBuf,
        message: String,
    },
    PrLifecycleUnavailable {
        branch: String,
        phase_head: String,
    },
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

impl CommandInvocation {
    pub fn new<I, S>(program: impl Into<String>, args: I, current_dir: &Path) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            current_dir: command_dir(current_dir),
            combined_output_file: None,
        }
    }

    pub fn with_combined_output(mut self, path: PathBuf) -> Self {
        self.combined_output_file = Some(path);
        self
    }

    fn display_name(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn command_dir(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
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

    let runs = config.phase_runs();
    if config.run_mode == RunMode::DryRun {
        Ok(CliAction::DryRun(DryRunPlan {
            runs,
            config,
            base_commit: None,
        }))
    } else {
        if runs.len() != 1 {
            return Err(CliError::ExecuteRequiresSinglePhase);
        }
        Ok(CliAction::Execute(ExecutionPlan { runs, config }))
    }
}

pub fn plan_from_env() -> Result<CliAction, CliError> {
    let repo_root = env::current_dir().map_err(|err| CliError::CurrentDir(err.to_string()))?;
    let mut action = plan_from_args(env::args().skip(1), &repo_root)?;
    if let CliAction::DryRun(plan) = &mut action {
        plan.base_commit = Some(git_rev_parse(&repo_root, &plan.config.base_branch)?);
    }
    Ok(action)
}

pub fn execute_plan(
    plan: &ExecutionPlan,
    runner: &mut dyn CommandRunner,
) -> Result<ExecutionReport, ExecutionError> {
    for tool in ["codex", "gh", "jq"] {
        ensure_tool(tool, &plan.config.plan.plan_file, runner)?;
    }
    ensure_base_checkout(plan, runner)?;

    let mut completed = Vec::new();
    for run in &plan.runs {
        reject_existing_branch(run, &plan.config.plan.plan_file, runner)?;
        if run.layout.worktree_path.exists() {
            return Err(ExecutionError::ExistingWorktree(
                run.layout.worktree_path.clone(),
            ));
        }

        run_command(
            runner,
            CommandInvocation::new(
                "git",
                ["fetch", "origin", plan.config.base_branch.as_str()],
                &plan.config.plan.plan_file,
            ),
        )?;
        run_command(
            runner,
            CommandInvocation::new(
                "git",
                [
                    "merge",
                    "--ff-only",
                    &format!("origin/{}", plan.config.base_branch),
                ],
                &plan.config.plan.plan_file,
            ),
        )?;
        let base_commit = command_stdout(
            runner,
            CommandInvocation::new(
                "git",
                ["rev-parse", plan.config.base_branch.as_str()],
                &plan.config.plan.plan_file,
            ),
        )?;

        let phase_start = Instant::now();
        run_command(
            runner,
            CommandInvocation::new(
                "git",
                [
                    "worktree",
                    "add",
                    run.layout.worktree_path.to_string_lossy().as_ref(),
                    "-b",
                    &run.branch,
                    plan.config.base_branch.as_str(),
                ],
                &plan.config.plan.plan_file,
            ),
        )?;
        write_active_marker(run)?;
        fs::create_dir_all(
            run.layout
                .handoff_file
                .parent()
                .unwrap_or(&run.layout.log_dir),
        )
        .map_err(|err| io_err(&run.layout.handoff_file, err))?;
        fs::create_dir_all(&run.layout.log_dir).map_err(|err| io_err(&run.layout.log_dir, err))?;

        let prompt = ExecutorPrompt::for_phase(&plan.config.plan.name, &run.phase, &run.branch);
        let git_common_dir = command_stdout(
            runner,
            CommandInvocation::new(
                "git",
                ["rev-parse", "--path-format=absolute", "--git-common-dir"],
                &plan.config.plan.plan_file,
            ),
        )?;
        let executor_start = Instant::now();
        let codex_result = runner
            .run(&codex_invocation(
                plan,
                run,
                Path::new(&git_common_dir),
                &prompt.render(),
            ))
            .map_err(|err| command_err("codex", err))?;
        let executor_seconds = executor_start.elapsed().as_secs();
        if !codex_result.success {
            return Err(ExecutionError::CodexFailed {
                phase: run.phase.clone(),
                worktree: run.layout.worktree_path.clone(),
                log: run.layout.codex_log.clone(),
                tail: log_tail(&run.layout.codex_log),
            });
        }

        let handoff_text = fs::read_to_string(&run.layout.handoff_file)
            .map_err(|err| io_err(&run.layout.handoff_file, err))?;
        let handoff =
            ExecutorHandoff::parse_json(&handoff_text).map_err(ExecutionError::Handoff)?;
        if handoff.status != HandoffStatus::Completed {
            return Err(ExecutionError::HandoffBlocked {
                phase: run.phase.clone(),
                status: handoff.status,
                worktree: run.layout.worktree_path.clone(),
                log: run.layout.codex_log.clone(),
                tail: log_tail(&run.layout.codex_log),
            });
        }
        let _ = fs::remove_file(&run.layout.active_marker);

        let status = command_stdout(
            runner,
            CommandInvocation::new(
                "git",
                ["status", "--porcelain=v1"],
                &run.layout.worktree_path,
            ),
        )?;
        if !status.trim().is_empty() {
            return Err(ExecutionError::DirtyWorktree {
                phase: run.phase.clone(),
                worktree: run.layout.worktree_path.clone(),
                status,
                log_tail: log_tail(&run.layout.codex_log),
            });
        }

        let commit_count = command_stdout(
            runner,
            CommandInvocation::new(
                "git",
                ["rev-list", "--count", &format!("{base_commit}..HEAD")],
                &run.layout.worktree_path,
            ),
        )?;
        if commit_count.trim() == "0" {
            return Err(ExecutionError::MissingCommit {
                phase: run.phase.clone(),
                base_commit,
                worktree: run.layout.worktree_path.clone(),
                log_tail: log_tail(&run.layout.codex_log),
            });
        }

        let phase_file = run
            .layout
            .worktree_path
            .join("plans")
            .join(&plan.config.plan.name)
            .join(run.phase.file_name());
        let phase_text = fs::read_to_string(&phase_file).map_err(|err| io_err(&phase_file, err))?;
        if !phase_marked_done(&phase_text) {
            return Err(ExecutionError::MissingDoneMarker {
                phase: run.phase.clone(),
                phase_file,
            });
        }

        let phase_head = command_stdout(
            runner,
            CommandInvocation::new("git", ["rev-parse", "HEAD"], &run.layout.worktree_path),
        )?;
        write_timing(
            run,
            &base_commit,
            &phase_head,
            executor_seconds,
            phase_start.elapsed().as_secs(),
        )?;
        completed.push(LocalPhaseResult {
            phase: run.phase.clone(),
            branch: run.branch.clone(),
            base_commit,
            phase_head: phase_head.clone(),
            timing_file: run.layout.timing_file.clone(),
        });

        return Err(ExecutionError::PrLifecycleUnavailable {
            branch: run.branch.clone(),
            phase_head,
        });
    }

    Ok(ExecutionReport { completed })
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

fn ensure_tool(
    tool: &str,
    repo_path: &Path,
    runner: &mut dyn CommandRunner,
) -> Result<(), ExecutionError> {
    let probe = format!("command -v {tool}");
    let result = runner
        .run(&CommandInvocation::new(
            "sh",
            ["-c", probe.as_str()],
            repo_path,
        ))
        .map_err(|err| command_err("command -v", err))?;
    if result.success {
        Ok(())
    } else {
        Err(ExecutionError::MissingTool(tool.to_string()))
    }
}

fn ensure_base_checkout(
    plan: &ExecutionPlan,
    runner: &mut dyn CommandRunner,
) -> Result<(), ExecutionError> {
    let repo_path = &plan.config.plan.plan_file;
    let actual = command_stdout(
        runner,
        CommandInvocation::new("git", ["branch", "--show-current"], repo_path),
    )?;
    if actual != plan.config.base_branch {
        return Err(ExecutionError::NotOnBase {
            expected: plan.config.base_branch.clone(),
            actual,
        });
    }
    let status = command_stdout(
        runner,
        CommandInvocation::new("git", ["status", "--porcelain=v1"], repo_path),
    )?;
    if !status.trim().is_empty() {
        return Err(ExecutionError::DirtyBase);
    }
    let origin = runner
        .run(&CommandInvocation::new(
            "git",
            ["remote", "get-url", "origin"],
            repo_path,
        ))
        .map_err(|err| command_err("git remote get-url origin", err))?;
    if origin.success {
        Ok(())
    } else {
        Err(ExecutionError::MissingOrigin)
    }
}

fn reject_existing_branch(
    run: &PhaseRun,
    repo_path: &Path,
    runner: &mut dyn CommandRunner,
) -> Result<(), ExecutionError> {
    let result = runner
        .run(&CommandInvocation::new(
            "git",
            [
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{}", run.branch),
            ],
            repo_path,
        ))
        .map_err(|err| command_err("git show-ref", err))?;
    if result.success {
        Err(ExecutionError::ExistingBranch(run.branch.clone()))
    } else {
        Ok(())
    }
}

fn codex_invocation(
    plan: &ExecutionPlan,
    run: &PhaseRun,
    git_common_dir: &Path,
    prompt: &str,
) -> CommandInvocation {
    let repo_root = repo_root_from_plan(&plan.config.plan);
    let schema_file = repo_root
        .join("scripts")
        .join("phase-runner-result.schema.json");
    let mut args = vec![
        "exec".to_string(),
        "--cd".to_string(),
        run.layout.worktree_path.to_string_lossy().to_string(),
        "--add-dir".to_string(),
        git_common_dir.to_string_lossy().to_string(),
        "--sandbox".to_string(),
        "workspace-write".to_string(),
        "--output-schema".to_string(),
        schema_file.to_string_lossy().to_string(),
        "--output-last-message".to_string(),
        run.layout.handoff_file.to_string_lossy().to_string(),
    ];
    if let Some(model) = &plan.config.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }
    args.push(prompt.to_string());
    CommandInvocation::new("codex", args, &run.layout.worktree_path)
        .with_combined_output(run.layout.codex_log.clone())
}

fn repo_root_from_plan(plan: &PlanRef) -> PathBuf {
    plan.plan_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn write_active_marker(run: &PhaseRun) -> Result<(), ExecutionError> {
    fs::create_dir_all(&run.layout.active_marker_dir)
        .map_err(|err| io_err(&run.layout.active_marker_dir, err))?;
    fs::write(
        &run.layout.active_marker,
        format!(
            "plan={}\nphase={}\nbranch={}\nworktree={}\n",
            run.plan_name,
            run.phase,
            run.branch,
            run.layout.worktree_path.display()
        ),
    )
    .map_err(|err| io_err(&run.layout.active_marker, err))
}

fn write_timing(
    run: &PhaseRun,
    base_commit: &str,
    phase_head: &str,
    executor_seconds: u64,
    total_seconds: u64,
) -> Result<(), ExecutionError> {
    let timing = PhaseTiming {
        phase: run.phase.to_string(),
        branch: run.branch.clone(),
        base_ref: base_commit.to_string(),
        phase_head: phase_head.to_string(),
        pr_number: None,
        pr_url: None,
        merge_wait_state: "not_waited".to_string(),
        timings_seconds: PhaseTimingSeconds {
            total: total_seconds,
            executor: executor_seconds,
            pr: 0,
            wait: 0,
        },
    };
    let json = serde_json::to_string_pretty(&timing).map_err(|err| ExecutionError::Command {
        command: "serialize timing".to_string(),
        message: err.to_string(),
    })? + "\n";
    fs::write(&run.layout.timing_file, json).map_err(|err| io_err(&run.layout.timing_file, err))
}

fn run_command(
    runner: &mut dyn CommandRunner,
    invocation: CommandInvocation,
) -> Result<(), ExecutionError> {
    let display = invocation.display_name();
    let result = runner
        .run(&invocation)
        .map_err(|err| command_err(&display, err))?;
    if result.success {
        Ok(())
    } else {
        Err(ExecutionError::CommandFailed {
            command: display,
            stderr: result.stderr,
        })
    }
}

fn command_stdout(
    runner: &mut dyn CommandRunner,
    invocation: CommandInvocation,
) -> Result<String, ExecutionError> {
    let display = invocation.display_name();
    let result = runner
        .run(&invocation)
        .map_err(|err| command_err(&display, err))?;
    if result.success {
        Ok(result.stdout.trim().to_string())
    } else {
        Err(ExecutionError::CommandFailed {
            command: display,
            stderr: result.stderr,
        })
    }
}

fn command_err(command: &str, err: io::Error) -> ExecutionError {
    ExecutionError::Command {
        command: command.to_string(),
        message: err.to_string(),
    }
}

fn io_err(path: &Path, err: io::Error) -> ExecutionError {
    ExecutionError::Io {
        path: path.to_path_buf(),
        message: err.to_string(),
    }
}

fn log_tail(path: &Path) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(80);
    lines[start..].join("\n")
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
            Self::MissingPlan => write!(
                f,
                "--plan is required and at least one phase must be selected"
            ),
            Self::PrRequired => write!(
                f,
                "phase-runner is PR-first now; pass --pr, optionally with --wait"
            ),
            Self::WaitRequiresPr => write!(f, "--wait requires --pr"),
            Self::RangePairRequired => write!(f, "--from and --to must be used together"),
            Self::MixedExplicitAndRange => write!(
                f,
                "pass either explicit phases or --from/--to discovery, not both"
            ),
            Self::InvalidPlanName(name) => {
                write!(
                    f,
                    "plan name must be a simple plans/ directory name: {name}"
                )
            }
            Self::InvalidBase(base) => write!(
                f,
                "phase-runner opens PRs against main; --base must be main, got {base}"
            ),
            Self::MissingPlanFile(path) => {
                write!(f, "missing plan entry point: {}", path.display())
            }
            Self::MissingSchemaFile(path) => write!(f, "missing result schema: {}", path.display()),
            Self::InvalidPhase(err) => write!(f, "{err}"),
            Self::MissingPhaseFile(path) => write!(f, "missing phase file: {}", path.display()),
            Self::PhaseDiscovery(err) => write!(f, "{err}"),
            Self::ExecuteRequiresSinglePhase => write!(
                f,
                "the Rust runner currently supports exactly one non-dry phase per invocation"
            ),
            Self::CurrentDir(err) => write!(f, "failed to read current directory: {err}"),
            Self::Git(err) => write!(f, "git command failed: {err}"),
        }
    }
}

impl std::error::Error for CliError {}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { command, message } => {
                write!(f, "{command} could not be started: {message}")
            }
            Self::CommandFailed { command, stderr } => {
                write!(f, "{command} failed: {}", stderr.trim())
            }
            Self::MissingTool(tool) => write!(f, "required tool is not available: {tool}"),
            Self::NotOnBase { expected, actual } => {
                write!(
                    f,
                    "start phase-runner from the local {expected} checkout, got {actual}"
                )
            }
            Self::DirtyBase => write!(f, "local base checkout has uncommitted changes"),
            Self::MissingOrigin => write!(f, "origin remote is required"),
            Self::ExistingBranch(branch) => write!(f, "branch already exists: {branch}"),
            Self::ExistingWorktree(path) => {
                write!(f, "worktree path already exists: {}", path.display())
            }
            Self::CodexFailed {
                phase,
                worktree,
                log,
                tail,
            } => write!(
                f,
                "Codex failed for {phase}; leaving worktree at {} (log: {})\n{}",
                worktree.display(),
                log.display(),
                tail
            ),
            Self::Handoff(err) => write!(f, "{err}"),
            Self::HandoffBlocked {
                phase,
                status,
                worktree,
                log,
                tail,
            } => write!(
                f,
                "{phase} reported status '{status:?}'; leaving worktree for inspection: {} (log: {})\n{}",
                worktree.display(),
                log.display(),
                tail
            ),
            Self::DirtyWorktree {
                phase,
                worktree,
                status,
                log_tail,
            } => write!(
                f,
                "{phase} reported completed but left uncommitted changes in {}\n{}\n{}",
                worktree.display(),
                status,
                log_tail
            ),
            Self::MissingCommit {
                phase,
                base_commit,
                worktree,
                log_tail,
            } => write!(
                f,
                "{phase} reported completed but created no commit over {base_commit} in {}\n{}",
                worktree.display(),
                log_tail
            ),
            Self::MissingDoneMarker { phase, phase_file } => write!(
                f,
                "{phase} reported completed but did not mark the phase document done: {}",
                phase_file.display()
            ),
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::PrLifecycleUnavailable { branch, phase_head } => write!(
                f,
                "local executor validation succeeded for {branch} at {phase_head}; branch push and PR lifecycle remain unavailable until Phase 4"
            ),
        }
    }
}

impl std::error::Error for ExecutionError {}

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

    #[test]
    fn local_execution_validates_success_then_blocks_pr_lifecycle() {
        let fixture = execution_fixture("phaserunner-local-success");
        let mut fake = FakeCommandRunner::new(FakeScenario::Success, fixture.run.clone());

        let err = execute_plan(&fixture.plan, &mut fake).unwrap_err();

        assert!(matches!(
            err,
            ExecutionError::PrLifecycleUnavailable { branch, phase_head }
                if branch == "zvorygin/phaserunner-phase-3" && phase_head == "head123"
        ));
        assert!(fixture.run.layout.timing_file.is_file());
        let timing = std::fs::read_to_string(&fixture.run.layout.timing_file).unwrap();
        assert!(timing.contains(r#""phase": "phase-3""#));
        let codex = fake
            .invocations
            .iter()
            .find(|call| call.program == "codex")
            .expect("codex should run");
        assert!(codex.args.contains(&"--output-schema".to_string()));
        assert!(codex.args.contains(&"--output-last-message".to_string()));
        assert!(codex
            .args
            .iter()
            .any(|arg| arg.contains("Current branch: zvorygin/phaserunner-phase-3")));

        let _ = std::fs::remove_dir_all(fixture.repo);
        let _ = std::fs::remove_dir_all(fixture.worktree_root);
    }

    #[test]
    fn local_execution_rejects_existing_branch_and_worktree() {
        let fixture = execution_fixture("phaserunner-existing-branch");
        let mut fake = FakeCommandRunner::new(FakeScenario::ExistingBranch, fixture.run.clone());
        assert!(matches!(
            execute_plan(&fixture.plan, &mut fake),
            Err(ExecutionError::ExistingBranch(branch))
                if branch == "zvorygin/phaserunner-phase-3"
        ));
        let _ = std::fs::remove_dir_all(fixture.repo);
        let _ = std::fs::remove_dir_all(fixture.worktree_root);

        let fixture = execution_fixture("phaserunner-existing-worktree");
        std::fs::create_dir_all(&fixture.run.layout.worktree_path).unwrap();
        let mut fake = FakeCommandRunner::new(FakeScenario::Success, fixture.run.clone());
        assert!(matches!(
            execute_plan(&fixture.plan, &mut fake),
            Err(ExecutionError::ExistingWorktree(path))
                if path == fixture.run.layout.worktree_path
        ));
        let _ = std::fs::remove_dir_all(fixture.repo);
        let _ = std::fs::remove_dir_all(fixture.worktree_root);
    }

    #[test]
    fn local_execution_covers_executor_and_validation_failures() {
        for (name, scenario, expected) in [
            ("codex-failure", FakeScenario::CodexFailure, "codex failed"),
            (
                "blocked-handoff",
                FakeScenario::BlockedHandoff,
                "blocked handoff",
            ),
            (
                "dirty-worktree",
                FakeScenario::DirtyWorktree,
                "dirty worktree",
            ),
            (
                "missing-commit",
                FakeScenario::MissingCommit,
                "missing commit",
            ),
            (
                "missing-done",
                FakeScenario::MissingDoneMarker,
                "missing done marker",
            ),
        ] {
            let fixture = execution_fixture(&format!("phaserunner-{name}"));
            let mut fake = FakeCommandRunner::new(scenario, fixture.run.clone());
            let err = execute_plan(&fixture.plan, &mut fake).unwrap_err();
            match expected {
                "codex failed" => assert!(matches!(err, ExecutionError::CodexFailed { .. })),
                "blocked handoff" => {
                    assert!(matches!(err, ExecutionError::HandoffBlocked { .. }))
                }
                "dirty worktree" => {
                    assert!(matches!(err, ExecutionError::DirtyWorktree { .. }))
                }
                "missing commit" => {
                    assert!(matches!(err, ExecutionError::MissingCommit { .. }))
                }
                "missing done marker" => {
                    assert!(matches!(err, ExecutionError::MissingDoneMarker { .. }))
                }
                _ => unreachable!(),
            }
            let _ = std::fs::remove_dir_all(fixture.repo);
            let _ = std::fs::remove_dir_all(fixture.worktree_root);
        }
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

    struct ExecutionFixture {
        repo: PathBuf,
        worktree_root: PathBuf,
        plan: ExecutionPlan,
        run: PhaseRun,
    }

    fn execution_fixture(name: &str) -> ExecutionFixture {
        let repo = temp_repo(name);
        write_plan(&repo, "phaserunner", &["phase-3.md"]);
        let worktree_root = repo.with_file_name(format!(
            "{}-worktrees",
            repo.file_name().unwrap().to_string_lossy()
        ));
        let phase = PhaseId::parse("3").unwrap();
        let run = PhaseRun::new("phaserunner", phase.clone(), worktree_root.clone());
        let config = RunnerConfig {
            plan: PlanRef::new(&repo, "phaserunner"),
            base_branch: "main".to_string(),
            worktree_root,
            run_mode: RunMode::Execute,
            pr_lifecycle: PrLifecycle::OpenAndStop,
            model: None,
            phases: vec![phase],
            phase_selection: PhaseSelection::Explicit,
        };
        ExecutionFixture {
            repo,
            worktree_root: config.worktree_root.clone(),
            plan: ExecutionPlan {
                config,
                runs: vec![run.clone()],
            },
            run,
        }
    }

    #[derive(Clone, Copy)]
    enum FakeScenario {
        Success,
        ExistingBranch,
        CodexFailure,
        BlockedHandoff,
        DirtyWorktree,
        MissingCommit,
        MissingDoneMarker,
    }

    struct FakeCommandRunner {
        scenario: FakeScenario,
        run: PhaseRun,
        invocations: Vec<CommandInvocation>,
    }

    impl FakeCommandRunner {
        fn new(scenario: FakeScenario, run: PhaseRun) -> Self {
            Self {
                scenario,
                run,
                invocations: Vec::new(),
            }
        }

        fn create_worktree(&self) {
            let phase_dir = self
                .run
                .layout
                .worktree_path
                .join("plans")
                .join(&self.run.plan_name);
            std::fs::create_dir_all(&phase_dir).unwrap();
            let text = match self.scenario {
                FakeScenario::MissingDoneMarker => "# Phase 3\n\n## Status\n\nDraft.\n",
                _ => "# Phase 3\n\n## Status\n\nDone.\n",
            };
            std::fs::write(phase_dir.join(self.run.phase.file_name()), text).unwrap();
        }

        fn write_handoff(&self, status: &str, blocked_reason: &str) {
            let parent = self.run.layout.handoff_file.parent().unwrap();
            std::fs::create_dir_all(parent).unwrap();
            std::fs::write(
                &self.run.layout.handoff_file,
                format!(
                    r#"{{
                        "status":"{status}",
                        "summary":"summary",
                        "files_changed":["server/crates/phaserunner/src/runner.rs"],
                        "verification":["fake verification"],
                        "gameplay_impact":"none",
                        "next_executor_notes":"next",
                        "manual_test_notes":"manual",
                        "blocked_reason":"{blocked_reason}",
                        "pr_number":null,
                        "pr_url":null,
                        "head_sha":null,
                        "auto_merge_state":"not_requested",
                        "merge_wait_state":"not_waited"
                    }}"#
                ),
            )
            .unwrap();
            std::fs::write(&self.run.layout.codex_log, "executor log\n").unwrap();
        }
    }

    impl CommandRunner for FakeCommandRunner {
        fn run(&mut self, invocation: &CommandInvocation) -> Result<CommandResult, io::Error> {
            self.invocations.push(invocation.clone());
            let success = |stdout: &str| {
                Ok(CommandResult {
                    success: true,
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                })
            };
            let failure = |stderr: &str| {
                Ok(CommandResult {
                    success: false,
                    stdout: String::new(),
                    stderr: stderr.to_string(),
                })
            };

            let args = invocation
                .args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            match (invocation.program.as_str(), args.as_slice()) {
                ("sh", ["-c", _]) => success("/usr/bin/tool\n"),
                ("git", ["branch", "--show-current"]) => success("main\n"),
                ("git", ["status", "--porcelain=v1"])
                    if invocation.current_dir == self.run.layout.worktree_path =>
                {
                    match self.scenario {
                        FakeScenario::DirtyWorktree => success(" M plans/phaserunner/phase-3.md\n"),
                        _ => success(""),
                    }
                }
                ("git", ["status", "--porcelain=v1"]) => success(""),
                ("git", ["remote", "get-url", "origin"]) => success("git@example.test:rts.git\n"),
                ("git", ["show-ref", "--verify", "--quiet", _]) => match self.scenario {
                    FakeScenario::ExistingBranch => success(""),
                    _ => failure("missing ref"),
                },
                ("git", ["fetch", "origin", "main"]) => success(""),
                ("git", ["merge", "--ff-only", "origin/main"]) => success(""),
                ("git", ["rev-parse", "main"]) => success("base123\n"),
                ("git", ["rev-parse", "--path-format=absolute", "--git-common-dir"]) => {
                    success("/tmp/fake-common-git\n")
                }
                ("git", ["worktree", "add", _, "-b", _, "main"]) => {
                    self.create_worktree();
                    success("")
                }
                ("codex", _) => match self.scenario {
                    FakeScenario::CodexFailure => {
                        std::fs::create_dir_all(&self.run.layout.log_dir).unwrap();
                        std::fs::write(&self.run.layout.codex_log, "codex exploded\n").unwrap();
                        failure("codex failed")
                    }
                    FakeScenario::BlockedHandoff => {
                        self.write_handoff("blocked", "blocked by fake");
                        success("")
                    }
                    _ => {
                        self.write_handoff("completed", "");
                        success("")
                    }
                },
                ("git", ["rev-list", "--count", _]) => match self.scenario {
                    FakeScenario::MissingCommit => success("0\n"),
                    _ => success("1\n"),
                },
                ("git", ["rev-parse", "HEAD"]) => success("head123\n"),
                _ => panic!("unexpected command: {:?}", invocation),
            }
        }
    }
}
