## 8. AI opponents (optional, `server/crates/ai`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). `addAi` accepts an optional `teamId` for
scripted team setup; when omitted, the server seats the AI into the next deterministic slot for
the current preset. Use lab rooms for player-facing experimentation until debug-style starts return
as explicit lab presets or scenarios. They are capped with humans at
`MAX_PLAYERS = 4`, and the selected authored map may impose a lower active-seat cap through its
available spawn-layout player counts.
AI players are seated after the humans in the lobby player list; their colors come
from the tail of `PLAYER_PALETTE` so they never collide with human colors. They persist across rematches and are cleared only when the room
empties of humans.
Their display names use the selected live profile label (`AI 1.0`, `AI 1.1`, `AI 1.2`);
multiple seats on the same profile receive deterministic numeric suffixes in lobby order.

**Where it runs.** `rts-ai` owns one `AiController` per AI player, while `Game` remains AI-free.
The room task invokes controllers before `game.tick()`, gives each controller the same
fog-filtered `snapshot_for(player)` plus the static `start_payload()`, then enqueues emitted
ordinary `SimCommand`s. Every AI action therefore goes through the identical validation / cost /
supply / placement path in `services/commands.rs` — the AI has **no special authority** over the
simulation and can't cheat economy, placement, or fog rules. For oil assignments, the controller
budgets Pump Jack construction like other paid buildings before reserving workers or oil nodes.
Outbound attacks target enemy
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
decisions to `rts_ai::ai_core::decision::decide_profile_with_analysis`, which requires the
AI-owned static map analysis for production callers. Live lobby AIs use the promoted
AI 1.2 suite by default and keep the resolved concrete profile for the whole match. Hosts can
select the `ai_1_0`, `ai_1_1`, `ai_1_2`, `ai_2_0`, `ai_2_1`, or `ai_turtle` profile suites per AI seat
from the lobby before countdown/start; exact concrete profile ids remain accepted for developer
compatibility.
Unsupported profile or suite ids are ignored or defaulted to the promoted live default request.
Team relationships are observation-only safety
inputs: player summaries carry `teamId`, visible allied entities are classified separately from
`visible_enemies`, public base targeting ignores allied starts, and live decisions receive the
current living player set so attack waves keep choosing living enemies. AI teammates still do not
share economy, production, command authority, build orders, attack plans, or a team controller.
It does not micro, scout, or choose hidden enemy unit positions. A local per-think budget in the
shared action layer prevents it from over-committing resources/supply it does not have.

**Shared AI core.** `rts_ai::ai_core` has deterministic profile data (`profiles.rs`) and a generic
ranked decision loop (`decision.rs`) that emits ordinary `SimCommand`s through shared action helpers.
It also owns static map analysis (`map_analysis.rs`) built only from `StartPayload.map`,
start tiles, and static resource nodes. `AiStaticMapContextCache` is the shared cache used by
live AI and self-play scripts, keyed by a stable map/start/resource identity, so profile decisions
do not have to remember to pass optional choke data separately. The analyzer records terrain
passability, centered clearance, passable components, 10-clearance open-region seeds grown to
5-clearance shoulders, region assignments for starts/resources, and <=4-clearance choke bands split
into region-pair corridors from contact-front distances. Regions are an internal implementation
detail for assigning starts/resources and
connecting choke cuts. Each published choke exposes one generated endpoint-to-endpoint line over
the full passable choke band, including stepped or diagonal bands, while retaining the underlying
tiles only as analyzer evidence/statistics; observer diagnostics intentionally do not expose
the tile evidence band or regions as map layers. The
previous Voronoi-style diagnostic layer was removed because it did not match the gameplay choke
definition. The turtle profile consumes the cached choke geometry to choose public, static
own-base defensive lines; other promoted profiles still use the analysis only for diagnostics.
Live spectator observer diagnostics can expose this cached analysis as `observerAnalysis.mapAnalysis`
overlay primitives: generated choke lines with approach markers, base markers, resource-cluster
markers, and labels. Turtle profiles also append live spectator-only plan
layers showing the defended choke lines, Machine Gunner coverage slots, Anti-Tank Gun backlines, and
setup-facing rays that their current decision code is using, with short labels and hover
explanations. That payload is derived from the same AI-owned cache and
remains spectator-only; active players and AI command logic do not receive a new authority surface
from it.
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

