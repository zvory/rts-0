pub mod completion;
pub mod handoff;
pub mod phase;
pub mod pr;
pub mod runner;
pub mod timing;

pub use completion::phase_marked_done;
pub use handoff::{AutoMergeState, ExecutorHandoff, HandoffError, HandoffStatus, MergeWaitState};
pub use phase::{discover_phases, PhaseDiscoveryError, PhaseId, PhaseIdParseError};
pub use pr::{ensure_pr_ready, GitHubPullRequest, PrReadiness, PrReadinessError};
pub use runner::{PhaseRun, PlanRef, PrLifecycle, RunMode, RunnerConfig, WorktreeLayout};
pub use timing::{PhaseTiming, PhaseTimingSeconds};
