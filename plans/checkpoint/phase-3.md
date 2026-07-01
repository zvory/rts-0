# Phase 3 - Checkpoint-Backed Starts

Status: Not started.

## Scope

Make normal match and non-scenario lab setup flows construct their initial authoritative state as a
start composition: the exact map plus a `GameCheckpointV1` payload bound to that map. Restore the
live `Game` from that composition. Public constructors and room callers should keep their current
signatures where practical, but their internals should exercise the same import path that later
replay and lab assets will use.

This phase should prove that tick-zero checkpoint starts are behaviorally identical to direct setup
for normal matches, replay-compatible starts, dev scenarios, and blank lab starts. Bundled/catalog
lab scenarios that currently restore from `LabScenarioV1` may keep using the existing scenario
restore path until Phase 5 adds the side-by-side scenario adapter. This phase should not change
committed replay artifact or lab scenario JSON shapes yet. Keep the direct setup path available as a
private/test-only oracle until this phase's parity tests prove the checkpoint-backed path is
equivalent; do not delete it merely because the live constructors have switched.

Explicit non-goals:

- No replay artifact schema migration.
- No lab catalog asset rewrite.
- No public checkpoint upload/download UI.
- No balance, faction loadout, map generation, or spawn policy changes.

## Expected Touch Points

- `server/crates/sim/src/game/setup.rs`: compile setup inputs into a map plus checkpoint DTOs and
  restore through the validated import path.
- `server/crates/sim/src/game/mod.rs`: preserve public constructor signatures or add narrow
  constructor overloads only where needed.
- `server/crates/sim/src/game/lab.rs` and setup/dev scenario helpers: route blank lab setup through
  checkpoint-backed construction where the setup inputs can be compiled directly to equivalent
  start state. Leave `LabScenarioV1` catalog/import adapter work to Phase 5 unless a very small
  internal shim is required for parity tests and does not change scenario JSON.
- Private/test-only direct setup helpers: retain enough of the old construction path to compare
  direct setup against checkpoint-backed setup throughout this phase.
- `server/src/lobby/launch.rs`, `server/src/lobby/room_task/**`, and `server/src/main.rs`: read-only
  unless constructor signatures force small call-site updates.
- Focused tests in sim setup, replay setup, and lab setup modules.
- Docs only if constructor behavior or checkpoint policy becomes part of the public `Game` seam.

## Verification

- Compare direct setup versus checkpoint-backed setup for normal matches with multiple players,
  teams, factions, authored maps, generated maps, and AI slots.
- Compare blank lab starts before and after checkpoint-backed construction. For catalog lab
  scenarios, verify they still launch through the existing `LabScenarioV1` path and record that
  checkpoint adapter parity belongs to Phase 5.
- Prove first snapshot and several post-start ticks match for each setup family.
- Prove a mismatched map identity/hash fails before constructing a live `Game`.
- Confirm replay artifact capture still records the same command log/start metadata as before for
  existing replay schema.
- Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint_start
cargo test --manifest-path server/Cargo.toml -p rts-sim setup
cargo test --manifest-path server/Cargo.toml -p rts-sim lab
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check -- server/crates/sim/src/game server/src/lobby docs/design/server-sim.md docs/context/server-sim.md plans/checkpoint
```

Use narrower filters if the implementation adds more precise names.

## Manual Testing Focus

Start one ordinary local match and one lab scenario. Confirm the first visible state, resources,
starting units/buildings, fog, lab controls, and first few commands behave as before.

## Handoff

The handoff must name:

- which constructors now go through checkpoint import;
- any setup paths deliberately left direct, with reason;
- how the private direct setup oracle is retained for future parity/debugging, or why it was safe to
  remove;
- how map-plus-payload start validation works;
- parity tests added;
- whether public constructor signatures changed;
- exact verification commands that passed;
- manual match/blank-lab smoke focus for Phase 4, plus confirmation that catalog lab scenarios were
  intentionally left for Phase 5 if not migrated.