Profile experiments are registered as concrete `AiProfile` entries with stable ids and manifest
metadata. The manifest records module names and a fingerprint for arena artifacts, keeping promoted
variants inspectable without changing the live lobby contract.

The economy plan is backed by an AI-owned resource availability model derived only from the
fog-filtered observation, public start-payload resource positions, completed own City Centres,
visible resource deltas, current worker latches, and AI-owned reservations. The model keeps
known resources separate from resources that are mineable now: a steel or oil node is assignable
only when it has remaining resources, is in range of a completed own City Centre, is not occupied by
a latched worker or owned Pump Jack extractor, and is not already reserved by the current think.
Known but non-mineable nodes remain visible to expansion planning as future candidates, but economy
worker assignment suppresses oil demand when there is no free mineable oil and passes only free
mineable node ids to `assign_workers_to_resource`. The action layer also requires callers to provide
that assignable set; steel assignments emit `Gather`, while oil assignments emit contextual
`Build pump_jack` commands instead of direct oil gather commands. Post-expansion assignment prefers
workers near the expansion resource line, and profiles that opt into remote fallback can still send
a main-base idle worker to the expansion once the main line is saturated instead of leaving it idle.
Self-play regression coverage preserves the pre-expansion case where oil is known but outside
completed-City-Centre mining range, and the post-expansion case where oil assignment begins after the
expansion City Centre completes.
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
Gun Works plus Medium Guns research. It may build Pump Jacks for those support counters; if
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
defensive Machine Gunner group, pushes toward full two-base steel saturation, and stays capped at
one Factory. Vehicle Works and Gun Works placement uses a modest expanded center-facing search band
so support and vehicle production do not pile up behind the base. Worker queues reserve economy
before normal tech, production, or combat spending while below the main-plus-natural saturation
target, except during defensive panic or supply-depot handling. Its
Tank-era production and frontal-wave composition are Tank-only, so Riflemen remain an
opening/defensive Barracks output rather than a continuing mid-game spend. It reserves up to four
ready Machine Gunners before frontal-wave readiness is calculated, so those MGs do not satisfy Tank
wave sizes.
Tank-pivot transitions are gated by floated steel and oil rather than live supply count. This lets
AI 1.0, AI 1.1, and AI 1.2 advance into Training Centre, Research Complex, Factory, and Tank
production after low-supply attrition stalls, while avoiding a transition just because a large army
survived. Once a transition-only tech building is owned or pending, the transition remains active
after the first resource spend.
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
units that are excluded from outbound wave formation. AI 1.2 also targets a second Vehicle Works once
its bank is above 600 steel and 400 oil, while still using the normal build placement, prerequisite,
pending-build, expansion-save, and defensive-panic gates.

AI 2.0 is exposed as the `ai_2_0` suite rather than a single inspectable lobby target. The promoted
suite is currently pinned to `ai_2_0_tank_pressure`, which pivots earlier into faster mixed
Tank/Rifleman pressure. It expands off the shared two-base economy plan, reserves defensive Machine
Gunners, unlocks Factory production earlier than AI 1.2, and targets a second Factory once the
economy can support it. Exact concrete profile ids remain registered for arena pinning, replay
debugging, and profile-manifest fingerprints. The retired `ai_2_0_agent_rush` and
`ai_2_0_rifle_tank` profile ids remain rejected.

AI 2.1 is exposed as the `ai_2_1` suite and is currently pinned to
`ai_2_1_economy_manager`. It intentionally keeps AI 2.0's worker, supply, expansion, oil,
production, defensive Machine Gunner, frontal-wave, and tech-transition policy values, but routes
economy decisions through the proposal-based economy manager. The manager consumes the ordinary
fog-filtered observation, `AiFacts`, active profile policy, expansion plan, and owner-provided
signals such as defensive-panic oil demand or temporary oil holds. It returns economic proposals
for supply, expansion, worker training, oil assignment/Pump Jacks, and steel assignment; the owner
decision loop still executes or rejects those proposals through `AiActionContext`, shared budgets,
worker/resource reservations, placement validation, and ordinary `SimCommand` emission. AI 2.1 is a
parity refactor target for AI 2.0 rather than an intended balance upgrade until matchup evidence and
human review say otherwise.

