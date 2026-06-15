## 8. AI opponents (optional, `server/crates/ai`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). `addAi` accepts an optional `teamId` for
scripted team setup; when omitted, the server seats the AI into the next deterministic slot for
the current preset. The lobby also has a host-only
`setQuickstart` toggle labeled "Debug mode", which causes the next match to begin
with 99,999 steel and 99,999 oil for every player plus a prebuilt human-only army/base loadout.
They are capped with humans at
`MAX_PLAYERS = 4` (the bundled maps have v2 spawn layouts for one through four active players).
AI players are seated after the humans in the lobby player list; their colors come
from the tail of `PLAYER_PALETTE` so they never collide with human colors. They persist across rematches and are cleared only when the room
empties of humans.

**Where it runs.** `rts-ai` owns one `AiController` per AI player, while `Game` remains AI-free.
The room task invokes controllers before `game.tick()`, gives each controller the same
fog-filtered `snapshot_for(player)` plus the static `start_payload()`, then enqueues emitted
ordinary `SimCommand`s. Every AI action therefore goes through the identical validation / cost /
supply / placement path in `services/commands.rs` — the AI has **no special authority** over the
simulation and can't cheat economy, placement, or fog rules. Outbound attacks target enemy
**start tiles**, which are public via the `start` payload; direct attacks only target currently
visible enemy units/buildings during local defense.
The worker direct-hit retreat reflex is the one extra live input: `Game::worker_retreat_commands_for`
projects recent own-worker damage metadata into ordinary `Move` commands, and the controller emits
them alongside profile decisions without reading private sim state.

`rts-ai` may import `rts-sim` public API, `rts-rules`, `rts-protocol`, and `rts-contract`. It must
not import the server shell, lobby internals, Axum/Tokio transport, or private sim modules through
path tricks. If AI needs more observations, add a public, fog-respecting `Game`/snapshot surface
instead of reaching into entity stores from the server layer.

**Strategy.** Each controller, on a staggered cadence
(`DECISION_INTERVAL` ticks), builds a constrained snapshot-backed `AiObservation` and delegates RTS
decisions to `rts_ai::ai_core::decision::decide_profile`. Live lobby AIs use the promoted
`ai_1_0_tech` profile by default and keep that profile for the whole match. There is no lobby
protocol or UI for selecting AI profiles. Team relationships are observation-only safety
inputs: player summaries carry `teamId`, visible allied entities are classified separately from
`visible_enemies`, public base targeting ignores allied starts, and live decisions receive the
current living player set so attack waves keep choosing living enemies. AI teammates still do not
share economy, production, command authority, build orders, attack plans, or a team controller.
It does not micro, scout, or choose hidden enemy unit positions. A local per-think budget in the
shared action layer prevents it from over-committing resources/supply it does not have.

