# Phase 4 - Cold Checkpoint V0

Status: Done.

## Scope

After Phase 3 has landed the private `GameState` aggregate and Phase 2's `DerivedState` shell is
still separate, add the first cold export/import proof for authoritative game state. This phase is a
behavior-preserving internal checkpoint pass: export a crate-private or test-friendly
`GameCheckpoint` from `GameState`, import it into a fresh `GameState`, rebuild `DerivedState` from
that imported state, tick both games forward under the same command stream, and prove they remain
semantically equivalent.

This is the first `Game -> GameCheckpoint -> Game` proof, not the final checkpoint product. The
checkpoint type may be an internal Rust DTO and should not be treated as stable JSON, a wire
protocol, a replay artifact schema, or a lab scenario schema. It must not use
`Game::clone_for_replay_keyframe` or any equivalent direct full-`Game` clone as the restore
mechanism; the point is to make the durable `GameState` export/import boundary explicit.

Keep the first scenario deliberately small. Use a simple non-AI game with deterministic setup and a
short command/tick stream that exercises ordinary state enough to prove the plumbing: stable entity
ids, allocator/high-water state, player state, tick state, command application, fog/memory
projection, and snapshot equivalence after additional ticks. Later phases should expand coverage
for every durable subsystem; this phase should establish the reusable cold-restore path and reuse
the Phase 0.5 semantic comparator/harness instead of inventing a separate proof mechanism.

Preconditions:

- Phase 1's ownership registry has no unresolved blocker for any field currently under
  `GameState`.
- Phase 2's `DerivedState` clear/rebuild seam exists and is covered by the Phase 0.5 comparator.
- Phase 3's private `GameState` aggregate has landed, and `GameState` owns the Phase 1
  `authoritative/serialized` and `compatibility metadata` fields.

If any precondition is false, stop and repair the earlier phase result before adding checkpoint
export/import.

Explicit non-goals:

- No public `Game` API, wire protocol, server endpoint, client, snapshot DTO, or start-payload
  change.
- No final checkpoint format, JSON schema, versioning policy, migration policy, or persisted file
  format commitment.
- No replacement of clone-based replay seek keyframes, replay artifact capture/playback, lab
  timeline keyframes, or lab scenario import/export.
- No migration of lab scenarios to checkpoints and no change to lab scenario id-remap behavior.
- No broad coverage of every durable subsystem; later phases should expand movement, production,
  combat, projectiles, smoke, abilities, trenches, building memory, lab god mode, and other
  subsystem coverage.
