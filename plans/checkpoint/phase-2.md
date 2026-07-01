# Phase 2 - File Round Trip Proof

Status: Not started.

## Scope

Implement the real versioned checkpoint file round trip for current authoritative game state:
`Game -> GameCheckpointV1 -> bytes/file -> Game`. This phase should convert every current
`GameState` field through explicit DTOs, validate before import, rebuild `DerivedState`, and reuse
the existing game-state semantic comparator to prove restored games continue exactly like the
baseline.

The persisted DTOs should be stable and intentionally shaped. Do not expose private struct layout
as schema just because a Rust type derives serde; use explicit conversion helpers so future
refactors can change private internals without breaking files.

Explicit non-goals:

- No normal match start, replay artifact, or lab scenario migration yet.
- No public endpoint, client UI, or wire protocol command for checkpoint files.
- No gameplay, balance, combat, fog, economy, or pathing policy changes.
- No cross-version migration beyond rejecting unsupported versions with clear errors unless Phase 1
  explicitly scoped a small migration.

## Expected Touch Points

- `server/crates/sim/src/game/state.rs` and a new or existing checkpoint module under
  `server/crates/sim/src/game/`: explicit DTOs, export/import, serde, validation, and error types.
- `server/crates/sim/src/game/mod.rs`: narrow public or crate-visible APIs for exporting and
  importing checkpoint files while keeping room callers on the existing `Game` seam.
- Entity, player, fog, memory, trench, smoke, ability runtime, shell, and command/order modules:
  narrow DTO conversion helpers only where the checkpoint module cannot otherwise serialize stable
  state.
- Existing checkpoint tests under `server/crates/sim/src/game/tests/`: extend the Phase 4-6
  comparator so it round-trips through actual serialized bytes or a temp file, not the cfg-test
  clone-shaped DTO.
- `docs/design/server-sim.md` and `docs/context/server-sim.md` if the implemented API differs from
  the Phase 1 contract.

Implementation should stay inside `rts-sim` unless test fixtures need temporary files under
`target/`.

## Verification

- Add tests for `Game -> checkpoint bytes -> Game` and `Game -> checkpoint file -> Game` using the
  existing movement/economy and visibility/combat/effects scenarios.
- Add canonical `checkpoint -> Game -> checkpoint` tests where Phase 1 requires stable normalized
  output.
- Add negative validation tests for unsupported version, duplicate ids, stale owner references,
  invalid map dimensions, out-of-bounds coordinates, invalid timers/counts, oversized queues, and
  corrupted/truncated files.
- Prove imported games rebuild derived state and do not serialize pathing cache/search entries.
- Prove stable entity id allocation, RNG continuity, pending commands, command logs, fog/memory,
  events after restore, and per-player/spectator/full-world snapshots remain equivalent.
- Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint_file
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check -- server/crates/sim/src/game docs/design/server-sim.md docs/context/server-sim.md plans/checkpoint
```

Use the closest narrow test filters if final names differ.

## Manual Testing Focus

No player-facing manual test is required. If useful, run a local dev or lab scenario, export a
checkpoint file through a test-only/debug helper, restore it in a focused command, and confirm the
same visible snapshot after several ticks.

## Handoff

The handoff must name:

- the checkpoint module/API shape and file format;
- every field family converted through explicit DTOs;
- validation errors and bounds added;
- how temp/golden files are handled;
- focused tests and archcheck that passed;
- remaining blockers before normal starts can be routed through checkpoints.
