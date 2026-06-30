# Phase 3 - Blanket Fire Server Runtime

## Phase Status

Status: pending.

## Objective

Implement Blanket Fire as a real server-authoritative artillery fire mode. It should use its own
ability and order-stage identity, store a locked center point, own setup or redeploy before firing,
and deterministically sample impact points within a 15-tile radius around the stored center.

## Scope

- Add an authoritative order representation for Blanket Fire. Prefer a shared artillery-fire order
  mode or focused enum only if it reduces duplication with Point Fire without obscuring the two
  mirrored command identities.
- Accept `useAbility(blanketFire)` only for owned, completed Artillery that can receive the same
  fire-order flow as Point Fire.
- Reuse the phase 2 target-locking helper for the stored blanket center. Store the locked center,
  not the raw click.
- Use the stored center for setup/redeploy decisions. The 15-tile blanket radius must not make a
  current cone/range decision valid or invalid.
- Make immediate and queued Blanket Fire terminal per artillery unit, matching Point Fire terminal
  behavior.
- On each shot, sample a deterministic pseudo-random impact point uniformly from the 15-tile radius
  around the stored center using authoritative inputs such as match seed or tick, artillery id,
  owner, and shot number.
- Do not re-clamp sampled impacts to the cone or range band after sampling.
- Use the same ammunition cost, reload cadence, shell delay, impact radius, damage behavior,
  no-ammo behavior, fog/reveal behavior, and artillery shell scheduling surface as Point Fire unless
  the requirements explicitly say otherwise.
- Keep Ballistic Tables repeated-shot tightening Point-Fire-only. Blanket Fire remains deterministic
  uniform area suppression.
- Update owner/team target markers so they communicate sampled shell targets without leaking hidden
  enemy information beyond existing artillery rules.
- Update `docs/design/server-sim.md`, `docs/design/protocol.md`, and `docs/design/balance.md` for
  Blanket Fire execution, deterministic sampling, and the Ballistic Tables exclusion.

## Expected Touch Points

- `server/crates/sim/src/game/entity/order.rs`
- `server/crates/sim/src/game/services/order_execution.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_planner.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/services/commands/artillery_scatter.rs` or a nearby deterministic
  sampling helper
- `server/crates/sim/src/game/artillery.rs`
- `server/crates/sim/src/rules/projection.rs`
- `server/crates/sim/src/game/tests/artillery_tests.rs`
- `server/crates/sim/src/game/phase7_privacy_tests.rs` if event visibility changes
- `server/crates/rules/src/faction.rs`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/balance.md`

## Edge Cases To Cover

- Packed artillery accepts immediate Blanket Fire, sets up in place toward the locked center, and
  starts blanket firing after deployment.
- Deployed artillery uses current cone when the locked center is inside the cone and redeploys when
  it is outside.
- Queued Blanket Fire after movement locks from the future position when available and promotes
  through setup/redeploy safely.
- Blanket Fire is terminal: later queued unit stages do not append behind it for that artillery.
- Blanket Fire consumes the same steel per shot and applies the same reload delay as Point Fire.
- Unaffordable ammunition produces the same user-facing notice/cooldown behavior as Point Fire.
- Deterministic sampling produces identical impact sequences for the same command log and
  authoritative inputs.
- Different artillery pieces with the same raw click may store different centers and sample
  independent impact sequences.
- Sampled points near map edges are allowed if they fall outside the playable map only if existing
  shell scheduling and impact resolution safely handle them; otherwise define and test a
  deterministic safe clamp that does not reinterpret the stored center.
- Ballistic Tables does not tighten Blanket Fire into precision fire.
- Fog-gated `artilleryTarget`, `artilleryImpact`, `artilleryFiring`, and attack reveal behavior
  matches the documented artillery visibility rules.

## Verification

- Focused Rust tests for Blanket Fire command admission, setup/redeploy, queued promotion,
  terminal queues, ammunition behavior, deterministic sampling, and event visibility.
- Focused replay or deterministic simulation coverage proving repeated command logs produce the
  same sampled impact sequence.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  service boundaries change.
- `node tests/protocol_parity.mjs` if compact metadata or projection docs are touched.
- `git diff --check`

## Manual Test Focus

In a local match or dev scenario, issue Blanket Fire at a terrain point with a packed gun and a
deployed gun. Confirm the gun does not move, setup/redeploy happens in place, shells land scattered
around the stored center, Stop cancels the repeating fire, and Point Fire still tightens with
Ballistic Tables while Blanket Fire does not.

## Handoff Expectations

Describe the order representation chosen for Point Fire versus Blanket Fire and the deterministic
sampling inputs. Call out how event visibility was preserved and which client command-card or
preview work remains hidden until phase 4.
