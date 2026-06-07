# Crate Split Boundary Audit

Phase 0 records the current dependency direction violations before behavior moves into narrower crates. The target crate names and responsibilities remain the ones in `plans/crate-split/PLAN.md`.

## Confirmed Couplings

- `config` depends on sim/rules domain types: `server/src/config.rs` imports `game::entity::EntityKind` and `rules::defs`, while rules code also reads config constants. Target phase: Phase 2 (`rts-rules` / balance ownership).
- Sim/domain code depends on wire protocol DTOs and constants: `game/mod.rs`, `game/replay.rs`, `game/command.rs`, `game/entity/kind.rs`, `game/setup.rs`, `game/snapshot.rs`, AI/self-play helpers, and several services import `crate::protocol`. Target phases: Phase 1 for contract/protocol DTOs, then Phase 3 for removing protocol from sim.
- `rules::projection` is not pure rules: it imports `game::entity`, `game::fog`, `game::smoke`, and protocol view DTOs. Target phases: Phase 1 and Phase 3, likely by moving projection/fog-filtered view assembly out of `rts-rules`.
- `Game` owns live AI controllers: `game/mod.rs` stores `AiController`, and `game/setup.rs` builds AI-backed players. Target phase: Phase 4 (`rts-ai` emits ordinary commands through a public observation/query surface).
- Simulation tick code imports server perf instrumentation: `game/mod.rs` and `game/systems.rs` take `crate::perf::TickPerf`; `Game::perf_entity_counts` returns a perf DTO. Target phase: Phase 5, replacing direct server perf types with a sim-local instrumentation trait or event surface.
- Server/lobby is the only place with tokio/axum dependencies in production code: `main.rs`, `lobby/mod.rs`, and `lobby/room_task.rs` import tokio/axum/futures/tower. This is the intended outer shell; keep it from leaking inward during later phases.
- `lobby/snapshots.rs` owns compacting snapshots for wire fanout and depends on protocol DTOs. Target phase: Phase 5, keeping compacting/resource elision at the server/protocol boundary rather than in sim.

## Phase 0 Package Layout Decision

- Keep `server/` as the Cargo workspace root and the `rts-server` package for now.
- Add a root library target at `server/src/lib.rs` that re-exports the existing module tree as a compatibility surface.
- Keep future crate names from the plan: `rts-contract`, `rts-protocol`, `rts-rules`, `rts-sim`, `rts-ai`, `rts-server`, and `rts-tools` or individual tool packages.
- Do not move `src/main.rs` or module files in Phase 0; use imports from the shared library to remove binary-local duplicate module trees first.

## Useful Audit Commands

```bash
cd server
rg "crate::(protocol|game|rules|config|perf|lobby|dev_scenarios)" src/game src/rules src/protocol.rs src/config.rs src/perf.rs src/lobby
rg "\b(tokio|axum)::|futures_util|tower_http" src
rg "AiController|ai_core|ai_shared|crate::game::ai|super::ai" src/game
rg "crate::perf|TickPerf|PerfConfig|EntityCounts|SnapshotRecord|SnapshotEnqueue" src/game src/rules src/protocol.rs src/config.rs
```
