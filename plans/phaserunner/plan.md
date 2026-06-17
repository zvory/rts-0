# Rust Phase Runner Plan

## Status

Draft.

## Summary

Replace `scripts/phase-runner.sh` with a Rust utility that preserves the current executor and PR
lifecycle before adding new modes. The existing shell runner is a 700-line orchestration script that
mixes option parsing, phase discovery, prompt text, Git worktree management, Codex execution,
handoff JSON handling, PR automation, wait semantics, timing logs, and failure reporting in one
place. The Rust replacement should make those pieces explicit, unit-tested, and easy to extend for
future features such as prompt-section injection and experimental worktree iteration without PRs.

## Current Behavior To Preserve

- The runner is executor-only. It runs approved `plans/<name>/phase-*.md` files and must not create
  plans or perform final review.
- Current normal execution is PR-first: `--plan NAME` plus explicit phases or `--from/--to`, with
  `--pr` required and `--wait` optional. Without `--wait`, the runner opens and arms the first PR,
  then stops with a pending handoff.
- `--base` exists only for compatibility and must be `main`. Non-dry execution must start from a
  clean local `main` checkout with an `origin` remote.
- Phase ids accept `N`, `N.M`, `Na`, and `phase-*` forms. Range discovery sorts matching phase
  files, excludes the `--from` phase, includes the `--to` phase, and supports decimal interstitials
  such as `5.5` plus suffixed phases such as `3a`.
- Worktrees live under `RTS_WORKTREE_ROOT` or `/tmp/rts-worktrees`. Each phase uses
  `zvorygin/<plan>-<phase>` and `<worktree-root>/<plan>-<phase>`.
- The runner syncs local `main` from `origin/main` before each phase, records the base commit,
  creates one clean worktree and branch, and writes an active marker under
  `<worktree-root>/phase-runner-active`.
- The Codex invocation runs inside the phase worktree with `$phase-runner`, the plan path, the phase
  path, the branch name, the repo git common directory as an added directory, the structured output
  schema, and an output handoff JSON file under
  `<worktree-root>/phase-runner-logs/<plan>/handoffs`.
- The executor prompt requires exactly one phase, no nested worktree, no PR/push/merge by the inner
  executor, a successful commit before reporting completion, focused verification, and a compact
  JSON handoff.
- Completion is accepted only when the handoff status is `completed`, the worktree is clean, at
  least one commit exists over the recorded base, and the phase document is marked done using one
  of the accepted completion marker forms.
- The runner pushes the phase branch, writes a PR body from the handoff, invokes
  `scripts/agent-pr.sh`, verifies the PR is open, auto-merge is armed, and merge state is not dirty.
- With `--wait`, the runner invokes `scripts/wait-pr.sh`, fetches `origin/main`, verifies the phase
  head is reachable from `origin/main`, syncs local `main`, then continues to the next phase.
- The runner enriches the handoff JSON with PR fields and writes timing JSON for each phase.
- `--dry-run` creates no worktrees, opens no PRs, prints the planned worktree/PR actions, prints the
  rendered prompt, and continues across discovered phases only when `--wait` is set.
- The shell runner depends on `codex`, `node`, `gh`, and `jq` today. The Rust runner should remove
  Node from its own parsing and JSON duties, while initially preserving `gh`, `agent-pr.sh`, and
  `wait-pr.sh` as external lifecycle boundaries.

## Target Architecture

- Add a dedicated Rust workspace crate, tentatively `server/crates/phaserunner`, with a library for
  behavior and a thin binary for CLI wiring.
- Keep the public command available at `scripts/phase-runner.sh` during migration by turning the
  shell file into a compatibility wrapper once the Rust binary is ready.
- Model runner behavior as explicit data:
  `RunnerConfig`, `PlanRef`, `PhaseId`, `PhaseSelection`, `PhaseRun`, `WorktreeLayout`,
  `PromptTemplate`, `PromptSection`, `ExecutorHandoff`, `PrLifecycle`, and `RunMode`.
- Keep command execution behind a small trait or adapter so unit tests can verify Git, Codex, PR,
  and wait sequencing without creating real worktrees or hitting GitHub.
- Keep PR-first behavior as the only production mode until parity is proven. Design `RunMode` so a
  later phase can add local experimental iteration without changing phase discovery, prompt
  rendering, logging, or handoff validation.
- Treat prompt text as data with stable named sections. Initial parity should render the existing
  prompt, but the design should make future section injection an additive option instead of string
  surgery.
- Use typed `serde` structs for handoff and timing JSON. Keep
  `scripts/phase-runner-result.schema.json` in sync or generate it only after a deliberate schema
  decision.
