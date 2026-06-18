# Phase 4 - Projection And Diagnostics Contract

## Phase Status

- [ ] Pending.

## Objective

Move diagnostic data such as movement paths behind room projection and diagnostic policy. The room
should decide what diagnostics a recipient may receive; `Game` should provide diagnostic facts only
when asked through neutral snapshot options.

## Work

- Add neutral snapshot/projection options to the public `Game` snapshot API as needed, keeping the
  `Game` seam documented and panic-free.
- Stop using durable product-mode state such as `debug_path_overlays`, `StartingLoadout::DebugHuman`,
  or dev scenario identity as the direct source of whether debug path fields are included.
- Extend projection policy so it owns both visibility and diagnostics for each recipient. Preserve
  current data scope: ordinary Debug-mode movement paths remain owner-scoped, dev scenarios may show
  full-world diagnostics where current behavior does, and lab diagnostics are enabled only if policy
  says so.
- Replace start-payload `debugMode` with explicit capability metadata. Backwards compatibility is not
  required, so remove `debugMode` from protocol docs, Rust DTOs, and JavaScript state once the new
  field exists.
- Update client state, settings, and renderer feedback to use diagnostic capability metadata instead
  of `debugMode` or `devWatch.kind`.
- Update protocol docs, server-sim docs, and tests.

## Expected Touch Points

- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/setup.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/src/protocol.rs`
- `server/src/lobby/projection.rs`
- `server/src/lobby/launch.rs`
- `server/src/lobby/room_task.rs`
- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/match.js`
- `client/src/settings_panels.js`
- `client/src/renderer/feedback_view_model.js`
- `client/src/renderer/feedback.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/context/protocol.md`
- `docs/context/server-sim.md`
- `plans/lab/room2/phase-4.md`

## Implementation Checklist

- [ ] Add neutral snapshot options or projection options for diagnostics.
- [ ] Move debug-path inclusion decisions out of starting loadout and into projection policy.
- [ ] Replace `debugMode` with explicit diagnostic capability metadata.
- [ ] Update client debug settings and renderer feedback to consume the new metadata.
- [ ] Add focused tests for owner-only diagnostics, dev full-world diagnostics, and lab/non-lab
      diagnostic classification.
- [ ] Mark this phase as done in this file.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server projection`
- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

## Manual Test Focus

Start a Debug-mode match and verify movement waypoints still show for owned moving units only. Open a
dev scenario and verify the expected diagnostic overlay behavior. Open a lab and verify diagnostic
controls appear only when the start payload advertises them.

## Handoff Expectations

Describe the final diagnostic capability shape, the `Game` snapshot API change, the removed
`debugMode` or loadout coupling, and any remaining diagnostic behavior that is intentionally
mode-local.
