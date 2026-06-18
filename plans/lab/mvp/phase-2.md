# Phase 2 - Authoritative Lab Game API

## Phase Status

- [ ] Not started.

## Objective

Add narrow, typed `Game` APIs for lab mutations while keeping all authoritative validation and
derived-state repair inside the simulation crate.

## Work

- Add a sim-owned lab module, for example `server/crates/sim/src/game/lab.rs`, that exposes a small
  public API rather than a generic debug backdoor.
- Define typed lab operation inputs and results for spawn, delete, move entities, set owner, set
  player resources, set completed research, scenario export, and scenario restore.
- Validate entity kinds, owners, player ids, research ids, coordinates, map bounds, collision and
  placement rules, construction state, and stale ids before mutating state.
- Apply accepted mutations through helpers near the owning system when needed. Avoid one giant
  function that casually rewrites unrelated fields.
- Recompute or repair derived state after mutations: supply, fog, spatial index, building memory,
  resource reservations, command targets, queued orders, and construction/production state where
  affected.
- Keep `Game::tick()` panic-free. Bad lab input should return structured errors or intentional
  no-op results, never `unwrap`, `expect`, or unchecked indexing on the tick path.
- Add scenario export/restore foundations for the authoritative JSON format, but keep browser and
  protocol wiring for Phase 6 unless a tiny internal round trip is needed for tests.
- Add focused unit tests for every accepted and rejected operation.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- `server/crates/sim/src/game/lab.rs`
- `server/crates/sim/src/game/entity/*`
- `server/crates/sim/src/game/player_state.rs`
- `server/crates/sim/src/game/services/*`
- `server/crates/sim/src/game/snapshot.rs`
- `server/crates/sim/src/game/setup.rs`
- `server/crates/rules/src/*` only if a reusable rule helper is needed
- `docs/design/server-sim.md`
- `docs/context/server-sim.md`

## Implementation Checklist

- [ ] Add `Game::new_lab` or an equivalent narrow public constructor around real map/player setup.
- [ ] Add `Game::apply_lab_op` with typed results and errors.
- [ ] Add `Game::issue_lab_command_as` only if normal validation can remain authoritative.
- [ ] Add entity spawn/delete/move/owner APIs with map and collision validation.
- [ ] Add resource and research mutation APIs with player and upgrade validation.
- [ ] Add internal scenario export/restore types with schema versioning.
- [ ] Repair derived state after every accepted mutation.
- [ ] Add tests for stale ids, invalid owners, invalid kinds, invalid upgrades, invalid positions,
      occupied placement, supply recomputation, fog recomputation, and snapshot visibility.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim snapshot`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim supply`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

If a filtered command matches zero tests, add exact lab/sim tests before claiming verification.

## Manual Test Focus

No browser manual test is required for this phase unless the implementation adds temporary debug
exposure. The important manual review is code-level: confirm lobby/server code still cannot mutate
entity stores or player state directly.

## Handoff Expectations

Describe the public `Game` lab API, the accepted operation set, and the derived-state repair path.
List any intentionally deferred edge cases, such as impossible placement flags, partial
construction editing, ability runtime editing, or full replay checkpoint export.