- Prefer focused Rust unit tests for parsing, discovery, prompt rendering, handoff validation, PR
  readiness decisions, failure paths, and dry-run behavior. Use one tiny docs-only canary through
  the runner only after shell parity is covered by tests.

## Phase Summaries

1. Phase 1 captures the current runner contract in tests and scaffolds the Rust crate without
   changing the active shell behavior. It moves phase id parsing, range discovery, completion
   marker detection, handoff JSON types, and PR readiness decisions into testable Rust units. The
   outcome is a behavior model that can fail fast when the Rust implementation drifts from the
   shell runner's accepted semantics.
2. Phase 2 implements CLI parsing, worktree/log layout planning, dry-run output, and prompt
   rendering in Rust while the shell runner remains the default entrypoint. It preserves the
   current option surface that matters for existing calls and introduces named prompt sections
   internally without exposing new user-facing features yet. The outcome is a side-effect-free Rust
   runner path that can be compared against `scripts/phase-runner.sh --dry-run --pr`.
3. Phase 3 implements the single-phase executor lifecycle in Rust up through local validation of
   the executor result. It creates the phase worktree, runs Codex with the existing schema, validates
   completed handoffs, checks clean committed work, verifies the phase document is done, records
   logs/timing, and leaves blocked worktrees intact on failure. The outcome is a Rust executor core
   that can run one phase locally but still delegates PR lifecycle work to the next phase.
4. Phase 4 adds PR-first lifecycle parity and serial `--wait` behavior. It pushes phase branches,
   calls the existing `scripts/agent-pr.sh` and `scripts/wait-pr.sh` helpers, validates auto-merge
   state through `gh`, enriches handoff JSON, verifies head reachability from `origin/main`, and
   preserves the no-wait pending-handoff stop. The outcome is a Rust runner that matches all
   current production behavior while reusing the repository's established PR policy helpers.
5. Phase 5 cuts over the repository to the Rust runner and retires the shell implementation to a
   wrapper. It updates docs, canaries the new runner on a tiny docs-only phased plan, keeps rollback
   simple, and records the extension points for later prompt injection and experimental non-PR
   worktree iteration. The outcome is one maintained Rust utility with tests, repo docs, and the
   same operator-facing command path.

## Cross-Phase Constraints

- Preserve the existing PR-first contract until the Rust runner has full parity. Do not introduce
  experimental no-PR behavior in this plan; leave that as a follow-up that builds on `RunMode`.
- Keep phase-runner execution separate from plan creation and final review.
- Do not reimplement GitHub ownership policy before it is necessary. Initially call
  `scripts/agent-pr.sh` and `scripts/wait-pr.sh` so PR metadata, labels, auto-merge, and wait
  failure semantics stay centralized.
- Keep `scripts/phase-runner.sh` as the stable operator entrypoint throughout migration.
- Preserve current branch and path conventions unless a phase explicitly updates the docs and
  cleanup behavior that depend on them.
- Preserve failure inspectability. On Codex failure, blocked handoff, dirty worktree, missing
  commit, missing done marker, PR lifecycle failure, or wait failure, the runner must print the
  relevant path and leave the worktree/logs available for repair.
- Avoid broad local test bundles. Each implementation phase should run focused Rust tests and only
  the targeted script or docs checks that match touched files. The PR full gate remains
  authoritative.
- Update `plans/README.md`, `docs/context/planning.md`, `CLAUDE.md`, and README workflow snippets
  when the active runner behavior or invocation path changes.
- Every phase implementation must provide a handoff message naming the next step and the core
  manual checks to run.
- Each phase must be pushed as an owned PR with auto-merge armed. After opening each PR, wait for a
  definite merge and verify the phase head is reachable from `origin/main` before reporting the
  phase complete or starting the next phase.

## Phase Index

1. [Phase 1 - Behavior Model and Crate Scaffold](phase-1.md)
2. [Phase 2 - CLI, Layout, Dry Run, and Prompt Rendering](phase-2.md)
3. [Phase 3 - Executor Lifecycle Core](phase-3.md)
4. [Phase 4 - PR Lifecycle and Serial Wait Parity](phase-4.md)
5. [Phase 5 - Cutover, Docs, and Canary](phase-5.md)

## Follow-Up Capabilities After Parity

- Prompt-section injection for plan-specific warnings, product constraints, or temporary
  verification instructions.
- Experimental local iteration mode that creates or reuses a worktree and Codex prompt but does
  not push, open a PR, or mark the phase complete.
- Resume or repair commands that inspect an active marker, handoff JSON, branch, and worktree to
  report the exact blocked state.
- More structured lifecycle integration if `agent-pr.sh` and `wait-pr.sh` eventually become Rust
  utilities too.
