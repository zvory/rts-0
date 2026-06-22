## 8. AI opponents (optional, `server/crates/ai`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). `addAi` accepts an optional `teamId` for
scripted team setup; when omitted, the server seats the AI into the next deterministic slot for
the current preset. The legacy host-only `setQuickstart` compatibility command can still start the
next match with 99,999 steel and 99,999 oil for every player plus a prebuilt human-only
army/base loadout, but the normal lobby no longer exposes that command as a visible Debug mode
toggle. Use lab rooms for player-facing experimentation until debug-style starts return as explicit
lab presets or scenarios.
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
`ai_1_2_wave_cohorts` profile by default and keep that profile for the whole match. Hosts can select
`ai_1_0_tech`, `ai_1_1_tank_mg`, or `ai_1_2_wave_cohorts` per AI seat from the lobby before
countdown/start; unsupported
profile ids are ignored or defaulted to the highest supported live AI version. Team relationships are observation-only safety
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

The economy plan is backed by an AI-owned resource availability model derived only from the
fog-filtered observation, public start-payload resource positions, completed own City Centres,
visible resource deltas, current worker latches, and AI-owned reservations. The model keeps
known resources separate from resources that are mineable now: a steel or oil node is assignable
only when it has remaining resources, is in range of a completed own City Centre, is not occupied by
a latched worker, and is not already reserved by the current think. Known but non-mineable nodes
remain visible to expansion planning as future candidates, but economy worker assignment suppresses
oil demand when there is no free mineable oil and passes only free mineable node ids to
`assign_workers_to_resource`. The action layer also requires callers to provide that assignable set,
so an upstream economy mistake cannot knowingly emit a `Gather` command to non-mineable oil while
free mineable steel exists. Post-expansion assignment prefers workers near the expansion resource
line, and profiles that opt into remote fallback can still send a main-base idle worker to the
expansion once the main line is saturated instead of leaving it idle. Self-play regression coverage
preserves the pre-expansion case where oil is known but outside completed-City-Centre mining range,
and the post-expansion case where oil assignment begins after the expansion City Centre completes.
The AI 1.0 profile is `ai_1_0_tech`; it parameterizes worker targets,
supply buffers, building/tech goals, production priorities, resource timing, expansion timing, and
attack thresholds without providing its own `think()` function. It opens with
four-Rifleman frontal waves, expands off a completed Training Centre, builds Research
Complex and Factory without adding Machine Gunners, Anti-Tank Guns, Artillery, or Command Cars,
produces Scout Cars while Tank research or Methamphetamines is blocked or pending, then prioritizes
Tanks once both Tank research and Methamphetamines complete. Scout Cars are not reserved for
harassment, flank routes, or threat evasion; if they are present in the ready combat group, they use
the same frontal-wave attack-move behavior as Tanks and Riflemen. It does not focus workers, ignore
hidden buildings, regroup, or use Scout Car smoke in AI 1.0. Tank frontal waves require a Tank in the
ready group and Methamphetamines before launch; while waiting, ready Tank groups stage toward the
enemy instead of dribbling into attack orders. Methamphetamines is enforced before first Tank
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
Developer self-play tooling also registers `ai_1_1_tank_mg` and `ai_1_2_wave_cohorts` for direct
comparison through `ai-matchup` and related profile-backed scripts. AI 1.1 is a close AI 1.0 fork
that keeps the same
expansion timing, Tank tech path, Methamphetamines-before-Tanks gate, and Tank-required
frontal-wave posture, but launches its first Tank-era wave as soon as one Tank is ready. It removes
Scout Car production and harassment, caps ordinary Barracks growth at one, trains a bounded
defensive Machine Gunner group, pushes toward full two-base steel saturation, and can add a second
Factory once Tank production is active. Vehicle Works and Gun Works placement uses an expanded
center-facing search band so support and vehicle production do not pile up behind the base. Its
Tank-era production and frontal-wave composition are Tank-only, so Riflemen remain an
opening/defensive Barracks output rather than a continuing mid-game spend. It reserves up to four
ready Machine Gunners before frontal-wave readiness is calculated, so those MGs do not satisfy Tank
wave sizes.
When there is no local base threat, the reserved MGs receive deterministic individual attack-move
stage orders roughly 20 tiles past the main steel line toward the nearest living public enemy start,
using public resource geometry rather than hidden enemy positions. This pushes the defensive group
out far enough to contest approaches before attackers reach the expansion. Visible threats near the
base, home resource line, or workers still take priority over passive perimeter staging.
AI 1.2 (`ai_1_2_wave_cohorts`) is an AI 1.1 fork with explicit frontal-wave cohorting and
MG-style line staging for forming frontal waves. Once a frontal wave launches, its unit ids are
excluded from future frontal-wave readiness for a bounded window while they remain alive, so newly
trained Riflemen or Tanks must form the next outbound wave instead of being counted together with
the already-launched group. Forming waves receive deterministic individual attack-move staging slots
along the same enemy-facing main-steel line shape used by the defensive Machine Gunner perimeter,
avoiding a single rally point. Local defense still selects any eligible local combat unit, including
units that are excluded from outbound wave formation.
The aliases `ai_1_2` and `ai12` resolve to `ai_1_2_wave_cohorts`; `ai_1_1` and
`ai11` resolve to `ai_1_1_tank_mg`; `ai_1_0`, `ai_1_0_tech`, and `ai1` resolve to
`ai_1_0_tech`; `ai` and `default` resolve to the live default.
The live lobby AI uses this shared core through `AiController`, which only owns live identity,
profile id, cadence, and persistent decision memory. Unknown live profile ids resolve to the
highest supported live AI version, currently `ai_1_2_wave_cohorts`. The ordinary lobby exposes
AI 1.0, AI 1.1, and AI 1.2; older experimental profile ids are no longer listed or accepted by
developer tooling. AI 1.2 is the live lobby default.

**Self-play scorecards.** The `ai-matchup` and `ai-balance-matrix` developer tools emit
profile-agnostic baseline scorecards from public self-play commands and snapshots. Per-player
results include army value, building value, final worker count, final unit counts, command count,
attack command count, damage events dealt, deduplicated deaths, first attack command, first
Rifleman attack command, first Scout Car completion, first legacy Scout Car harassment-style `Move`
command, first expansion City Centre planned/completed, and first Tank completion. Match-level results include
winner or tick-cap status, first damage, attack events, death events, replay verification status,
and optional replay artifact path. Compact baseline scenario metadata for AI 1.0 early production,
tech-blocked production, Scout Car unlock, and Tank unlock lives in
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