**Shared AI core.** `rts_ai::ai_core` has deterministic profile data (`profiles.rs`) and a generic
ranked decision loop (`decision.rs`) that emits ordinary `SimCommand`s through shared action helpers.
The decision loop also emits manager traces: every think records typed strategic goals for economy,
supply, expansion, tech, production, local defense, frontal attack, and harassment, plus stable
blocker labels, high-level intent labels, command labels emitted through `AiActionContext`, and
budget/reservation deltas. Economy, expansion, and frontal-wave attack now have explicit plan
records. The economy plan owns worker targets, steel/oil assignment counts, occupied resource
nodes, and post-expansion local-assignment bounds. The expansion plan owns due/save decisions,
tech-blocking state, and blocked reasons such as defensive panic, missing prerequisite building,
missing defenders, pending City Centre, no candidate resources, or no valid site. The frontal-wave
plan owns ready combat groups, required-unit readiness, attack reissue cadence, staging, visible
combat target selection, and blockers such as waiting for units, waiting for a required Tank,
waiting for Methamphetamines, and cadence. Final command emission still goes through
`AiActionContext` and `ai_core::actions`.
The live lobby default and promoted profile is `ai_1_0_tech`; it parameterizes worker targets,
supply buffers, building/tech goals, production priorities, resource timing, expansion timing,
harassment, and attack thresholds without providing its own `think()` function. It opens with
four-Rifleman frontal waves, expands off a completed Training Centre, builds Research
Complex and Factory without adding Machine Gunners, Anti-Tank Guns, Artillery, or Command Cars,
produces Scout Cars while Tank research or Methamphetamines is blocked or pending, then prioritizes
Tanks once both Tank research and Methamphetamines complete. It reserves up to two completed Scout
Cars for harassment before frontal-wave readiness is calculated, so those cars do not satisfy Tank
wave sizes. The harassment manager chooses the nearest living enemy public start, derives the enemy
main steel-line center from public start-resource locations plus visible resource deltas, and moves
the Scout Cars through an outer flank waypoint before queueing a back-side, off-axis point beyond
that steel line. If the harassment group sees enemy combat units near its route, it breaks contact
with a fog-respecting evasive `Move`; otherwise it reissues the harassment route on a short cadence.
It does not focus workers, ignore hidden
buildings, regroup, or use Scout Car smoke in AI 1.0. Tank frontal waves require a Tank in
the ready group and Methamphetamines before launch; while waiting, ready Tank groups stage toward
the enemy instead of dribbling into attack orders. Methamphetamines is enforced before first Tank
production, not only before Tank attack launch, so Tank production and Tank-wave readiness cannot
race ahead of the upgrade.
The profile includes a defensive panic mode. Visible enemy units near the AI's base, home resource
line, or workers temporarily suspend expansion, worker training, and non-defensive tech spending
only when their steel+oil value is at least 75% of the AI's own local unit value. While panicking,
the AI classifies the visible local threat by weapon DPS: tank-dominated pressure (75%+ of visible
local DPS) prioritizes Anti-Tank Guns, infantry-dominated pressure prioritizes Machine Gunners, mixed
pressure asks for a support mix, and no-DPS pressure falls back to Riflemen. Support panic only uses
already-completed support tech: Machine Gunners need a Training Centre and Anti-Tank Guns need a
Gun Works plus Anti-Tank Gun Crews research. It may pull workers onto oil for those support counters; if
the relevant support tech is absent, production falls back to Riflemen and panic mode does not
create tech buildings.
If the pressure persists through the panic window, the AI asks for an additional Barracks before
resuming its normal profile once the threat has cleared.
Developer self-play tooling also registers `ai_1_1_tank_mg` for direct comparison through
`ai-matchup` and related profile-backed scripts. AI 1.1 is a close AI 1.0 fork that keeps the same
economy, expansion timing, Tank tech path, Methamphetamines-before-Tanks gate, and Tank-required
frontal-wave posture, but removes Scout Car production and harassment, caps ordinary Barracks growth
at two, and carries a bounded defensive Machine Gunner target for later perimeter-staging behavior.
The aliases `ai_1_1` and `ai11` resolve to `ai_1_1_tank_mg`; `ai`, `ai1`, `ai_1_0`, and `default`
still resolve to `ai_1_0_tech` until release replay evidence justifies promotion.
The live lobby AI uses this shared core through `AiController`, which only owns live identity,
profile id, cadence, and persistent decision memory. Unknown live profile ids resolve to the
promoted `ai_1_0_tech` default. Profiles are still not client-selectable, and older experimental
profile ids are no longer listed or accepted by developer tooling. AI 1.1 is available only in
developer tooling at this stage; it is not the live lobby default.

**Self-play scorecards.** The `ai-matchup` and `ai-balance-matrix` developer tools emit
profile-agnostic baseline scorecards from public self-play commands and snapshots. Per-player
results include army value, building value, final worker count, final unit counts, command count,
attack command count, damage events dealt, deduplicated deaths, first attack command, first
Rifleman attack command, first Scout Car completion, first Scout Car harassment command, first
expansion City Centre planned/completed, and first Tank completion. Match-level results include
winner or tick-cap status, first damage, attack events, death events, replay verification status,
and optional replay artifact path. Compact baseline scenario metadata for opening pressure,
mid-game expansion, tank tech, and blocked-goal pressure lives in
`server/crates/ai/src/selfplay/scenarios.rs` so later AI changes can compare the same authored
fixtures without rewriting the harness.
Profile matchup JSON also includes a bounded `aiTraceTail` of compact trace entries for recent
profile-backed thinks. The tail is diagnostic output only; deterministic replay artifacts continue
to use the command log as the source of player intent.

Spectators never count toward win/elimination and receive a neutral final scoreboard result.

**Win/elimination.** AI players count as match players: a 1-human + N-AI match is a real match
(it resolves to a winner), while a lone human with no AI remains a never-ending sandbox. They have
one special elimination rule: an AI with no units left is defeated even if it still owns buildings,
because it has no player input path back into the game. The lobby's `match_player_count` is humans
**+** AIs.

---
