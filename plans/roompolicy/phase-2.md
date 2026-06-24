# Phase 2 - Protocol Capability Names

Status: done.

## Goal

Rename public protocol capability and message names that still describe shared room affordances as
replay-only features. This phase is a synchronized current-protocol rename across Rust, JS, docs,
and focused tests.

## Scope

- Rename observer-analysis delivery from replay-specific to observer-specific naming:
  - Server message tag `replayAnalysis` to `observerAnalysis`.
  - Client constants/listeners from `REPLAY_ANALYSIS` to `OBSERVER_ANALYSIS`.
  - Any remaining docs/comments that describe the payload as replay-only.
- Rename selectable vision/perspective capability and command names:
  - `VisibilityCapabilities.replay_vision` / `capabilities.visibility.replayVision` to a neutral
    name such as `visionSelection`.
  - `SetReplayVision`, `ReplayVisionRequest`, JS `setReplayVision`, and related client helpers to
    the same neutral concept.
  - Keep the behavior the same: the request still selects which players' fog/perspective a viewer
    uses in rooms that advertise the capability.
- Rename replay-branch action capability and creation tags where they describe a reusable
  branch-from-current-tick affordance:
  - `ActionCapabilities.replay_branch` / `actions.replayBranch` to a neutral name such as
    `branchFromTick` or `forkFromTick`.
  - `requestReplayBranch` and `replayBranchCreated` to matching branch/fork-from-tick protocol
    names if the implementation can do so as a direct mirrored rename.
  - Keep branch staging and replay artifact/source metadata names where they describe actual replay
    source data rather than the generic action.
- Update the protocol design doc, protocol capsule wording if needed, and client-ui/server-sim docs
  that name the old public fields.
- Update focused tests that assert protocol tags, start payload capabilities, and client capability
  parsing.

## Touch Points

- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/crates/protocol/src/contract_metadata.rs`, if message-tag metadata changes there
- `server/src/main.rs`, if message tag name tests or helpers assert the old name
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/launch.rs`
- `server/src/lobby/room_task/replay.rs`
- `server/src/lobby/room_task/branch.rs`, only for request/created tag renames
- `client/src/protocol_constants.js`
- `client/src/protocol.js`
- `client/src/net.js`
- `client/src/app.js`
- `client/src/match.js`
- `client/src/replay_controls.js`
- `client/src/room_capabilities.js`
- `client/src/observer_analysis_overlay.js`, only for event/tag naming
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- Protocol parity and room-task tests that assert the old capability/tag names

## Constraints

- Keep all semantics unchanged: same recipients, same fog selection rules, same room-time state,
  same branch seed, same branch staging flow, and same observer-analysis payload body.
- Prefer direct mirrored renames over compatibility aliases. If a concrete stale-client/deploy-order
  issue appears, stop and report the needed compatibility choice instead of building a broad
  migration system.
- Do not rename `ReplayStartMetadata`, replay artifact schema fields, `RoomMode::Replay*`, lab
  metadata, or branch staging domain types unless the old name is part of the public capability
  surface being changed.
- Update Rust and JS mirrors in one commit; do not leave mixed old/new protocol vocabulary.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- Focused room-task tests for replay playback, live spectator observer analysis, AI-only live
  room-time capabilities, and branch creation. Use exact `cargo test --manifest-path
  server/Cargo.toml <filter>` filters discovered during implementation.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Testing Focus

Start or load a replay and confirm observer analysis, vision selection, timeline controls, and
branch-from-tick creation still work. Join a live match as a spectator and confirm observer
analysis still arrives. Confirm a normal active live player does not receive replay/observer-only
affordances.

## Handoff

Mark this phase done only after committing the protocol rename. Summarize each public old name and
new name, verification run, manual testing performed or skipped, any compatibility aliases left
behind, and any product-specific names intentionally kept because they represent real replay/lab
source data.