The `ai_turtle` suite is pinned to `ai_turtle_chokes`, a first-pass turtle profile for visual
matchups and tuning. It uses the proposal-based economy manager to keep its supply, worker-training,
resource-assignment, and two-City-Centre expansion behavior aligned with AI 2.0 while preserving
its three-Rifleman opening oil hold. It targets full main-base worker saturation, opens one Barracks,
builds Training Centre, then adds a second Barracks once steel exceeds 450 while it accelerates
Research Complex / Steelworks R&D for Anti-Tank Guns. After its first completed Gun Works, it adds a
second once the bank exceeds 600 steel and 250 oil. It queues Entrenchment from Training Centre
before Machine Gunner production,
starts the R&D chain once Entrenchment is queued, and prioritizes the Anti-Tank Gun unlock before
downstream construction spend when Research Complex is ready. Its opening combat plan trains exactly
three Riflemen from the first Barracks and sends them to a compact defensive line in front of the
main steel cluster, while delaying oil assignment and tech buildings beyond Barracks until those
opening Riflemen have been ordered. After that opening, it starts oil earlier than ordinary
full-steel saturation, techs to Training Centre, queues Entrenchment, then can begin Research
Complex and Steelworks before Entrenchment completes. Its Steelworks uses a city-center-facing
nine-tile search band (half the ordinary forward production range) and keeps two clear tiles from
other buildings, because its support weapons are mobile. Its economy-manager two-City-Centre
expansion flow activates after Training Centre and the normal steel-or-supply trigger are met,
regardless of opening Rifleman attrition, capping pre-expand steel workers at 18 and lifting the
post-expand steel target toward 36 so both City Centres can keep producing workers. Its post-opening
production is pure Machine Gunners from up to two Barracks plus priority Anti-Tank Guns from up to
two Steelworks;
Riflemen are not replenished after the three-unit opening. Machine Gunner production pauses once the
first two enemy-facing choke lines each have four staffed Machine Gunners, and resumes if either line
falls below four; those Machine Gunners use a wider three-tile
slot spacing around each choke line. When idle and not handling a local visible threat, it selects up
to three chokes adjacent to its own start region from cached static map analysis, uses each choke's
generated full-band line endpoints as the defended line, orders those exits by direct distance from
each choke line to the public enemy start, and treats the closest line as the main choke. Cross spawns staff
only that main enemy-facing choke early, while close spawns staff the first two enemy-facing own-base
chokes. Once
Entrenchment is researched and two Machine Gunners are holding the main choke, fresh Machine Gunners
and Anti-Tank Guns can reinforce all configured active chokes by current under-staffing and width
proportion, so replacements flow toward emptied lines before already-staffed ones. Machine Gunners
occupy coverage slots along the full choke line at roughly three-tile gaps: the first slot is seeded
from the sector where the public enemy-start to own-start route crosses or comes closest to that
choke line, then later slots maximize empty space from already chosen sectors. They switch to Hold
Position after reaching their assigned slot. Riflemen do not staff chokes; they stay on the main
steel-line screen as a fallback against missed routes. Anti-Tank Guns use the same coverage-slot
selection on a line ten tiles behind the averaged choke line on the own-start side, then set up
facing orthogonally down the enemy approach lane. It does not launch ordinary frontal waves.

