# Phase 7 - Integration Audit and Hardening Pass

Goal: audit the completed movement stack for contract drift, missing tests, replay determinism,
and user-visible regressions.

Run this after the implemented movement phases, even if Phase 6 local steering is skipped.

## Scope

In scope:

- Update documentation to match final behavior.
- Add missing regression coverage discovered during integration.
- Run broad local test suites.
- Inspect self-play artifacts if movement behavior looks wrong.
- Remove stale comments that describe old symmetric collision or welded tank barrels.

Out of scope:

- No new movement features.
- No balance redesign.
- No persistent slots or flow-field work.

## Files To Check

- `DESIGN.md`
- `docs/movement-options-research.md`
- `docs/movement/PLAN.md`
- Implemented phase docs if behavior intentionally diverged.
- `server/src/game/services/movement.rs`
- `server/src/game/services/move_coordinator.rs`
- `server/src/game/services/combat.rs`
- `server/src/game/invariants.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/renderer.js`
- tests under `server/src/**` and `tests/`

## Audit Checklist

- `DESIGN.md` accurately describes:
  - weighted collision and ghost exceptions,
  - formation goal behavior,
  - `facing` as body facing,
  - `weaponFacing` if Phase 4 landed,
  - facing-aware damage if Phase 5 landed,
  - local steering if Phase 6 landed.
- Protocol mirrors agree:
  - semantic `EntityView`,
  - compact snapshot field order,
  - JS compact decoder indexes,
  - docs in `DESIGN.md`.
- Fog safety is preserved:
  - hidden targets do not expose `targetId`,
  - hidden target direction does not expose `weaponFacing`,
  - events remain visibility-gated.
- Tick path stays hard:
  - no `unwrap()` or `expect()` on stale ids in movement/combat,
  - no unchecked indexing into paths or compact fields,
  - no unbounded per-tick loops or all-pairs work outside spatial-index broad phase.
- Determinism is preserved:
  - no unseeded randomness in simulation,
  - neighbor iteration is stable or sorted,
  - tests do not rely on hash iteration order.
- Client teardown/rendering remains stable:
  - no leaked listeners or resources,
  - no renderer assumptions that all entities have `weaponFacing`,
  - interpolation handles missing optional fields.

## Regression Scenarios

Make sure tests or manual self-play cover:

- Workers can still harvest without being blocked by pass-through collision exceptions.
- Moving units can shove idle soft units aside.
- Deployed machine gunners stay planted.
- Far group moves preserve rough shape.
- Close group moves compact.
- Tanks turn smoothly through path corners.
- Tank turrets rotate independently if Phase 4 landed.
- Tank front/side/rear damage differs if Phase 5 landed.
- Fogged snapshots do not leak hidden aim information.
- AI attack-move and expansion behavior still functions.

## Commands

Run formatting and focused tests first, then the broader suite:

```bash
cd server && cargo fmt && cargo test
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
cd tests && npm install && node client_smoke.mjs
```

The Node integration tests need a running server. Use the normal `cd server && cargo run` flow.

If a self-play test fails and the cause is not obvious, follow `CLAUDE.md`: start a fresh server
and open the local `/dev/selfplay?replay=...` URL with the macOS `open` command so the failure state
can be inspected.

## Acceptance Criteria

- Documentation matches implemented behavior.
- Full Rust tests pass.
- Client contract/smoke tests pass.
- Server integration, regression, and AI integration tests pass, or failures are clearly unrelated
  and documented.
- No protocol mirror drift remains.
- No movement/combat fog leak is known.
- No follow-up movement TODO is required for correctness.
