# Developer Tool Migration Plan

## Purpose

Move the normal developer and agent workflow commands out of shell-script orchestration and into a
small, testable, cross-platform Rust tool while keeping the existing command paths stable during the
migration. The first public interface is `./tool` on Unix-like systems and `tool.ps1` on Windows;
the implementation should call a repo-local Rust binary for migrated commands and fall back to the
existing Unix scripts for commands that are not migrated yet. The initial cross-platform command set
is `run-all`, `agent-pr`, and `wait-pr`; other operational scripts may remain Unix-only until they
become part of the normal Windows or agent lifecycle.

## Target Command Surface

- `./tool run-all [args...]`
- `./tool agent-pr [args...]`
- `./tool wait-pr <pr> [args...]`
- `./tool <legacy-command> [args...]` may fall back to an existing Unix script when that command is
  explicitly mapped and the host is Unix-like.
- `tool.ps1 <command> [args...]` must support migrated Rust commands on Windows and must print a
  clear unsupported-command error for Unix-only fallbacks.
- Existing stable entrypoints, including `tests/run-all.sh`, `scripts/agent-pr.sh`, and
  `scripts/wait-pr.sh`, stay in place as compatibility shims until the migration has proven itself.

## Phase Summaries

### [Phase 1 - Tool Facade and Rust CLI Scaffold](phase-1.md)

Add the repo-level `./tool` and `tool.ps1` launchers plus a small Rust CLI crate for migrated
developer commands. The launchers should be thin dispatchers, not a new place for orchestration
logic, and should route migrated commands to Rust while keeping explicit Unix fallback mappings for
unmigrated scripts. This phase establishes the command surface, help text, Cargo alias or documented
Cargo fallback, and unit-tested argument dispatch without changing `run-all`, `agent-pr`, or
`wait-pr` behavior yet.

### [Phase 2 - Run-All Characterization and Suite Model](phase-2.md)

Before replacing the current test runner, capture the behavior that must be preserved from
`tests/run-all.sh`. Model the suite graph, modes, environment variables, server lifecycle, skip
rules, timing summary, and client dependency cache behavior in Rust data structures with fake-runner
tests. This phase should make the eventual Rust implementation testable without spawning the full
suite and should leave the existing shell runner as the active implementation.

### [Phase 3 - Run-All Core Rust Orchestrator](phase-3.md)

Implement the first functional Rust-backed `run-all` path for the non-browser core: argument
parsing, per-worktree target dir selection, server build or prebuilt reuse, server boot and cleanup,
parallel suite execution, static JavaScript checks, live Node suites, Rust format, nextest, and
clippy. Route only the modes that have reached parity through Rust and let compatibility wrappers
fall back to the shell runner for unsupported modes. This phase should prove the process model,
failure aggregation, logs, and timing summaries before browser and Windows-specific edge cases are
added.

### [Phase 4 - Run-All Browser, Dependency Cache, and CI Parity](phase-4.md)

Complete Rust-backed `run-all` parity for browser smoke, tri-state browser scenarios, Chrome
detection, client dependency hydration, timing details, nextest JUnit summary, and the existing CI
sub-modes. Keep the required GitHub check name and the stable `tests/run-all.sh` entrypoint intact
while shifting its implementation to the Rust tool. This phase should preserve current full-gate
coverage, output usefulness, and performance characteristics closely enough that CI and local agent
workflows can use the Rust path by default on Unix-like systems.

### [Phase 5 - Native Windows Run-All Support](phase-5.md)

Make `tool.ps1 run-all` a supported Windows developer command for the same migrated modes that Unix
uses. Replace POSIX assumptions with portable Rust behavior for temp paths, child-process cleanup,
server health polling, Chrome discovery, dependency cache linking or copying, executable suffixes,
and path display. This phase should add Windows-focused automated coverage for the dev tool and a
documented manual canary for running the game test gate from a Windows client checkout.

### [Phase 6 - Agent-PR Rust Migration](phase-6.md)

