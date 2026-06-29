# Phase 2 - Persistent Trench State And Visibility

## Phase Status

Status: not started.

## Objective

Add server-owned persistent trench state and fog-safe projection without yet allowing units to make
or occupy trenches through normal play. This phase should make trenches a durable battlefield
object/state that can be snapshotted, replayed, inspected in dev/lab flows, and rendered later by
the client.

## Scope

- Add a trench store owned by `Game`, with deterministic ids and stable world-pixel positions.
- Keep trenches neutral. They should not consume supply, block building placement like structures,
  count for scoring, take damage, or participate in death cleanup unless a later requirement adds
  that behavior.
- Add minimal lab/dev or test-only mutation hooks if needed to seed trenches for projection and
  rendering tests.
- Project trenches through snapshots using the Phase 1 contract.
- Gate trench projection by recipient visibility in the same spirit as smoke, remembered buildings,
  and ability objects. Do not reveal hidden enemy units occupying or creating trenches.
- Decide and document whether trenches remain visible after discovery while fogged, or only while
  currently visible. If the implementation chooses remembered trench terrain, keep that memory
  terrain-only and do not expose current occupancy.
- Ensure replay, spectator, and dev full-world snapshots preserve the same authority model.
- Add client state storage for received trench snapshots, but keep final brown trench rendering for
  Phase 5 unless a minimal debug visualization is needed to test projection.
- Update `docs/design/protocol.md` and `docs/design/server-sim.md` for the new trench lifecycle and
  visibility contract.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/sim/src/game/lab.rs`
- `server/crates/sim/src/game/setup/dev_scenarios.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/compact_snapshot.rs`
- `client/src/state.js`
- `client/src/protocol_snapshot.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/projection-audit-checklist.md` as an audit checklist, if projection behavior is non-trivial

## Verification

- Focused Rust tests proving seeded trenches persist across ticks and appear in full-world
  snapshots.
- Fog projection tests proving a player does not receive a trench outside the chosen visibility
  policy.
- Replay or snapshot representative tests covering the trench field.
- `node tests/protocol_parity.mjs` and `node tests/client_contracts/protocol_contracts.mjs` if the
  compact or JSON wire shape changes in this phase.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if the
  trench store introduces new sim module boundaries.
- `git diff --check`.

## Manual Test Focus

Use a dev or lab scenario with seeded trenches to confirm each player sees only the trench terrain
they are allowed to see. Check spectator or full-world views to confirm trenches persist and remain
neutral.

## Handoff Expectations

Describe the trench store API, projection visibility rule, and any lab/dev seeding mechanism. Tell
the Phase 3 agent how to create a trench from the sim without bypassing id allocation, visibility,
or replay assumptions.
