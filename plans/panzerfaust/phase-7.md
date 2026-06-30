# Phase 7 - Integration Regression And Balance Readiness

## Phase Status

Status: pending.

## Objective

Add final focused regression coverage and playtest scaffolding for the complete Panzerfaust feature.
This phase should harden the feature after normal production exposure rather than add new gameplay.

## Scope

- Build a focused test matrix for the final unit:
  - Production path, prerequisite, resources, supply, queue, cancel/refund, and rally spawn.
  - Direct Attack, Attack Move, Idle, Hold Position, Stop, queued commands, and Move non-autofire.
  - Windup cancellation, launched-shot consumption, projectile travel, target death, damage, and
    recovery.
  - Same-id conversion continuity for selection/control groups, HP, position, owner, queues, trench
    occupation, fog projection, and death cleanup.
  - Methamphetamines loaded movement/timing and post-conversion Rifleman behavior.
  - Entrenchment active range extension and defensive interaction while loaded.
  - Fog-safe events, replay visibility, spectator visibility, and lab/dev inspection.
  - AI non-training and AI-owned spawned target acquisition.
- Add or update dev scenarios for manual inspection:
  - One Panzerfaust versus one Tank.
  - Windup cancel and target death during travel.
  - Entrenched Panzerfaust range check.
  - Methamphetamines timing check.
- Avoid new tuning unless a regression exposes a clear mismatch with [checklist.md](checklist.md).
  If numbers feel wrong in playtest, record the observation as a follow-up instead of changing the
  approved spec inside this hardening phase.
- Update patch-note bullets with factual final behavior.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/tests/*.rs`
- `server/crates/sim/src/game/services/commands/tests/*.rs`
- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/ai/src/*.rs`
- `tests/client_contracts/*.mjs`
- `tests/server_integration.mjs`
- `tests/regression.mjs`
- `tests/ai_integration.mjs`
- `docs/design/testing.md`
- `plans/panzerfaust/checklist.md`

## Edge Cases To Cover

- A Panzerfaust killed during windup does not later fire or convert.
- A Panzerfaust killed during recovery does not convert after death.
- A converted Rifleman does not retain loaded-only range, target filter, visuals, or timers.
- Queued commands do not strand the converted Rifleman in an invalid attack order.
- Control groups and selection do not lose the unit because conversion preserves id.
- Fogged enemy clients do not receive hidden launch, impact, target id, conversion, or death
  position data.
- Reconnect, replay, spectator, and lab views do not require a different state shape from normal
  snapshots.
- Full integration still respects command unit-list caps and invalid inbound command handling.

## Verification

- Focused Rust tests for the full matrix above.
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default`
  if the changed tests are broad sim behavior tests or the phase touches many combat/order paths.
- Focused live Node suite with a running server if production or protocol behavior crosses the live
  client/server boundary.
- `node tests/regression.mjs` with a running server if hardening or invalid command behavior
  changes.
- `node tests/ai_integration.mjs` with a running server if AI/lobby behavior changes.
- `tests/run-all.sh --no-rust` if this phase touches client rendering/input/UI coverage.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  boundaries changed.
- `git diff --check`.

## Manual Test Focus

Use the dedicated dev scenarios and one normal tech-path match. Confirm production, one-shot firing,
cancellation, target death during travel, conversion, entrenched range, Methamphetamines timing, fog
projection, and replay/spectator readability.

## Handoff Expectations

List the final automated coverage added, manual scenarios used, and any playtest watch points. Tell
Phase 8 which docs, patch-note bullets, generated references, or deferred items still need cleanup.
