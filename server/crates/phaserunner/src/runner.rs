use crate::phase::PhaseId;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerConfig {
    pub plan: PlanRef,
    pub base_branch: String,
    pub worktree_root: PathBuf,
    pub run_mode: RunMode,
    pub pr_lifecycle: PrLifecycle,
    pub model: Option<String>,
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
}

impl PhaseRun {
    pub fn new(plan_name: impl Into<String>, phase: PhaseId, worktree_root: PathBuf) -> Self {
        let plan_name = plan_name.into();
        let phase_label = phase.to_string();
        let branch = format!("zvorygin/{plan_name}-{phase_label}");
        let worktree_path = worktree_root.join(format!("{plan_name}-{phase_label}"));
        let log_dir = worktree_root.join("phase-runner-logs").join(&plan_name);
        let handoff_dir = log_dir.join("handoffs");

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
                active_marker_dir: worktree_root.join("phase-runner-active"),
            },
        }
    }
}

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
    }
}