The suite aliases `ai_2_1` and `ai21` resolve to the AI 2.1 suite request, `ai_2_0` and `ai20`
resolve to the AI 2.0 suite request in live and arena-style tooling; `ai_1_2` and `ai12` resolve
to the AI 1.2 suite request; `ai_1_1` and `ai11` resolve to the AI 1.1 suite request; `ai_1_0` and
`ai1` resolve to the AI 1.0 suite request; `ai_turtle` and `turtle` resolve to the turtle suite
request. Exact profile ids such as `ai_1_2_wave_cohorts`, `ai_2_0_tank_pressure`,
`ai_2_1_economy_manager`, or `ai_turtle_chokes` pin one concrete member. `ai` and `default` resolve
to the promoted live default request.
The live lobby AI uses this shared core through `AiController`, which only owns live identity,
profile id, cadence, persistent decision memory, and its latest bounded decision trace for
spectator-only observer diagnostics. Unknown live profile ids resolve to the promoted live default,
currently `ai_1_2`, which resolves to `ai_1_2_wave_cohorts`. The ordinary lobby exposes AI 1.0,
AI 1.1, AI 1.2, AI 2.0, AI 2.1, and AI Turtle as suite requests. AI 1.2 is the live lobby default.
Panzerfaust is trainable for Kriegsia players after a completed Training Centre. Scout Plane is a
Command Car world-point ability that launches from an owned completed City Centre, but current AI
profiles intentionally omit Panzerfaust training and Scout Plane ability usage in the first pass.
AI-owned Panzerfaust or Scout Plane units spawned by lab/dev setup still use their normal simulation
behavior.

**Self-play scorecards.** The `ai-matchup`, `ai-arena`, and `ai-balance-matrix` developer tools emit
profile-agnostic baseline scorecards from public self-play commands and snapshots. Per-player
results include army value, building value, final worker count, final unit counts, command count,
attack command count, damage events dealt, deduplicated deaths, first attack command, first
Rifleman attack command, first Scout Car completion, first legacy Scout Car harassment-style `Move`
command, first expansion City Centre planned/completed, and first Tank completion. Match-level results include
the captured starting City Centre objective ids/death ticks, winner or draw status, first damage,
attack events, death events, replay verification status, and optional replay artifact path.
AI-vs-AI profile matchups declare a winner only when a profile destroys the enemy's starting City
Centre first; if no starting City Centre winner exists by the default 25,000-tick fixed horizon,
the matchup is a draw.
Material, army, building, worker, damage, and survival metrics are diagnostics only, never
tiebreakers. Compact baseline scenario metadata for AI 1.0 early production,
tech-blocked production, Scout Car unlock, and Tank unlock lives in
`server/crates/ai/src/selfplay/scenarios.rs` so later AI changes can compare the same authored
fixtures without rewriting the harness.
Profile matchup JSON also includes a bounded `aiTraceTail` of compact trace entries for recent
profile-backed thinks. The tail is diagnostic output only; deterministic replay artifacts continue
to use the command log as the source of player intent.
Arena runs wrap the same profile matchup result in agent-facing sidecars: `manifest.json` records
git, seed, side, requested profile/suite ids, resolved concrete profile identities, and module
fingerprints; `summary.json` stores the machine-readable result row; `decision-trace.jsonl` indexes
compact trace labels by tick and player; and `brief.md` gives a short textual match brief with
replay and trace pointers.

Spectators never count toward win/elimination and receive a neutral final scoreboard result.

**Observed live match horizon.** A normal live room with two or more AI seats and no active human
seats is treated as an AI observation session. The live stream stays interactive for spectators and
continues to use the normal post-match replay path, but it must resolve no later than tick 25,000.
If a starting primary base is eliminated on that tick, the normal winner takes precedence;
otherwise the server records a draw. The score screen shows the generated Observation ID, which
matches the persisted replay lookup and structured server lag logs so a reported behavior can be
reconstructed after the watcher leaves.

**Win/elimination.** AI players count as match players: a 1-human + N-AI match is a real match
(it resolves to a winner), while a lone human with no AI remains a never-ending sandbox. AI-only
live matches use the same strategic objective as profile-vs-profile tooling: a player loses when
its starting main base is gone, so a surviving expansion City Centre or other base does not keep
that AI alive. Mixed human/AI matches keep the ordinary live elimination rule; there, an AI with no
units left is defeated even if it still owns buildings, because it has no player input path back
into the game. The lobby's `match_player_count` is humans **+** AIs.

---
