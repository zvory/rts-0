# Hotspot Cleanup Triage Plan

## Purpose

Turn the churn/hotspot scout into actionable cleanup evidence before any broad refactor begins. The
goal is to learn which large, frequently edited files are genuinely tangled, which ones are merely
large because they aggregate tests or mirrors, and which mechanical extractions would reduce
cognitive load without changing gameplay behavior. This plan produces responsibility maps,
ranked extraction candidates, and a repeatable hotspot-analysis path that later cleanup plans can
use as input.

This is an analysis and planning effort, not the cleanup itself. Phases may add Markdown reports,
CSV or JSON evidence files, and small tooling/runbook support for future hotspot analysis, but they
must not split runtime modules, move gameplay logic, or rewrite tests as part of this plan. If a
phase finds an urgent correctness bug while reading code, document it as a follow-up candidate
rather than fixing it here unless the user explicitly redirects the phase.

## Starting Hotspots

The initial scout pointed at these areas as the first audit targets:

- `server/src/lobby/room_task.rs`
- `tests/client_contracts.mjs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js`
- `client/src/config.js`
- `server/crates/rules/src/balance.rs`
- `client/src/match.js`
- `client/src/hud.js`
- `client/src/state.js`
- `client/styles.css`
- large simulation and AI test files under `server/crates/sim/src/game/**/tests.rs` and
  `server/crates/ai/src/**/tests.rs`

The room-task file is already under active cleanup in adjacent plans. This plan may analyze it
read-only and record boundaries, but it must not take ownership of room-runtime edits unless the
user explicitly merges that work into this plan.

## Overall Constraints

- Start every phase from fresh `origin/main` in an isolated `/tmp/rts-worktrees` worktree on a
  `zvorygin/` branch.
- Preserve unrelated dirty state, especially `playtest_notes.md`.
- Keep the work evidence-driven. Every recommendation must cite current code, Git history, tests,
  or design docs rather than intuition.
- Use rename-aware history where practical: `git log --follow --find-renames=<threshold>` for
  current file history, `git log --find-renames` for repository-wide rename events, and
  `git blame -w -M -C -C` when line-origin evidence matters.
- Treat copy/split history as heuristic. Git can follow whole-file renames well, but it cannot
  perfectly preserve identity after a file is split; the reports must call out that limitation.
- Exclude generated artifacts, archived plans, bulky replay fixtures, build outputs, and historical
  stale paths from the primary hotspot ranking unless a phase is explicitly analyzing artifact
  churn.
- Prefer architectural groups over raw file paths once a file has been split or moved. Future
  analysis should keep tracking groups such as protocol mirror, balance mirror, room runtime,
  client match shell, command service, and contract tests.
- Do not change gameplay behavior, wire protocol shape, balance values, client UI behavior,
  production CSS, or test assertions in these phases.
- Do not make broad cleanup PRs from this plan. The expected output is a ranked map of later cleanup
  plans and candidate extraction phases.
- Keep committed analysis artifacts concise enough to review. Large raw command output should be
  summarized or stored as machine-readable evidence only when it is directly useful.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited on until GitHub reports the PR merged and the phase head is
  reachable from `origin/main`.
- After each phase, the implementing agent must provide a handoff message with exact commands run,
  evidence produced, remaining uncertainty, next-phase guidance, and the core manual review focus.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phase Summaries

### [Phase 1 - Baseline Hotspot Dataset](phase-1.md)

Build a reproducible hotspot baseline from current `origin/main` using rename-aware file history,
recent churn, fix-looking commits, current file size, blame freshness, and temporal coupling. The
phase should produce a concise report plus machine-readable evidence that separates current source
hotspots from stale paths, generated artifacts, fixtures, and archived plan churn. The result should
explain what the metrics do and do not prove, so later phases use hotspots as triage input rather
than as a verdict.

### [Phase 2 - Responsibility Maps](phase-2.md)

Read the top hotspots and map their internal responsibilities, public entry points, collaborators,
contract boundaries, and test coverage seams. The phase should decide which sections are cleanly
extractable, which are coupled by real domain rules, and which should be left alone because another
active plan owns them. The result should be a responsibility map for each top hotspot and an
architectural-group map that future churn analysis can use after files are split.

### [Phase 3 - Extraction Candidate Ranking](phase-3.md)

Convert the metrics and responsibility maps into a ranked cleanup backlog. Each candidate should
state the proposed mechanical extraction, expected risk, expected payoff, files likely touched,
verification, manual review focus, and reasons to defer or avoid the work. The result should make it
clear which follow-up cleanup plans are safe mechanical decomposition and which require deeper
design work before code moves.

### [Phase 4 - Repeatable Hotspot Workflow](phase-4.md)

Turn the validated analysis method into a repeatable workflow for future runs. The phase should add
a small repo-local script or a precise runbook, plus a group-mapping file if needed, so future
hotspot passes can compare current-file churn, architectural-group churn, coupling, and split-file
history caveats without rebuilding the method from scratch. The result should include a final
handoff that says when to rerun the analysis, what thresholds are useful, and how to compare results
before and after cleanup PRs.

## Phase Index

1. [Phase 1 - Baseline Hotspot Dataset](phase-1.md)
2. [Phase 2 - Responsibility Maps](phase-2.md)
3. [Phase 3 - Extraction Candidate Ranking](phase-3.md)
4. [Phase 4 - Repeatable Hotspot Workflow](phase-4.md)

## Suggested Execution

Run one phase at a time and wait for each PR to merge before starting the next phase. This plan is
allowed to spend substantial analysis time and tool calls, but each phase should commit only the
smallest useful evidence artifacts.

```bash
scripts/phase-runner.sh --plan hotspots phase-1 phase-2 phase-3 phase-4 --pr --wait
```

After Phase 4 lands, use its ranked backlog to create separate cleanup plans for actual file splits
or architectural refactors.
