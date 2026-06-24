# Phase 2 - Lab Mortar Regression Coverage

Status: done.

## Goal

Add a narrow end-to-end regression that proves the lab operator receives the server launch event
that drives the persistent mortar warning circle. This phase should validate the fixed behavior
through the same lab command path used by the browser without broadening the fix beyond event
projection.

## Scope

- Add a focused WebSocket/Node integration test, browser smoke check, or similarly narrow test that:
  - Joins a lab room.
  - Spawns or prepares a completed P2 Mortar Team and a valid target.
  - Issues `issueCommandAs(P2, useAbility(mortarFire, ...))`.
  - Asserts a decoded snapshot contains `mortarLaunch` before the later `mortarImpact`.
- If a browser-level check is practical without making the suite flaky, verify that the client
  stores a live mortar target after the launch event. Do not rely only on local command feedback,
  because the bug is specifically that command feedback can appear while the server launch event is
  missing.
- Keep the test targeted. It should not become a broad lab UI command-card, autocast research, or
  renderer visual matrix.
- Update test selection notes if a new test file or selector rule is introduced.
- Update documentation only if Phase 1 left a deliberate doc follow-up.

## Expected Touch Points

- `tests/*.mjs` for a targeted live WebSocket regression, or
  `tests/client_smoke.mjs` / `tests/client_contracts/*` if a browser/client-side assertion is the
  lower-risk option
- `tests/select-suites.mjs` only if adding the test requires selector metadata
- `docs/context/testing.md` only if the new test changes the testing map

## Constraints

- Do not re-open the server projection design unless Phase 1's implementation left a concrete gap.
- Do not assert on pixel-perfect rendering. The durable regression is that `mortarLaunch` reaches
  the client path that creates `mortarTargets`.
- Keep local server requirements clear. If the test needs a running server like the existing live
  Node suites, document that in the test header and use the existing `RTS_WS` convention.
- Avoid adding broad test time to default local workflows unless the regression is cheap enough and
  selector rules justify it.

## Verification

- Run the new targeted regression.
- Run any selector verification if a selector rule changes.
- If the test is live-server based, include the exact server command and test command in the PR
  verification note.

## Manual Testing Focus

- Open `/lab`, prepare a P2 mortar shot, and confirm the persistent ground warning circle appears
  for both manual fire and autocast.
- Switch lab vision between full-world and Team 2 and confirm P2 launch warnings remain visible in
  the authorized views.
- Sanity-check a normal live match or replay branch does not show unauthorized hidden mortar launch
  warnings.

## Handoff

After implementation, mark this phase done in this file and summarize the exact regression command,
what it proves, and any manual browser findings. If the regression could not be automated
reliably, explain the blocker and leave a precise manual test script.