Move `agent-pr` into the Rust tool while preserving the existing PR body metadata block, labels,
dry-run behavior, draft handling, owner detection, update-vs-create behavior, and auto-merge
arming. Keep using the GitHub CLI as the transport initially, but replace shell parsing and `jq`
dependencies with typed Rust JSON handling and fake `gh` runner tests. This phase makes PR creation
and ownership metadata reliable and testable before changing the wait loop.

### [Phase 7 - Wait-PR Rust Migration](phase-7.md)

Move `wait-pr` into the Rust tool with typed PR/check-state parsing, deterministic fixture tests,
timeout and once modes, failed-check summaries, and the existing merged-head ancestry verification.
Keep the old fixture environment variables or provide equivalent test-only inputs so current edge
cases remain easy to reproduce. This phase should make `tool.ps1 wait-pr` usable on Windows and keep
`scripts/wait-pr.sh` as a compatibility shim for phase-runner and older docs.

### [Phase 8 - Workflow Rollout and Script Debt Boundary](phase-8.md)

Update docs, wrappers, and agent workflow guidance so `./tool` is the preferred command while the
old paths remain stable compatibility entrypoints. Run the workflow canaries needed for confidence:
docs-only PR, representative implementation PR, and phase-runner serial PR wait. This phase draws
the explicit line between migrated core commands and Unix-only operational scripts so future cleanup
does not accidentally grow another orchestration layer in shell.

## Overall Constraints

- Keep compatibility first. Existing command paths must continue to work during the migration:
  `tests/run-all.sh`, `scripts/agent-pr.sh`, `scripts/wait-pr.sh`, and phase-runner invocations
  should not break while Rust implementations land.
- Keep `./tool` and `tool.ps1` thin. They may locate the repo root and dispatch, but orchestration,
  parsing, process lifecycle, GitHub state handling, and test-runner behavior belong in Rust modules
  with unit tests.
- Keep the required PR check context named `./tests/run-all.sh` unless a later explicit workflow
  decision changes branch protection. GitHub Actions may call the compatibility wrapper while the
  Rust tool owns the underlying behavior.
- Preserve `tests/run-all.sh` functionality and operator value: same modes, same relevant
  environment variables, private server boot/reuse behavior, per-worktree Cargo target policy,
  timing summary, skipped-suite explanations, nextest install hint, Chrome/browser behavior, and
  dependency-cache safety.
- Preserve `agent-pr` metadata exactly enough for the existing PR ownership workflow to keep
  accepting owned agent PRs.
- Preserve `wait-pr` completion semantics: do not report success until GitHub reports the PR merged
  and the head SHA is reachable from `origin/main` or the configured main ref.
- Use structured parsers and typed data where possible. Do not re-create `jq`, `awk`, or `sed`
  pipelines inside Rust as ad hoc string parsing.
- Keep `gh`, `git`, `cargo`, `node`, `npm`, Chrome, Fly, and other external tools as external tools
  where they are product dependencies. The migration goal is reliable orchestration, not replacing
  every external CLI.
- Add tests at the logic boundary. Process execution should go through an injectable runner so most
  command-state tests can run without invoking GitHub, Cargo, Node, or the full suite.
- Use platform-specific code only at clear seams such as child termination, executable lookup, temp
  locations, symlink or copy behavior, and PowerShell launcher behavior.
- Do not migrate unrelated Unix-only scripts in these phases unless the current phase explicitly
  needs a compatibility shim. `deploy.sh`, `fly-logs.sh`, docdrift daily launchers, sound preview
  helpers, and local macOS helpers can remain Unix scripts for now.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- When a phase is complete, mark that phase document done in the implementation commit and provide
  a handoff message describing what changed, what the next agent should do, and the core manual
  testing focus.

## Handoff Requirements

After every phase, the implementing agent must provide a handoff message for the next agent. The
handoff must summarize what shipped, any behavior intentionally left on the old shell path, the
focused verification that passed, and any blockers or platform caveats. Manual testing notes should
cover core workflows, for example `./tool run-all --only-rust`, `./tool agent-pr --dry-run`, or
`tool.ps1 wait-pr --help`, rather than an exhaustive test matrix.
