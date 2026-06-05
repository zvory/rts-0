# Phase 8 - Final Hardening and Documentation Audit

Goal: finish the cleanup by removing temporary seams, checking documentation, and validating that
the new component layout is easier to maintain.

## Scope

- Remove temporary compatibility wrappers that were only needed during extraction.
- Tighten visibility from `pub(crate)` to `pub(super)` or private where possible.
- Check that module names match actual ownership rather than vague helper buckets.
- Update `DESIGN.md` if any final module boundaries or public contracts changed.
- Add brief module-level docs for new module roots that own important behavior.
- Re-run line-count and public-item baselines from Phase 0.

## Quality Checks

- No extracted module should become a generic dumping ground.
- No new cyclic dependency or hidden service coupling should exist.
- Tests should live near the behavior they protect.
- `systems.rs` should still read as the simulation tick orchestrator.
- `Game` should still be the API seam for lobby/main callers.
- Client modules should still be usable as plain ES modules without a build step.

## Tests

- Run `cargo test` in `server/`.
- Run Node integration/regression scripts for any touched lobby, protocol, or client behavior.
- Run client smoke tests after client decomposition.

## Done

- The largest files are reduced by cohesive extraction, not by scattering behavior.
- Documentation matches the resulting architecture.
- The cleanup does not change gameplay except where a separate, explicit follow-up change says so.

## Final Audit - 2026-06-04

Phase 8 removed the temporary client facade re-export files:

- `client/src/renderer.js`
- `client/src/input.js`

Runtime imports now point at the owning module roots, `renderer/index.js` and `input/index.js`.
`DESIGN.md` and the context capsules were updated to match the extracted server/client layout.
No protocol, balance, or gameplay contracts changed.

Module-level ownership docs were added to important server roots that now own behavior:

- `server/src/game/services/mod.rs`
- `server/src/game/services/movement/mod.rs`
- `server/src/game/services/combat/mod.rs`
- `server/src/game/ai_core/mod.rs`

### Current Hotspot Line Counts

Measured with `wc -l` after Phase 8 edits:

| Path | Lines | Public items |
| --- | ---: | ---: |
| `server/src/game/ai_core/decision/` | 5,424 | 121 |
| `server/src/game/services/movement/` | 4,950 | 43 |
| `server/src/game/selfplay/` | 3,633 | 74 |
| `server/src/game/services/combat/` | 2,517 | 30 |
| `client/src/renderer/` | 2,409 | N/A |
| `server/src/lobby/` | 1,770 | 30 |
| `server/src/game/entity/` | 1,692 | 139 |
| `server/src/game/services/move_coordinator.rs` | 1,446 | 11 |
| `server/src/game/services/pathing.rs` | 1,214 | 10 |
| `server/src/game/ai_core/actions.rs` | 1,147 | 37 |
| `client/src/input/` | 1,110 | N/A |
| `server/src/game/services/commands.rs` | 867 | 2 |
| `server/src/game/mod.rs` | 270 | 18 |

The largest individual files after cleanup are test-heavy modules:

| File | Lines |
| --- | ---: |
| `server/src/game/services/movement/tests.rs` | 3,158 |
| `server/src/game/ai_core/decision/tests.rs` | 2,758 |
| `server/src/game/selfplay/tests.rs` | 1,543 |
| `server/src/game/services/combat/tests.rs` | 1,461 |

### Verification Scope

Required verification for this phase:

- `cd server && cargo test`
- `node tests/client_contracts.mjs`
- Client smoke test, because the client module import paths changed.