- No AI decision determinism promise. Use deterministic non-AI command streams for this first proof;
  AI controller memory remains outside the checkpoint contract.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`: add only private/crate-private checkpoint entry points or
  helpers needed to construct a `Game` from an imported `GameState` plus rebuilt `DerivedState`;
  keep public `Game` API signatures stable.
- `server/crates/sim/src/game/state.rs` or the Phase 3 equivalent private state module: define the
  internal `GameCheckpoint` DTO and `GameState` export/import helpers. Export every current
  `GameState` field or stop if the Phase 1 registry does not provide a checkpoint policy.
- `server/crates/sim/src/game/setup.rs`: share construction or repair helpers only if needed to
  rebuild `DerivedState` and maintain initial invariants after checkpoint import.
- `server/crates/sim/src/game/snapshot.rs`: no behavior change expected; use per-player
  fog-filtered snapshots as verification evidence.
- `server/crates/sim/src/game/replay.rs`: read-only compatibility check unless tests need to prove
  clone-based replay keyframes and replay artifacts still use their existing paths.
- `server/crates/sim/src/game/lab.rs`: read-only compatibility check unless tests need to prove lab
  scenario import/export still use their existing path.
- Focused tests under `server/crates/sim/src/game/**`, preferably alongside the Phase 0.5
  comparator/harness so cold checkpoint restore and derived-state wipe/rebuild share the same
  semantic comparison code.
- `docs/design/server-sim.md` and `docs/context/server-sim.md` only if the internal checkpoint seam
  becomes part of the documented simulation contract or section references shift.
- `plans/game-state/phase-4.md`: mark complete only in the implementation commit that lands this
  phase.

Implementation Rust/JS outside `rts-sim::game` should be treated as out of scope unless compiler
errors prove a private helper must move. Client code, protocol crates, replay artifact schema, lab
scenario schema, and room/session code should not need changes.

## Verification

- Extend the Phase 0.5 comparator/harness to support a cold checkpoint leg:
  `baseline Game` continues normally, while `restored Game` is built by exporting
  `GameCheckpoint` from `GameState`, importing a fresh `GameState`, rebuilding `DerivedState`, then
  ticking forward under the same subsequent commands.
- Compare semantic authoritative state rather than raw struct bytes. The comparison must include
  all currently exported `GameState` fields or a canonical semantic view of them, and it must ignore
  only fields Phase 1 classified as derived or transient.
- Compare per-player fog-filtered snapshots after additional ticks for every player in the small
  scenario. Include stable entity ids and fog-gated projection details; do not rely only on a
  full-world snapshot.
- Prove stable ids and allocator/high-water state survive export/import. Prefer a focused assertion
  that the same post-restore action allocates the same next id in both baseline and restored games,
  or an equivalent internal allocator-state comparison if that is cleaner.
- Prove `DerivedState` is rebuilt after import, not serialized as authoritative state. The test
  should fail if final spatial state, pathing cache/search bookkeeping, or other Phase 2 derived
  state is missing its rebuild path.
- Confirm `Game::clone_for_replay_keyframe`, replay artifact capture/playback, lab timeline
  keyframes, and lab scenario import/export are not replaced or routed through `GameCheckpoint`.
- Run the narrowest focused Rust tests that cover the cold checkpoint proof and the reused
  derived-state comparator. Suggested commands:

```bash
cargo fmt --manifest-path server/Cargo.toml
cargo test --manifest-path server/Cargo.toml -p rts-sim checkpoint
cargo test --manifest-path server/Cargo.toml -p rts-sim derived_state
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
git diff --check -- server/crates/sim/src/game docs/design/server-sim.md docs/context/server-sim.md plans/game-state/phase-4.md plans/game-state/plan.md
```

If the final test names do not include `checkpoint` or `derived_state`, use the closest narrow
filters that cover export/import, derived-state rebuild, semantic state comparison, per-player
fog-filtered snapshots, and stable id/allocator preservation. No broad Node suite or full local test
bundle is expected unless implementation changes escape the sim crate or alter public
protocol-facing behavior; the PR `./tests/run-all.sh` gate remains the authoritative full-suite
check.

## Manual Testing Focus

No broad manual gameplay pass is expected because this phase should add no public or UI-visible
checkpoint behavior. If a manual check is useful, run one ordinary local two-player or dev scenario,
issue a few movement/economy commands, and confirm visible gameplay, fog-filtered snapshots, replay
capture, and lab scenario import/export still behave as before. There should be no browser control,
server route, replay option, or lab option that exposes `GameCheckpoint`.

## Handoff

The implementation handoff must name:

- the final internal `GameCheckpoint` shape, visibility, and the module where export/import lives;
- every `GameState` field exported/imported, plus any field intentionally excluded with its Phase 1
  checkpoint policy;
- how import rebuilds `DerivedState`, including final spatial state and pathing cache/search
  bookkeeping;
- how the reused Phase 0.5 comparator performs semantic comparison and per-player fog-filtered
  snapshot comparison after additional ticks;
- how stable ids and allocator/high-water state were preserved and tested;
- confirmation that public APIs, wire protocol, client code, replay keyframes/artifacts, lab
  timeline keyframes, and lab scenario import/export did not change;
- the exact focused Rust test commands, archcheck command, and `git diff --check` command that
  passed;
- the remaining subsystem coverage gaps that later checkpoint phases should add before any public
  checkpoint format, replay migration, or lab migration is considered.
