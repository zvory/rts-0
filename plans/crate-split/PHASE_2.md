# Phase 2 - Domain, Rules, and Balance Extraction

Status: Done.

Goal: create a rules/balance crate that owns gameplay vocabulary and formula data without importing
simulation state.

## Scope

- Move `EntityKind` and kind classification into the rules/domain crate.
- Move unit/building/node stats structs and definitions into the rules/domain crate.
- Move terrain vocabulary and terrain formulas into the rules/domain crate.
- Move scalar constants from `config.rs` into the rules/domain crate when they are true simulation
  or balance constants.
- Keep compatibility wrappers for `config::unit_stats`, `config::building_stats`, and other common
  names while call sites migrate.
- Replace `EntityKind -> protocol string` conversion with an adapter layer outside the core domain
  type.
- Split `rules::projection`:
  - pure visibility predicates can stay in rules if they only use traits/primitives;
  - DTO construction and sim-state reads should move to sim snapshot/projection code.

## Design Notes

This phase is where most circular ownership gets fixed. The target is:

```text
rts-rules -> rts-contract only if needed
rts-sim   -> rts-rules
rts-protocol does not own EntityKind
```

If the client-visible balance mirror still requires string constants, generate or maintain those
through explicit adapter functions rather than letting domain types import protocol modules.

Projection is the riskiest part. It owns fog-gated entity view construction today, which is
gameplay-critical and wire-facing. Move it in small steps and keep fog leak tests close to the
change.

## Implementation Notes

- Added `server/crates/rules` as `rts-rules`.
- Moved `EntityKind`, kind classification helpers, unit/building/node stats and definitions,
  terrain vocabulary/formulas, combat formulas, economy rules, and simulation/balance constants
  into `rts-rules`.
- Reduced `server/src/config.rs` and `server/src/rules/mod.rs` to compatibility shims for existing
  server call sites.
- Kept `rules::projection` in the server crate because it still reads `Entity`, `EntityStore`,
  fog/smoke state, and builds protocol DTOs. Pure visibility extraction can happen later once a
  trait/primitives boundary is introduced.
- Replaced `EntityKind -> protocol string` inherent conversion with explicit server-side
  `protocol::kind_to_wire` / `protocol::kind_from_wire` adapters. The domain type no longer imports
  protocol constants.
- Added adapter tests for kind wire round-trips and terrain code parity between `rts-rules` and
  `rts-protocol`.

## Tests

- Unit tests for kind parsing/format adapters.
- Existing rules tests.
- Snapshot/fog/projection tests.
- `cd server && cargo test`
- Relevant Node regression tests for hardening/fog if projection moves.

## Done

- Rules/balance crate does not import sim entities, sim services, lobby, protocol transport, tokio,
  axum, or server perf.
- `EntityKind` no longer imports protocol constants.
- `config.rs` is either gone, reduced to a compatibility shim, or clearly owned by the rules crate.
- Fog-filtered snapshots and events remain behaviorally unchanged.
