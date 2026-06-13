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
- Player economy, tech, supply, production queues, build authority, command authority, and score rows
  remain per-player. Do not add shared economy or shared control.
- Server relationship checks must be centralized. Combat, command validation, fog, projection,
  events, AI, victory, and replay should not grow new raw `owner != player` hostility checks.
- Client relationship checks must be centralized in `GameState` helpers. Gameplay UI should classify
  owners as own, ally, enemy, or neutral instead of comparing directly to `state.playerId`.
- Protocol mirrors stay synchronized across `server/crates/contract`, `server/crates/protocol`,
  `server/src/protocol.rs`, `client/src/protocol.js`, and `docs/design/protocol.md`.
- Shared vision is authoritative on the server. If any living teammate currently sees an enemy,
  every teammate may receive that enemy; if no teammate sees it, no teammate should.
- Allied entity details are visible for inspection, but allied resources and command affordances are
  not.
- Tests should be the main verification path. Build reusable Node and Rust harnesses so team lobby
  setup, AI seating, match start, fog sharing, combat safety, and score outcomes are exercised
  without hand-driving three or four browser tabs.
- After each phase, the implementing agent must commit, merge the phase branch to `main`, push
  `main`, and provide a handoff message for the next agent. The handoff must summarize what changed,
  what automated tests were run, any residual risk, and a short manual testing focus for core
  behavior only.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Phases

- [Phase 0 - Automated team harness foundation](phase-0.md)
- [Phase 1 - Team identity and relationship contract](phase-1.md)
- [Phase 2 - Lobby presets and scripted team setup](phase-2.md)
- [Phase 3 - Team-safe simulation combat and victory](phase-3.md)
- [Phase 4 - Shared vision, projection, and event delivery](phase-4.md)
- [Phase 5 - Client team model, inspection, and command safety](phase-5.md)
- [Phase 6 - Team-aware starts on authored maps](phase-6.md)
- [Phase 7 - AI, replay, branch, and match-history coverage](phase-7.md)
- [Phase 8 - End-to-end hardening and release audit](phase-8.md)

## Phase Summaries

Phase 0 builds the automated testing foundation before team behavior exists. It adds reusable test
helpers for creating multi-client rooms, seating AIs, waiting for starts/snapshots/game-over, and
asserting protocol shapes. The expected behavior remains FFA, but later phases should be able to
reuse the harness instead of opening multiple tabs manually.

Phase 1 introduces canonical team fields and relationship helpers while preserving current FFA
behavior. It threads `teamId` through lobby, start, score, replay-facing contracts, and client state
helpers. The phase should prove singleton-team FFA still behaves like today and should not change
combat, fog, starts, or victory yet.

Phase 2 makes the lobby able to configure teams through presets and scripted commands. Host-only
team preset, explicit team assignment, and AI-to-team seating are added, along with automated
integration tests that set up solo, FFA, 1v2, 1v3, and 2v2 rooms. This phase should make team setup
machine-testable but should still not depend on in-game team combat or shared vision.

Phase 3 makes the authoritative simulation treat allies as allies for command validation, targeting,
damage attribution, and victory. Raw attack commands, auto-acquisition, overpenetration, support
weapon damage, notices, kill credit, worker retreat, and game-over logic must use the relationship
API. Automated Rust tests should cover team-safe combat and team defeat, with Node regression tests
covering malicious allied attack attempts.

Phase 4 adds team-shared current vision, allied full-detail snapshots, and team-safe event delivery.
Fog recompute, lingering sight, smoke gates, remembered buildings, target tracers, mortar/artillery
markers, and resource deltas must be audited through team relationships. Tests should verify that
allies see each other's current vision and support-fire markers while hidden enemy entities and
positions remain hidden from the whole team.

Phase 5 updates the client so allies are readable, inspectable, and not commandable. GameState
relationship helpers should replace direct owner comparisons in input, HUD, minimap, renderer,
command card, local fog, and prediction-sensitive paths. Client contract and smoke tests should
verify allied single-click inspection, box-selection exclusion, right-click non-attack behavior,
scoreboard team display, and no command emission for allied-only selections.

Phase 6 makes authored map start assignment team-aware. The map loader should accept match player
team assignments, preserve FFA randomness, and choose together-biased starts for 1v2, 1v3, and 2v2
without encoding those presets into the simulation. Rust map tests should cover the current authored
maps plus a synthetic larger layout so the algorithm is vector-based, not fixed to four slots.

Phase 7 extends team identity and relationship semantics into AI, replay playback, replay branch
staging, self-play artifacts, local prediction fixtures, structured logs, and match history. AI
players should remain strategically independent but must ignore allies as targets, use shared vision,
and select enemy players and starts only. Replay and branch tests should prove team ids, winner team,
score rows, and vision modes survive capture, playback, seeking, and branch starts.

Phase 8 performs the final automated hardening and documentation audit. It should add or finalize a
dedicated `tests/team_integration.mjs` suite that drives the supported team shapes end to end without
manual tab work. Manual testing should be limited to one concise browser pass over lobby controls,
ally inspection, support-fire markers, and score UI after broad automated coverage passes.
