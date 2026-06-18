# Phase 5 - Setup Tools and Control Policy

## Phase Status

- [ ] Not started.

## Objective

Make the lab useful for scenario setup by adding operator tools and explicit omnipotent control
policy on the client while preserving server-authoritative validation.

## Work

- Implement `LabControlPolicy` for inspection, selection, setup operations, and gameplay issue-as
  behavior.
- Allow the operator to select and inspect any non-neutral visible entity in lab mode. Do not fake
  `state.playerId`; make the policy explicit.
- Allow mixed-owner selections for inspection and batch setup operations such as delete, move, and
  owner reassignment.
- Reject mixed-owner gameplay commands by default in the client and on the server. A later phase can
  partition mixed selections into one command per owner if that UX is worth adding.
- Add lab panel controls for spawning existing unit/building kinds, deleting selected entities,
  moving selected entities, changing owner, setting player resources, and setting completed
  research.
- Add a clear issue-as flow for single-owner selections that sends real gameplay commands through
  the lab protocol and normal command validation.
- Keep normal HUD command cards for real gameplay commands where they remain authentic. Lab-only
  setup actions belong in lab panels or lab action bars.
- Add feedback for rejected lab operations without guessing client-side success.
- Add client and server tests around selection policy, mixed-owner handling, stale ids, invalid
  inputs, issue-as routing, and operation result display.

## Expected Touch Points

- `client/src/lab_control_policy.js`
- `client/src/lab_panel.js`
- `client/src/lab_client.js`
- `client/src/match.js`
- `client/src/state.js`
- `client/src/input/*`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/minimap.js`
- `server/src/lobby/room_task.rs`
- `server/crates/sim/src/game/lab.rs`
- `tests/client_contracts.mjs`
- focused client input/HUD tests as needed
- `server/src/lobby/tests.rs`

## Implementation Checklist

- [ ] Add explicit control policy methods for lab inspect/select/setup/issue-as decisions.
- [ ] Inject the policy into input, HUD, minimap, and command issuer seams without broad
      cross-area imports.
- [ ] Add spawn controls with kind, owner, and world-position selection.
- [ ] Add delete, move, and set-owner controls for selected entities.
- [ ] Add resource and completed-research controls per player.
- [ ] Add single-owner issue-as command flow and server-side rejection for mixed-owner gameplay
      commands.
- [ ] Add user-visible result/error feedback for lab operations.
- [ ] Add focused tests for policy decisions and operation UI plumbing.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim lab`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- focused input/HUD/minimap tests touched by this phase
- `git diff --check`

## Manual Test Focus

Open `/lab`, spawn units for both teams, select enemy and allied units, move and delete selected
entities, reassign ownership, set resources/research, issue a move or attack command as one owner,
and confirm mixed-owner gameplay commands are rejected clearly.

## Handoff Expectations

Explain the final client control-policy shape, which modules consume it, and the exact server-side
issue-as enforcement. List any setup operations that are still awkward or developer-oriented before
scenario import/export lands.
