# Phase 2 - Domain, Rules, and Balance Extraction

Status: Planned.

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

