# Phase 7 - Timeline UI, Smoke, And Documentation

## Phase Status

- [x] Done.

## Objective

Expose the completed lab session-control model in the browser, harden coverage, and update
source-of-truth documentation.

## Work

- Make the browser render lab pause, resume, speed, step, and timeline seek from
  `StartPayload.capabilities.roomTime` and `roomTimeState`.
- Reuse or extract the existing replay/dev room-time controls so the shared portion has neutral names
  and replay-specific vision/branch behavior remains replay-owned.
- Keep lab UI app-owned and capability-driven. `Match`, HUD, input, minimap, renderer, and room-time
  controls should not infer behavior from URL mode or import `LabPanel`.
- Ensure per-operator lab vision status remains local while shared time status is visibly shared.
- Add or refresh client contracts for lab room-time controls, timeline rendering, seek clicks, step
  controls, and teardown.
- Add or refresh server/client protocol docs and context capsules for quickstart removal, per-user
  lab vision, and lab timeline semantics.
- Run a two-browser manual smoke covering: both operators join, each chooses different vision, one
  pauses/steps/resumes, one seeks the timeline, both see the same restored world, and both can still
  use lab setup tools afterward.
- Record remaining future work without expanding this plan: hand-authored presets, optional flags,
  durable public scenario libraries, branch-from-lab, and `/dev/scenario` migration.

## Expected Touch Points

- `client/src/replay_controls.js` or a new neutral room-time controls module
- `client/src/room_capabilities.js`
- `client/src/app.js`
- `client/src/match.js`
- `client/src/lab_panel.js`
- `client/styles.css`
- `tests/client_contracts/match_replay_contracts.mjs`
- `tests/client_contracts/lab_contracts.mjs`
- `tests/protocol_parity.mjs`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`
- `docs/context/protocol.md`
- `docs/context/client-ui.md`
- `docs/context/server-sim.md`
- `plans/lab/architecture.md`
- `plans/lab/session-controls/*.md`

## Verification

- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab_timeline`
- `git diff --check`

If the implementation changes visible browser behavior significantly, also run the smallest relevant
browser smoke or `tests/run-all.sh --only-browser` when local browser dependencies are available.

## Manual Test Focus

Run the two-browser lab smoke described above on a local server. The core acceptance check is that
vision is per operator while pause/speed/step/seek are shared room state, and lab operations still
work after a seek.

## Handoff Expectations

Summarize the completed player-facing behavior, list exact automated and manual verification, name
any intentionally preserved historical compatibility leftovers, and identify the next recommended
plan if the user wants hand-authored lab presets or branch-from-lab.
