# Phase 4 - Replay Artifact Migration

Status: Not started.

## Scope

Introduce a replay artifact version whose start state is a map binding plus a tick-zero
`GameCheckpointV1` plus the recorded authoritative command stream. New replay captures should save
the tick-zero map-plus-checkpoint composition at match launch, attach duration, final scores,
winners, and the authoritative command stream at match end, and use the checkpoint-backed artifact
while old replay artifacts remain loadable through compatibility code or fail with an intentional,
documented incompatibility reason. Do not try to derive the replay start checkpoint from the final
post-match `Game`; by then the authoritative state is no longer the replay start state.

Replay playback must remain authoritative through recorded actions. AI controller memory stays
outside the checkpoint; AI slots are restored as players, and replay correctness comes from the
captured command/action stream.

Explicit non-goals:

- No lab operation action stream yet unless a small adapter is required for existing replay tests.
- No lab scenario catalog migration.
- No client protocol change except surfaced replay incompatibility text if already routed through
  existing server messages.
- No deletion of old replay code until compatibility and fallback behavior are proven.
- No standalone checkpoint document product surface; the checkpoint payload is embedded inside the
  replay artifact.

## Expected Touch Points

- `server/crates/sim/src/game/replay.rs`: add the checkpoint-backed replay artifact version,
  validation, capture, load helpers, and a versioned enum or equivalent decoder that can distinguish
  legacy artifacts from checkpoint-backed artifacts.
- `server/src/lobby/replay_session.rs`, `dev_replay.rs`, `crash_replay.rs`,
  `room_task/lifecycle.rs`, `room_task/replay.rs`, and match-history helpers: route new captures and
  playback through the checkpoint start path. Add room/lifecycle storage for the launch-time
  replay start checkpoint or equivalent start composition, then finalize the replay artifact with
  end-of-match command log, duration, winner, and scores.
- `server/src/db.rs`: update persisted replay artifact handling to decode the same versioned replay
  artifact contract used by file/dev loading. Database rows must not deserialize directly into only
  the old concrete replay type once a new artifact shape exists.
- Replay tests under `server/src/lobby/**` and `server/crates/sim/src/game/**`.
- Docs for replay artifact compatibility and migration behavior.

## Verification

- New captures produce checkpoint-backed replay artifacts and replay to the same final state as the
  live game.
- New captures preserve the tick-zero start checkpoint captured at launch; tests should fail if the
  artifact start checkpoint is accidentally exported from the final post-match `Game`.
- Existing saved/dev replay artifacts still launch if compatibility is retained; if not retained,
  failures are explicit and covered by tests.
- Match-history database replay rows use the same versioned compatibility or rejection policy as
  dev/self-play/crash replay files.
- The replay artifact's map binding rejects playback against the wrong map identity/hash before a
  live `Game` is constructed.
- Replay seek, branch seed, selected vision, spectator vision, crash replay capture, and match
  history replay launch keep their documented behavior.
- Command timing has no off-by-one drift around the start checkpoint tick.
- The replay start checkpoint has the intended tick, pending-command, and command-log state for the
  chosen convention, and the first recorded command is applied on the same simulation tick as before.
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
- how the artifact embeds `GameCheckpointV1` and map binding data;
- old artifact compatibility or rejection policy;
- every replay load surface audited: dev/self-play files, crash replay artifacts, match-history DB
  rows, and committed fixtures;
- capture/playback paths changed;
- where the launch-time replay start checkpoint is stored before match end, and how artifact
  finalization combines it with command log, duration, winner, and scores;
- command timing convention at checkpoint start;
- focused tests that passed;
- manual replay smoke focus for Phase 5.
