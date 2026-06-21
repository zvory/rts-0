# Phase 1 - Baseline Hotspot Dataset

Status: done.

## Goal

Create a reproducible, rename-aware hotspot baseline for current `origin/main`. The result should
show which current source files and architectural areas deserve deeper review, while clearly
filtering out stale paths, generated artifacts, replay fixtures, build outputs, and archived plans.

## Scope

- Run a current-source hotspot pass over Rust, JavaScript, shell, CSS, and HTML files under
  `server/`, `client/`, `tests/`, and `scripts/`.
- Compute at least:
  - current LOC or non-empty LOC;
  - rename-aware commit touches per current file;
  - added plus deleted line churn per current file;
  - recent touches and recent churn, using a short window such as 7 or 14 days;
  - fix/regression-looking commit counts from commit subjects;
  - current-line freshness using `git blame -w -M -C -C` for the top hotspots;
  - temporal coupling pairs and high-degree files over a recent window;
  - repository-wide rename events that affect source files.
- Compare rename-aware current-file results against raw path-level churn to show why stale paths and
  artifacts should not drive cleanup priorities.
- Produce `plans/hotspots/baseline.md` with the summarized findings, commands, filters, caveats, and
  first-pass hotspot list.
- Produce a small machine-readable evidence file, for example
  `plans/hotspots/evidence/baseline-hotspots.json` or
  `plans/hotspots/evidence/baseline-hotspots.csv`, containing the top ranked rows.

## Expected Touch Points

- `plans/hotspots/baseline.md`
- `plans/hotspots/evidence/baseline-hotspots.json` or `.csv`
- `plans/hotspots/phase-1.md`

Do not modify runtime source, tests, protocol files, client modules, CSS, or design docs in this
phase.

## Method Notes

- Use `git log --no-merges --follow --find-renames=40% --numstat -- <file>` or an equivalent
  rename-aware command for current file history.
- Use `git log --no-merges --find-renames=40% --name-status` for repository-wide rename events.
- Use `git blame -w -M -C -C --line-porcelain -- <file>` for current-line freshness and moved/copied
  line origin checks on the highest-ranked files.
- Use `rg` and `fd` for source discovery and filtering.
- If a script is used temporarily to compute metrics, keep it in the phase handoff or evidence
  notes. Do not commit a permanent script until Phase 4 decides the repeatable workflow.

## Analysis Questions

- Which current files are both large and frequently edited?
- Which files have high churn because they are genuinely central, and which are high because they
  are mirrors, compatibility shims, or aggregate tests?
- Which files are frequent co-change hubs?
- Which hotspot rankings change materially when docs, fixtures, generated outputs, archived plans,
  and stale renamed paths are filtered out?
- Which metrics are reliable enough to drive Phase 2, and which are only weak hints?

## Verification

- `git status --short`
- `git diff --check`
- Re-run the documented metric command or temporary script and confirm it reproduces the committed
  evidence file.
- Manually inspect the top rows for obvious false positives such as generated fixtures, replay
  artifacts, archived plans, or stale pre-rename paths.

## Manual Review Focus

Review whether the top 10 to 20 hotspots make sense as human cleanup targets. Check especially
whether `server/src/lobby/room_task.rs`, `tests/client_contracts.mjs`, and
`server/crates/sim/src/game/services/commands.rs` remain high after filtering.

## Handoff

After implementation, mark this phase done and summarize the metric windows, filters, top hotspot
files, top coupled pairs, known false positives, and exact commands used. Tell the next phase which
files should receive responsibility maps first and which hotspots should be deferred because they
are owned by active work.
