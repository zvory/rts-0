# Phase 4 - Replay Artifact Migration

Status: Not started.

## Scope

Introduce a replay artifact version whose start state is `GameCheckpointV1` plus the recorded
authoritative command stream. New replay captures should use the checkpoint-backed artifact while
old replay artifacts remain loadable through compatibility code or fail with an intentional,
documented incompatibility reason.

Replay playback must remain authoritative through recorded actions. AI controller memory stays
outside the checkpoint; AI slots are restored as players, and replay correctness comes from the
captured command/action stream.

Explicit non-goals:

- No lab operation action stream yet unless a small adapter is required for existing replay tests.
- No lab scenario catalog migration.
- No client protocol change except surfaced replay incompatibility text if already routed through
  existing server messages.
- No deletion of old replay code until compatibility and fallback behavior are proven.

## Expected Touch Points

- `server/crates/sim/src/game/replay.rs`: add the checkpoint-backed replay artifact version,
  validation, capture, and load helpers.
- `server/src/lobby/replay_session.rs`, `dev_replay.rs`, `crash_replay.rs`,
  `room_task/lifecycle.rs`, `room_task/replay.rs`, and match-history helpers: route new captures and
  playback through the checkpoint start path.
- `server/src/db.rs`: update persisted replay artifact handling only if the stored JSON shape must
  distinguish versions.
- Replay tests under `server/src/lobby/**` and `server/crates/sim/src/game/**`.
- Docs for replay artifact compatibility and migration behavior.

## Verification

- New captures produce checkpoint-backed replay artifacts and replay to the same final state as the
  live game.
- Existing saved/dev replay artifacts still launch if compatibility is retained; if not retained,
  failures are explicit and covered by tests.
- Replay seek, branch seed, selected vision, spectator vision, crash replay capture, and match
  history replay launch keep their documented behavior.
- Command timing has no off-by-one drift around the start checkpoint tick.
- Suggested focused commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim replay
cargo test --manifest-path server/Cargo.toml -p rts-server replay
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
node scripts/check-crate-boundaries.mjs
git diff --check -- server/crates/sim/src/game server/src/lobby server/src/db.rs docs plans/checkpoint
```

If there is no `rts-server` replay filter, use the narrowest server/lobby replay test filters that
cover launch, seek, branch, and match-history capture.

## Manual Testing Focus

Capture a short local match replay, launch it from the saved/dev replay flow, seek through it, and
verify selected-player and spectator views still behave. Also try one old replay artifact if a
fixture or saved artifact exists for the compatibility path.

## Handoff

The handoff must name:

- the new replay artifact version and JSON shape;
- old artifact compatibility or rejection policy;
- capture/playback paths changed;
- command timing convention at checkpoint start;
- focused tests that passed;
- manual replay smoke focus for Phase 5.
