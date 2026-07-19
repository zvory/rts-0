# Phase 2 - Beta Evidence Gate and Neutral Analysis

## Phase Status

- [x] Archived as stale and not completed on 2026-07-18.

The proposed evidence window and incident assumptions are no longer trusted as a current
investigation basis. A future investigation may reuse individual diagnostic ideas only after
validating them against fresh deployed behavior and current protocol/reporting code.

## Objective

Require a fresh beta reproduction after phase 1 diagnostics are deployed, preserve the logs and
replay/artifacts, and analyze what happened without prescribing a fix. This is a hard stop gate:
later work should not proceed from the old Alex/Commander incident alone.

## Work

- Confirm the deployed beta build includes phase 1 diagnostics by checking `/version` and/or relevant
  startup/log evidence.
- Ask the user to manufacture three beta windows:
  - idle or no-command baseline
  - normal command cadence
  - high-density repeated command burst that triggers rising HUD `jit` or visible stutter
- For each window, preserve:
  - bounded Fly logs around the run
  - parser markdown/JSON/TSV output
  - match-history row and replay artifact when the run records one
  - lifecycle rows if the room is aborted before match history writes
  - brief player-report notes with concrete timestamps when possible
- Compare windows using only evidence:
  - command density
  - reliable-message/snapshot timing fields
  - snapshot jitter/gap/burst fields
  - RTT and command timing fields
  - frame gap/worst frame phase
  - prediction mode, disable reasons, correction, WASM replay cost, predicted snapshot presence
  - server tick/scheduler lag, slow ticks, head-of-line/backlog, snapshot replace/send age
- Write an incident evidence directory under `docs/network-incident-examples/` with raw logs,
  summaries, replay artifacts if available, quotes/notes, and `analysis.md`.
- The analysis must distinguish supported findings, contradicted findings, unknowns, and next
  diagnostic gaps. It must not recommend or prescribe a product fix.

## Expected Touch Points

- `docs/network-incident-examples/<date>-beta-command-jitter-repro/`
- `scripts/parse-net-report-logs.mjs` only if phase 1 parser output cannot parse its own new fields
- no gameplay/client/server behavior files

## Implementation Checklist

- [ ] Verify beta is running the phase 1 build.
- [ ] Collect idle/no-command beta evidence.
- [ ] Collect normal-command beta evidence.
- [ ] Collect high-density command beta evidence.
- [ ] Preserve replay artifacts or record why no replay exists.
- [ ] Produce parser summaries for every window.
- [ ] Write neutral analysis with supported/contradicted/unknown sections.
- [ ] Mark this phase as done in this file in the implementation commit.

## Verification

- `node scripts/parse-net-report-logs.mjs --out-dir <incident-dir>/parsed <logs.jsonl>`
- `git diff --check`

If logs are unavailable because beta was not deployed, `FLY_API_TOKEN` is unavailable, or the user
cannot manufacture the reproduction, stop and report the exact blocker. Do not continue to phase 3
from stale evidence.

## Manual Test Focus

The user-facing manual work is the reproduction itself: idle baseline, normal commands, and
high-density command burst. Ask the user to note the absolute time, visible symptom, HUD `rtt`,
HUD `jit`, and whether movement prediction was enabled. Keep the request concrete and short so the
evidence can be aligned with logs.

## Handoff Expectations

Report the incident directory, preserved replay/log files, and the neutral conclusions. State
explicitly whether high command density correlated with snapshot jitter/gaps/bursts, frame stalls,
prediction health changes, reliable-message pressure, or server tick/scheduler pressure. List the
minimum local behavior that phase 3 must reproduce.
