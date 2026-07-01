# Phase 6 - Integration, Documentation, And Playtest Hardening

## Phase Status

Status: done.

## Objective

Close the artillery UX effort with cross-area verification, final documentation alignment, and
playtest-focused cleanup. This phase should not introduce broad new behavior; it should prove that
the server contract, client affordances, protocol mirrors, balance docs, and tests all describe the
same shipped feature.

## Scope

- Audit all Point Fire and Blanket Fire behavior against [requirements.md](requirements.md).
- Update any remaining stale wording in:
  - `docs/design/protocol.md`
  - `docs/design/server-sim.md`
  - `docs/design/client-ui.md`
  - `docs/design/balance.md`
  - `docs/context/*.md` only if section lists or capsule pointers shifted.
- Add or adjust focused regression coverage for gaps left by earlier phases:
  - Point Fire and Blanket Fire separate command identities,
  - 25-tile minimum range target locking,
  - setup/redeploy owned by the fire order,
  - terminal queued fire behavior,
  - deterministic Blanket Fire sampling,
  - mixed selection behavior,
  - stale preview cleanup,
  - minimap/world targeting parity,
  - fog and event visibility.
- Run selector or focused checks to confirm the changed files map to expected suite coverage.
- Collect final gameplay patch-note bullets in the phase handoff and commit body.
- Prefer small fixes that close integration gaps. Stop as blocked if a discovered issue requires a
  new product decision or a broad redesign outside this plan.

## Expected Touch Points

- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `docs/design/balance.md`
- `docs/context/protocol.md`
- `docs/context/server-sim.md`
- `docs/context/client-ui.md`
- `docs/context/balance.md`
- `server/crates/sim/src/game/tests/artillery_tests.rs`
- `server/crates/sim/src/game/phase7_privacy_tests.rs`
- `tests/client_contracts/*.mjs`
- `tests/minimap_input_contracts.mjs`
- `tests/protocol_parity.mjs`
- `tests/select-suites.mjs`

## Edge Cases To Cover

- A packed artillery piece can accept immediate Point Fire and Blanket Fire, set up in place, and
  fire later without moving.
- A deployed artillery piece redeploys in place when the locked center is outside its current cone.
- Raw clicks inside minimum range and outside maximum range lock to stored effective points.
- A raw click near map edges that cannot produce a valid in-map locked point is ignored safely.
- Multiple selected artillery pieces store different effective targets when their origins differ.
- Blanket Fire samples deterministic area impacts around the stored center and does not tighten with
  Ballistic Tables.
- Point Fire still tightens with Ballistic Tables across the new 25-to-55 range band.
- Stop cancels active point fire, active blanket fire, setup/redeploy created by those commands, and
  queued orders.
- Enemies never receive owner-only target markers or hidden positions; global minimap firing markers
  remain visual-only as documented.
- Replays, reconnect snapshots, and owner-only order plans display accepted stages without relying
  on local transient preview state.

## Verification

- `node tests/select-suites.mjs --from=origin/main` to inspect expected suite mapping.
- Focused Rust and JS tests for any integration gaps fixed in this phase.
- `node tests/protocol_parity.mjs`
- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-wiki.mjs`
- `node scripts/check-client-architecture.mjs` if any client files change.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if any
  sim files change.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Play one local match or dev scenario that covers immediate Point Fire, immediate Blanket Fire,
queued move/setup/fire, minimap fire targeting, Stop cancellation, close/far target locking, and a
mixed artillery plus non-artillery selection. Confirm the player-facing behavior matches the final
patch notes and that no preview implies automatic walking.

## Handoff Expectations

Summarize the final player-facing behavior and list the patch-note bullets. Include exact
verification commands and manual scenarios covered, plus any remaining playtest risks to watch,
especially preview mismatches caused by later queue changes, blocked movement, ammo changes, or
loss of ownership before the first shot.
