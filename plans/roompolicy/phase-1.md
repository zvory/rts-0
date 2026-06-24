# Phase 1 - Internal And Client Names

Status: planned.

## Goal

Remove product-use-case names from internal room policy and shared client room-time controls where
the behavior is already capability-driven. This phase must not change any wire tags, serialized
field names, gameplay behavior, projection behavior, or UI affordances.

## Scope

- Rename server policy names that describe shared capabilities rather than product sources:
  - `ObserverAnalysisPolicy::LiveSpectators` to a spectator-scoped name.
  - `ObserverAnalysisPolicy::ReplayViewers` to an all-recipient or selected-recipient analysis
    name.
  - `DiagnosticPolicy::LIVE_SPECTATOR_OBSERVER_ANALYSIS` and
    `DiagnosticPolicy::REPLAY_OBSERVER_ANALYSIS` to audience/capability names.
  - `DiagnosticPolicy::DEV_MOVEMENT_PATHS` to a movement-path capability name.
  - `VisibilityPolicy::ReplayVision`, `DevFullWorld`, and `LabFullWorld` to projection/capability
    names such as selected perspective and full-world projection. Collapse equivalent full-world
    policy variants only if the call sites remain clearer after the rename.
- Rename helper/predicate names in `session_policy.rs`, `projection.rs`, and room-task call sites
  when they now refer to capabilities rather than replay/dev/lab identity.
- Rename client room-time control DOM handles, CSS classes, and methods that are now shared:
  - `dom.replaySpeed` toward `dom.roomTimeControls`.
  - `.dev-pause-btn` / `.dev-step-btn` toward room-time pause/step names.
  - `.replay-timeline`, `.replay-tick-status`, and related helper names toward room-time names
    while preserving any temporary CSS aliases needed to avoid a noisy styling rewrite.
- Keep `ReplayControls extends RoomTimeControls` as a temporary exported alias if any existing
  import or test still uses it; otherwise remove it only when the import graph is clean.
- Update `docs/design/server-sim.md` and `docs/design/client-ui.md` wording for the renamed policy
  and client control surfaces.

## Touch Points

- `server/src/lobby/session_policy.rs`
- `server/src/lobby/projection.rs`
- `server/src/lobby/room_task/*.rs`
- `client/src/bootstrap.js`
- `client/src/replay_controls.js`
- `client/src/room_capabilities.js`, only if parser-local naming needs a no-wire-change cleanup
- `client/index.html`
- `client/styles.css`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`

## Constraints

- Do not change `server/crates/contract/src/lib.rs` serialized fields or
  `server/crates/protocol/src/lib.rs` message tags in this phase.
- Do not change `client/src/protocol_constants.js` or protocol builder names except comments that
  remain true under the existing wire names.
- Do not infer room controls from replay/dev/lab identity. Controls must still render from
  `startPayload.capabilities` and `startPayload.diagnostics`.
- Preserve the existing room-time localStorage key unless there is a compelling reason to migrate
  it; a storage-key cleanup can wait.

## Verification

- `cargo test --manifest-path server/Cargo.toml session_policy`
- `cargo test --manifest-path server/Cargo.toml projection_policy`
- `node scripts/check-client-architecture.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Testing Focus

Start one replay or replay artifact and confirm room-time speed, pause, seek, timeline, replay
vision, and branch button still appear as before. Start an AI-only live match and confirm only
speed/pause room-time controls appear. Open a lab room and confirm pause, step, seek/timeline, and
lab panel controls still work.

## Handoff

Mark this phase done only after committing the internal/client rename. Summarize the final renamed
policy terms, any aliases intentionally left behind, verification run, manual testing performed or
skipped, and whether Phase 2 can safely rename the public protocol surface.
