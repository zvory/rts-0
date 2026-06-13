# Team Games - Multi-Phase Plan

This plan adds team games as a relationship layer over the existing per-player ownership model.
The implementation should make FFA remain the default, add short-run lobby support for solo
sandbox, FFA, 1v2, 1v3, and 2v2, and keep the simulation generic enough for future maps and team
sizes. Testing is a first-class deliverable: each phase should add focused automated coverage and
avoid relying on manual multi-tab setup except for a small final browser sanity check.

## Whole-Effort Constraints

- `Entity.owner` remains the owning player id. Team identity is separate relationship data.
- Team id `0` is invalid for match players. Neutral entity owner `0` remains neutral.
- FFA is represented as one nonzero team per player and must remain behavior-compatible with today.
- Supported short-run presets are `solo`, `ffa`, `1v2`, `1v3`, and `2v2`. Presets are a lobby
  setup policy only; the simulation must consume ordered player/team data and must not branch on
  preset names.
- `solo` means exactly one active non-spectator player on Team 1, no forced AI opponents, and the
  existing never-ending one-player sandbox outcome behavior.
- Player economy, tech, supply, production queues, build authority, command authority, and score rows
  remain per-player. Do not add shared economy or shared control.
- Server relationship checks must be centralized. Combat, command validation, fog, projection,
  events, AI, victory, and replay should not grow new raw `owner != player` hostility checks.
- Keep an audit list that separates hostile relationship checks from strict own-control checks.
  Strict ownership checks remain correct for economy, production, command authority, prediction,
  control groups, resource snapshots, and local-only planning affordances.
- Client relationship checks must be centralized in `GameState` helpers. Gameplay UI should classify
  owners as own, ally, enemy, or neutral instead of comparing directly to `state.playerId`.
- Protocol mirrors stay synchronized across `server/crates/contract`, `server/crates/protocol`,
  `server/src/protocol.rs`, `client/src/protocol.js`, and `docs/design/protocol.md`.
- Every new protocol or artifact field needs an explicit compatibility default. Older replay
  artifacts, branch fixtures, match-history score JSON, and hand-built tests should default missing
  team fields to singleton-team FFA unless a phase deliberately rejects a team-specific artifact.
- Shared vision is authoritative on the server. If any living teammate currently sees an enemy,
  every teammate may receive that enemy; if no teammate sees it, no teammate should.
- A living teammate is an active match player whose team is not defeated. Spectators are never
  teammates for vision, disconnected players contribute no live sight after elimination, and AI seats
  follow the same alive/defeated rules as other match players.
- Allied entity details are visible for inspection, but allied resources and command affordances are
  not.
- Team presets should stay scriptable and testable before they are fully user-facing. Do not expose
  normal lobby UI flows for non-FFA team games until both server-side team safety and client command
  safety are in place.
- Before team-aware starts land, tests may assert team ids and match behavior but must not assert
  teammate proximity. Start assignment remains the current player-order behavior until the start
  phase changes it.
- Tests should be the main verification path. Build reusable Node and Rust harnesses so team lobby
  setup, AI seating, match start, fog sharing, combat safety, and score outcomes are exercised
  without hand-driving three or four browser tabs.
- Node suites that use live WebSockets must state whether they start their own server or require one
  to be running, and must document the port behavior in the suite or helper.
- After each phase, the implementing agent must commit, merge the phase branch to `main`, push
  `main`, and provide a handoff message for the next agent. The handoff must summarize what changed,
  what automated tests were run, any residual risk, and a short manual testing focus for core
  behavior only.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phases

- [Phase 0 - Automated team harness foundation](phase-0.md)
- [Phase 1 - Team identity and relationship contract](phase-1.md)
- [Phase 2 - Scriptable lobby team setup](phase-2.md)
- [Phase 3 - Team-safe command targeting](phase-3.md)
- [Phase 4 - Team-safe damage, effects, and notices](phase-4.md)
- [Phase 5 - Team victory and game-over semantics](phase-5.md)
- [Phase 6 - Shared current vision](phase-6.md)
- [Phase 7 - Projection, memory, and event privacy](phase-7.md)
- [Phase 8 - Client command safety and ally inspection](phase-8.md)
- [Phase 9 - Lobby and score UI exposure](phase-9.md)
- [Phase 10 - Team-aware starts on authored maps](phase-10.md)
- [Phase 11 - AI team safety](phase-11.md)
- [Phase 12 - Replay, branch, sim-wasm, logs, and match history](phase-12.md)
- [Phase 13 - End-to-end hardening and release audit](phase-13.md)

