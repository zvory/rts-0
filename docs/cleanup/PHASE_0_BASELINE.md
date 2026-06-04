# Phase 0 Baseline - 2026-06-04

This records the cleanup baseline for the first implementation change after the decomposition plan.
The baseline was measured on commit `09d851e`.

## Phase Status

- Phase 0: implemented by this baseline note.
- Phase 1: not implemented yet. `server/src/game/selfplay.rs` is still a single source file.
- Phase 2: not implemented yet. `server/src/game/ai_core/decision.rs` is still a single source
  file.

## Hotspot Line Counts

Measured with `wc -l`:

| File | Lines |
| --- | ---: |
| `server/src/game/ai_core/decision.rs` | 4,932 |
| `server/src/game/selfplay.rs` | 3,459 |
| `server/src/game/services/movement.rs` | 3,597 |
| `server/src/game/services/combat.rs` | 2,081 |
| `client/src/renderer.js` | 1,735 |
| `server/src/game/mod.rs` | 1,699 |
| `server/src/game/entity.rs` | 1,611 |
| `server/src/lobby.rs` | 1,627 |
| `server/src/game/services/move_coordinator.rs` | 1,442 |

Total hotspot lines: 22,183.

## Public Surface Counts

Measured with `rg 'pub(\([^)]*\))?\s+(struct|enum|fn|trait|type|const|static|mod)'`:

| File | Public items |
| --- | ---: |
| `server/src/game/ai_core/decision.rs` | 6 |
| `server/src/game/selfplay.rs` | 23 |
| `server/src/game/services/movement.rs` | 12 |
| `server/src/game/services/combat.rs` | 1 |
| `client/src/renderer.js` | N/A |
| `server/src/game/mod.rs` | 40 |
| `server/src/game/entity.rs` | 133 |
| `server/src/lobby.rs` | 13 |
| `server/src/game/services/move_coordinator.rs` | 11 |

Phase 0 introduced zero new Rust public items.

## Current Test Coverage Map

Measured Rust unit test counts with `rg -c '#\[test\]'`:

| Area | Current coverage |
| --- | --- |
| `server/src/game/selfplay.rs` | 11 inline self-play tests plus replay artifact helpers. These run under `cd server && cargo test`. |
| `server/src/game/ai_core/decision.rs` | 41 inline AI decision tests. These run under `cd server && cargo test`. |
| `server/src/game/services/movement.rs` | 50 inline movement tests. These run under `cd server && cargo test`. |
| `server/src/game/services/combat.rs` | 32 inline combat tests. These run under `cd server && cargo test`. |
| `server/src/game/mod.rs` | 17 inline Game seam, simulation, replay, and snapshot tests. These run under `cd server && cargo test`. |
| `server/src/game/entity.rs` | 9 inline entity model tests. These run under `cd server && cargo test`. |
| `server/src/lobby.rs` | 9 inline lobby lifecycle tests. These run under `cd server && cargo test`. |
| `server/src/game/services/move_coordinator.rs` | 18 inline movement coordination tests. These run under `cd server && cargo test`. |
| `tests/server_integration.mjs` | Live-server lifecycle, protocol, fog, economy, training, and disconnect coverage. Run when lobby, protocol, or client/server integration behavior is touched. |
| `tests/regression.mjs` | Live-server hardening and robustness coverage. Run when network, command validation, placement, or tick safety behavior is touched. |
| `tests/ai_integration.mjs` | Live-server AI lobby flow coverage. Run when lobby AI controls or host-only AI lifecycle behavior is touched. |
| `tests/client_smoke.mjs` | Headless browser render and UI command loop coverage. Run when renderer, input, HUD, client protocol decode, or served client behavior is touched. |
| `server/src/ai_matchup.rs` | Manual fixed-horizon AI profile matchup binary. Use for profile-vs-profile balance checks after AI policy moves. |

## First Extraction Target

Phase 1 is the first extraction target: decompose `server/src/game/selfplay.rs`.

The first implementation should be mechanical move-only extraction, with only the visibility cleanup
needed to preserve current call sites from `lobby.rs`, `game/mod.rs`, and `ai_matchup.rs`.

Initial ownership boundary:

- `selfplay/mod.rs`: public test/dev entry points and re-exports for the existing call sites.
- `selfplay/live.rs`: `LiveSelfPlay` and live driver integration.
- `selfplay/replay.rs`: replay artifact structs, artifact loading/saving, replay driver, and replay
  comparison.
- `selfplay/scripts.rs`: `ScriptedPlayer`, script profiles, and command generation helpers.
- `selfplay/player_view.rs`: `PlayerView` and snapshot query helpers used by scripts.
- `selfplay/pending_build.rs`: pending build tracking and failed build site handling.
- `selfplay/milestones.rs`: milestone capture, combat goals, player goals, and assertion helpers.
- `selfplay/validation.rs`: snapshot/resource sanity checks and known-kind validation.
- `selfplay/tests.rs`: existing scenario tests if keeping them inline makes `mod.rs` too large.

Do not change self-play behavior, replay artifact field names, `Game` API usage, wire protocol,
config, or balance in Phase 1.

## Design Document Impact

`DESIGN.md` does not change in Phase 0 because this phase records baseline measurements and
guardrails only. No contract, module boundary, public seam, protocol shape, or gameplay behavior
changed.

## Verification

This is a docs-only baseline. No runtime tests were required for Phase 0.
