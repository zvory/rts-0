# Hotspot Architectural Group Map

This map defines the stable group names used by `scripts/hotspot-analysis.mjs`.
Use groups when a cleanup splits or moves files, because path-level history can make churn appear to
vanish even when the same architectural responsibility is still active.

The script-local table is the executable source for matching. This document is the reviewable
version that explains why each group exists and how to extend it after future splits.

## Default Groups

| Group | Current paths and future split paths | Why it is grouped |
| --- | --- | --- |
| `protocol-and-contracts` | `server/crates/protocol/**` including `server/crates/protocol/src/contract_metadata.rs`, `server/crates/contract/**`, `server/src/protocol.rs`, `client/src/protocol.js`, `client/src/protocol_constants.js`, future `client/src/protocol_*.js`, future `client/src/protocol/**`, `tests/protocol_parity.mjs`, `tests/client_contracts.mjs`, future `tests/client_contracts/**` | Rust protocol, JS protocol, compact codecs, parity tests, and the broad client contract runner co-change as one wire-contract surface. |
| `balance-and-config` | `server/crates/rules/src/balance.rs`, `server/crates/rules/src/defs.rs`, `server/crates/rules/src/faction.rs`, `server/src/config.rs`, `server/crates/sim/src/config.rs`, `client/src/config.js`, wiki/faction parity scripts | Rust rules are authoritative and the client config mirror is player-visible; split files must still be reviewed as one balance mirror. |
| `server-lobby-runtime` | `server/src/lobby/**` | Room lifecycle, session policy, projection, participants, lab/replay/live flow, and room-task helper splits stay one runtime ownership area. |
| `sim-command-service` | `server/crates/sim/src/game/services/commands.rs`, future `server/crates/sim/src/game/services/commands/**`, `server/crates/sim/src/game/command.rs`, `server/crates/sim/src/game/commands.rs` | Command input validation, command DTOs, planner adapters, and command-service tests should be compared together after extraction. |
| `sim-tests` | `server/crates/sim/src/game/tests.rs`, future `server/crates/sim/src/game/tests/**` | Broad `Game` API tests can be split by behavior family without changing their logical ownership. |
| `sim-movement-service` | `server/crates/sim/src/game/services/movement/**` | Movement tests and helpers already have their own service boundary. |
| `sim-combat-service` | `server/crates/sim/src/game/services/combat/**` | Combat tests and helpers already have their own service boundary. |
| `sim-services` | other `server/crates/sim/src/game/services/**` files | Shared sim service helpers that are not command, movement, or combat specific. |
| `sim-core` | remaining `server/crates/sim/**` files | `Game`, setup, systems, projection, stores, and other sim crate surfaces that are not narrower service groups. |
| `ai` | `server/crates/ai/**` | AI decision, self-play, fixture, and profile code often moves together and has environment-gated coverage. |
| `client-match-shell` | `client/src/match.js`, `client/src/match_*` app-shell collaborators, app/frame/health/replay/pause/observer shell helpers | Match composition, frame order, teardown, and live/replay/lab shell behavior should stay one review group. |
| `client-hud` | `client/src/hud.js`, `client/src/hud_*.js`, `client/src/resource_icons.js` | HUD rendering and command-card helpers are player-facing DOM/control surfaces with shared contracts. |
| `client-state-model` | `client/src/state.js`, state query/effect helpers, `client/src/client_intent.js`, `client/src/command_budget.js`, command composer, prediction, progress, and sim-wasm adapter helpers | Client model, intent, command-budget, and prediction state must stay separate from renderer/HUD while being analyzed as one model surface. |
| `client-input` | `client/src/input/**`, `client/src/replay_camera_input.js` | Input routing has its own dependency and browser-event constraints. |
| `client-renderer` | `client/src/renderer/**`, `client/src/camera.js`, `client/src/fog.js`, `client/src/minimap.js` | Rendering, fog display, camera, and minimap co-change around frame presentation. |
| `client-ui` | `client/styles.css`, lobby, lab, settings, match-history, and other UI modules not claimed above | Remaining DOM/UI surfaces share static-client and CSS selector constraints. |
| `server-backend` | remaining `server/src/**` files | Axum, startup, backend routing, and server support code outside room-runtime helpers. |
| `scripts-tooling` | `scripts/**` | Repo-local checks, runners, and analysis tools are tooling rather than runtime gameplay code. |
| `rules` | remaining `server/crates/rules/**` files | Rules data outside the balance mirror may still co-change with balance and sim code. |
| `tests` | remaining `tests/**` files | Integration and smoke tests outside the broad client contract runner should be reviewed as test infrastructure. |

## Update Rules

- Add new split files to the same group as the responsibility they came from before comparing
  before/after cleanup metrics.
- Keep protocol and balance mirrors grouped across Rust, JS, tests, docs checks, and parity scripts;
  do not judge one side by path-level churn alone.
- If a follow-up plan creates a new stable area, update both this document and the `GROUP_RULES`
  table in `scripts/hotspot-analysis.mjs` in the same commit.
- Treat raw stale paths as history clues, not cleanup targets. Prefer a current file or group with
  current LOC, recent churn, and recent co-change evidence.