## Phase Summaries

Phase 0 builds the automated testing foundation before team behavior exists. It adds reusable test
helpers for creating multi-client rooms, seating AIs, waiting for starts/snapshots/game-over, and
asserting protocol shapes. The expected behavior remains FFA, but later phases should be able to
reuse the harness instead of opening multiple tabs manually.

Phase 1 introduces canonical team fields and relationship helpers while preserving current FFA
behavior. It threads `teamId` through lobby, start, score, replay-facing contracts, and client state
helpers. The phase should prove singleton-team FFA still behaves like today and should not change
combat, fog, starts, or victory yet.

Phase 2 makes team setup scriptable without making non-FFA team games fully user-facing yet. It adds
host-only WebSocket commands, preset validation, AI-to-team seating, and integration tests for solo,
FFA, 1v2, 1v3, and 2v2 setup. This phase proves lobby/team data can be created by automation while
in-game behavior may still be FFA-like.

Phase 3 narrows the first simulation behavior change to command validation and target acquisition.
Raw allied attack commands, auto-acquisition, ordered target retention, attack-move targeting, and
building targeting must use the relationship API. This phase should not change damage attribution,
area effects, event delivery, fog, starts, or victory.

Phase 4 handles the remaining hostile damage and feedback surfaces. Direct damage, overpenetration,
mortar/artillery/smoke-related area effects, last-damage owner, kill credit, worker retreat, and
under-attack notices are audited separately from command targeting. Tests should prove allies are
not damaged or credited as enemies under the selected no-friendly-fire rule.

Phase 5 changes only team defeat, winner, score, and game-over semantics. A team stays alive while
any member remains alive, one-player sandbox remains never-ending, and final results include
`winnerTeamId` without breaking singleton-FFA `winnerId` compatibility. This gives match resolution
its own automated proof before fog/client/replay work begins.

Phase 6 implements server-authoritative shared current vision. Fog recompute, living teammate sight,
smoke blocking, lingering death sight, `visibleTiles`, and no-entity living teammate cases are tested
without also changing projection details or transient event fanout. Hidden enemies remain hidden
unless at least one living teammate currently sees them.

Phase 7 audits the privacy boundary for projected allied details, remembered buildings, resource
deltas, target tracers, support-fire markers, and transient events. It defines which details are
ally-visible and which remain owner-only, then proves hidden enemy positions and ids do not leak
through team sharing. This phase is where mortar/artillery marker delivery to allies becomes
visible.

Phase 8 updates in-match client behavior so allies are inspectable but not commandable. GameState
relationship helpers replace relationship-based direct owner checks in selection, input, HUD,
renderer, minimap, prediction-sensitive paths, and command emission guards. Non-FFA team presets
should still remain normally hidden unless the client command-safety tests are green.

Phase 9 exposes the user-facing lobby and score UI for team games. It adds compact preset controls,
grouped team rows, host-only team/AI affordances, score Team columns, and winning-team highlighting
after the authoritative and client command-safety pieces exist. This phase is a UI/exposure gate,
not another simulation behavior phase.

Phase 10 makes authored map start assignment team-aware. The map loader should accept match
player/team assignments, preserve FFA randomness as much as practical, and choose together-biased
starts for team games without encoding lobby preset names into simulation code. Rust map tests
should cover current authored maps plus a synthetic larger layout so the algorithm is vector-based.

Phase 11 extends relationship semantics into AI only. AI players remain strategically independent
but must ignore allies as targets, use shared vision, and choose enemy players/starts for attack and
expansion reasoning. AI tests should prove observation and decision helpers classify allies
separately from enemies.

Phase 12 carries team identity through replay playback, replay branch staging, local prediction
fixtures, sim-wasm, structured logs, and match history. Replay and branch tests should prove team
ids, winner team, score rows, and vision modes survive capture, playback, seeking, and branch starts.
Match-history/log changes should avoid schema churn unless the existing JSON fields are insufficient.

Phase 13 performs the final automated hardening and documentation audit. It finalizes
`tests/team_integration.mjs` as the canonical multi-client team suite, verifies selector rules, runs
the broad local gate, and limits manual testing to one scripted browser sanity pass over the core
team-game flow.
