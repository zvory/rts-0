# Phase 2 - Payload Round Trip Proof

Status: Done.

## Scope

Implement the real versioned checkpoint payload round trip for current authoritative game state:
`Game + exact supplied map -> GameCheckpointV1 -> text bytes -> exact supplied map -> Game`. This
phase should convert every current checkpointed state field through explicit DTOs, except full map
data when Phase 1 classifies it as container-owned, validate before import, rebuild `DerivedState`,
and reuse the existing game-state semantic comparator to prove restored games continue exactly like
the baseline.

The persisted DTOs should be stable and intentionally shaped. Do not expose private struct layout
as schema just because a Rust type derives serde; use explicit conversion helpers so future
refactors can change private internals without breaking embedded payloads. The checkpoint payload
is not a standalone product file format; any disk round trip in this phase should be a test/debug
document wrapper around the canonical payload.

Explicit non-goals:

- No normal match start, replay artifact, or lab scenario migration yet.
- No public endpoint, client UI, or wire protocol command for checkpoint payloads.
- No gameplay, balance, combat, fog, economy, or pathing policy changes.
- No cross-version migration beyond rejecting unsupported versions with clear errors unless Phase 1
  explicitly scoped a small migration.

## Expected Touch Points

- `server/crates/sim/src/game/state.rs` and a new or existing checkpoint module under
  `server/crates/sim/src/game/`: explicit DTOs, export/import, serde, validation, and error types.
- `server/crates/sim/src/game/mod.rs`: narrow public or crate-visible APIs for exporting and
  importing checkpoint payloads while keeping room callers on the existing `Game` seam.
- Map/setup helpers: pass the exact map supplied by the caller or test/debug document into import,
  validate it against the checkpoint payload's map binding before constructing the live `Game`, and
  construct the live `GameState.map` from that supplied map rather than a duplicated map body inside
  the payload.
- Entity, player, fog, memory, trench, smoke, ability runtime, shell, and command/order modules:
  narrow DTO conversion helpers only where the checkpoint module cannot otherwise serialize stable
  state.
- Existing checkpoint tests under `server/crates/sim/src/game/tests/`: extend the Phase 4-6
  comparator so it round-trips through actual serialized text bytes and, where useful, a temp
  debug document, not the cfg-test clone-shaped DTO.
- `docs/design/server-sim.md` and `docs/context/server-sim.md` if the implemented API differs from
  the Phase 1 contract.

Implementation should stay inside `rts-sim` unless test fixtures need temporary files under
`target/`.

## Verification

- Add tests for `Game -> checkpoint text bytes -> Game` and, if a debug wrapper exists,
  `Game -> debug document -> Game` using the existing movement/economy and
  visibility/combat/effects scenarios.
- Add canonical `checkpoint -> Game -> checkpoint` tests where Phase 1 requires stable normalized
  output.
- Add negative validation tests for unsupported version, duplicate ids, stale owner references,
  wrong map identity/hash, invalid map dimensions, out-of-bounds coordinates, invalid
  timers/counts, oversized queues, oversized payloads, and corrupted/truncated text.
- Prove imported games rebuild derived state and do not serialize pathing cache/search entries.
- Prove stable entity id allocation, RNG continuity, pending commands, command logs, fog/memory,
  events after restore, and per-player/spectator/full-world snapshots remain equivalent.
- Prove entity-local active orders, queued order intents, selected movement paths/waypoints/path
  goals, active and pending smoke, scheduled mortar/artillery impacts, ability runtime
  projectiles/world objects, and a representative sustained artillery order such as Point Fire or
  Blanket Fire survive payload import and continue equivalently. The representative artillery order
  does not need to cover both Point Fire and Blanket Fire unless implementation evidence shows their
  DTO paths diverge.
- Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint_payload
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check -- server/crates/sim/src/game docs/design/server-sim.md docs/context/server-sim.md plans/checkpoint
```

Use the closest narrow test filters if final names differ.

## Manual Testing Focus

No player-facing manual test is required. If useful, run a local dev or lab scenario, export a
checkpoint payload through a test-only/debug helper, restore it in a focused command with the
matching map, and confirm the same visible snapshot after several ticks.

## Handoff

The handoff must name:

- the checkpoint module/API shape, payload schema, and any test/debug document wrapper;
- how map data is supplied by callers and validated against the payload binding;
- every field family converted through explicit DTOs;
- explicit coverage for queued orders, active paths, smoke, delayed shell/projectile stores, ability
  runtime state, and the representative sustained artillery order tested;
- validation errors and bounds added;
- how temp/golden payloads or debug documents are handled;
- focused tests and archcheck that passed;
- remaining blockers before normal starts can be routed through checkpoints.
