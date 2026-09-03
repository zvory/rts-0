## 8. AI opponents (optional, server/crates/ai)

Computer opponents are opt-in: a room has none until its host adds one in the lobby. Hosts can
add, remove, move, and select AI seats only during the lobby phase. AI seats count toward the
normal four-player cap and toward any lower active-seat cap imposed by the selected map.

AI players are seated after human players, use colors from the tail of PLAYER_PALETTE, persist
through rematches, and are removed only when the room empties of humans. They are always ready.
When several seats use the same profile, their lobby names receive deterministic numeric suffixes.

### Canonical profiles

The player-facing lobby supports two AI profile IDs:

- ai_2_1 — AI 2.1, the default pressure profile.
- jeffs_ai — Jeff's AI, the server-authoritative port of the locally evaluated V3 champion policy.

`ai_turtle` is deprecated and internal-only. It remains registered for offline self-play,
diagnostics, and observer-only AI sessions, but it is not exposed in the lobby and the server
replaces it with `ai_2_1` whenever a room has an active human player. The replacement is enforced
again at the authoritative match-start seam, so stale or crafted clients cannot put Turtle into a
human match.

Those IDs are concrete match profile IDs used by controllers and diagnostics. Live AI selection
also accepts suite request IDs, such as ai_2_0, which resolve to concrete profiles for a match.
The convenience inputs ai and default resolve to ai_2_1. The lobby exposes AI 2.1 and Jeff's AI;
unsupported or internal profile IDs fall back to AI 2.1 when adding a seat and are ignored when
changing a seat in a player lobby.

Internal observer launch URLs may still use both IDs. For example, a spectator can launch an AI
2.1 versus Turtle diagnostic match with:

    /?rtsLaunch=match&rtsRoom=agent-ai-selfplay&rtsRole=spectator&rtsAi=1:ai_2_1&rtsAi=2:ai_turtle&rtsStart=1

### Where it runs

rts-ai owns one AiController per AI player, while Game remains AI-free. The
`CanonicalAiTickDriver` is the one normal live/offline profile host: before `game.tick()` it chooses
the host-requested normal or starting-primary-base alive policy, captures each controller's public
start payload, fog-filtered `snapshot_for(player)`, and worker-retreat input in stable controller
order, collects every result before enqueueing any, then enqueues ordinary `SimCommand`s in that
same order. The driver does not tick the game or own outcome, networking, replay, scoring, or
artifact policy. AI actions therefore go through the same validation, costs, supply, placement,
and fog rules as human commands; the AI has no simulation authority of its own.

Outbound attacks use public enemy start tiles. Direct attack targets are limited to currently
visible entities. The worker direct-hit retreat reflex projects recent own-worker damage into
ordinary Move commands without reading private simulation state.

rts-ai may depend on the public simulation API, rules, protocol, and contract crates. It must not
import the server shell, lobby internals, transport layer, or private simulation modules. New AI
observations must be added as a public fog-respecting Game or snapshot surface.

### Typed authoring SDK

`rts_ai::sdk` is the supported Rust authoring seam for custom strategies. An `AiController` built
with `AiController::with_strategy(player, Box<dyn AiStrategy>)` runs in the same canonical driver
as profile controllers. The object-safe `AiStrategy: Send` lifecycle calls `initialize` exactly
once immediately before the first `step`; both calls occur only on the player's normal nine-tick,
staggered decision cadence. Strategies have no required constructor, async lifecycle, associated
types, checkpoint contract, or direct simulation access.

`AiFrame` is an owned, read-only normalization of public start data, the player's
`snapshot_for(player)` result, the host-selected alive-player set, and controller-inferred
submitted-build bookkeeping. The frame adapter is the only production path that parses raw
start/snapshot DTOs into strategy-visible kind, state, production, upgrade, terrain, resource, and
memory facts. Its collections use stable id ordering; completed upgrades and submitted builds are
additionally sorted and deduplicated.
Active production identifies either its typed unit kind or its typed research upgrade when one is
present in the recipient snapshot.
Owned entities, currently visible allies, currently visible enemies, and remembered contacts are
separate collections. A remembered contact is stale last-seen knowledge, never a current target.

Static resource locations are public, but `AiResourceAmount::Unknown` is retained until a
recipient-scoped resource delta or visible node reveals a quantity. The public frame never exposes
the historical synthetic `remaining = 1` value or the policy-derived `free_for_combat` flag.
Opponent production queue length/payment state and non-owned construction activity are likewise
`None`, rather than false or zero, because those details are redacted by the player snapshot.
Likewise, `AiBuildObservation` means only that this controller inferred an outstanding submitted
build; it is not an accepted, legal, or active-build receipt.

`AiActions` is a typed per-think builder that retains at most 256 emitted actions in helper-call
order. Its supported helpers are paid explicit-site build, resume-without-repay, train, standing
production repeat, research, gather, move, attack-move, direct attack, Hold Position, and Anti-Tank
Gun setup. `UnitGroup`
canonicalizes tactical unit IDs into a sorted, deduplicated, non-empty set; caller-ordered worker,
resource-node, and producer candidate lists are not normalized.

The builder tracks Steel, Oil, and free Supply plus independent actor, resource-node, and producer
reservation namespaces. Every helper completes local preflight before changing the budget,
reservations, batch, or trace. `ActionError` exposes an `ActionBlocker` only for locally known
facts: empty input, unsupported kind, no compatible producer, insufficient same-think budget, an
already reserved id, no known candidate, or the action cap. Local success means only that an
SDK-owned action was emitted. It is not simulation acceptance, legality, completion, or atomicity,
and there are no intent IDs or task statuses.

The runtime emitter is the sole translator from SDK-owned actions to ordinary `SimCommand`s after
the strategy returns. Built-in profiles share the same budget/reservation implementations, typed
accumulation, and emitter behind their compatibility action context. Their tactical helpers retain
the historical lack of automatic tactical reservations, while production/economy paths retain
worker and resource-node candidate order, producer order, Pump Jack payment, resume semantics,
queue flags, command traces, and Jeff's Steel-only cross-tick submitted-build commitment. Canonical
controller ordering, all-controller observation before enqueue, worker-retreat-first ordering,
normal simulation validation, and replay logging are unchanged. The builder is deliberately not a
planner, task system, command receipt, or cross-tick scheduler.

The public-SDK-only [reference strategy](../ai-authoring.md) is the executable authoring specimen.
It runs through `AiController::with_strategy` and the canonical driver, enables Pump Jack repeat,
uses `UnitGroup` plus cross-tick memory to dispatch its Engineer as an attack-move scout, and is
covered by deterministic command-log and replay checks. The example is not a selectable profile or
a strength claim.

`AiRulebook` binds the authoritative `rts-rules` faction catalog to a strategy frame. Its bounded
answers cover catalog order and availability, costs, supply, production time, health, footprint,
builders, producers, prerequisites, gather capability, and train/research relationships; it owns
no duplicate definitions. Upgrade costs/timing remain absent because their current owner is
`rts-sim`, and the SDK does not move that authority merely to fill a catalog row.

`WorldQueries` builds stable-id indexes over the frame's owned entities, currently visible allies
and enemies, remembered contacts, and public known resources. It keeps current and remembered
knowledge distinct, filters only explicitly exhausted resources, reports known mining conflicts,
and uses stable-id tie-breaking after squared-`f32` distance for nearest selection. Checked world
and tile helpers reject non-finite, overflowing, or out-of-bounds coordinates.

Known building placement reuses the established AI footprint, public-resource, visible-building,
production-exit, ring traversal, and tie-breaking approximation. Results are named `Invalid`,
`KnownBlocked`, and `NoKnownConflict`; none claims authoritative legality, clearance, reachability,
or command acceptance. Public queries depend only on `AiFrame` and explicit controller-owned
failed-site exclusions. Legacy profiles retain their historical global-producer exit check through
a narrow compatibility policy, while all other placement work is shared. Static connectivity,
dynamic pathing, firing lanes, target legality, line of fire, and hidden occupancy remain outside
the SDK because no Phase 4 consumer required them.

### Shared decision core

Each controller runs on a staggered cadence and constructs the typed `AiFrame`. Built-in profiles
run through the crate-private `LegacyProfileStrategy`, which projects that frame back into the
internal `AiObservation` before the generic decision loop applies the selected `AiProfile` policy.
That compatibility projection alone preserves the historical false `is_ai` value, synthetic
unknown-resource amount, `free_for_combat` derivation, and old sorting/filtering rules; these quirks
are not represented as truthful SDK facts. The direct legacy observation constructor remains only
as a test oracle for field-for-field projection checks. Existing action helpers use the SDK-owned
typed action vocabulary and shared local per-think budget and reservation implementations to
prevent resource and supply overcommitment without changing compatibility policy.

The core also owns static map analysis derived only from StartPayload map terrain, start tiles, and
static resource nodes. When nearby steel is split into fields around the Resource Depot, defensive
staging and Rifleman raid readiness use the field on the map-center side, falling back to the full
steel cluster for degenerate layouts. Start and resource-cluster mappings prefer candidates in the same reachable
terrain component when component identity is known, with distance as the fallback for unknown
components. AiStaticMapContextCache keys that analysis by stable terrain, start, and resource
identity, so a Lab map edit naturally causes the next think to rebuild passability, clearance,
regions, chokepoints, starts, and resource analysis. Gameplay-choke detection uses local minimum vertex cuts between high-clearance basins for broad
middle passages and split-validated linear cuts across bounded passable runs for base mouths.
Ranked graph-cut candidates are scanned until the target count is filled, skipping candidates that
cannot be mapped. Region pairs come from region-bearing passable sides of the local split, with
basin metadata as a fallback only for basin-backed candidates. Default and Low Econ each expose
twelve gameplay chokes. The published observer layers show
generated choke lines, base markers, resource-cluster markers, and labels; regions remain internal.
The offline ai-map-analysis-debug tool loads bundled maps through the simulation map loader, runs
the same static analysis, and renders the observer layers over terrain as SVG. Its choke overlay
renders the exact detected choke tiles rather than choke bounding rectangles.

The economy model is also observation-owned. Engineers are construction-only and the built-in
profiles retain only their starting Engineer plus any explicitly configured extra builder. On the
first decision after each Resource Depot completes, the controller idempotently enables Steel Mine
and Pump Jack repeat production there. The authoritative Depot queue chooses eligible in-range
patches, pauses a saturated extractor kind, advances to the other repeated kind when possible, and
resumes replacement production after an extractor is destroyed. Resource availability counts only
completed, live extractors as current income while treating an in-progress scaffold as patch
occupancy. Expansion planning can still see known resources outside current Depot coverage.

Decision traces record the selected profile ID, tick, budget and reservation deltas, strategic
goals for economy, supply, expansion, tech, production, local defense, and frontal attack, plus
bounded command and blocker labels. Each live AI controller exposes its latest decision trace to
spectators, with the reliable-channel snapshot bounded at the AI adapter boundary. These traces
and map-analysis layers are spectator-only diagnostics.

### Profile behavior

AI 2.0 resolves only to the `ai_2_0_tank_pressure` profile. The retired
`ai_2_0_rifle_tank` profile is not registered or accepted as an exact selectable profile. Defensive
panic does not override an already-active tech transition, so tank pressure continues its Factory
path during pressure.

AI 2.1 is the promoted pressure profile. It fills in-range Steel and Oil extractor slots over time,
keeps an eight-supply buffer, opens one Barracks, expands to two Resource Depots, and reserves four
Machine Gunners for defense. It begins with Rifleman pressure, then transitions into mixed
Tank/Rifleman pressure once its tank-tech resource threshold is met. At a larger resource float it
adds a second Factory. Frontal waves stage in cohorts so newly produced units do not immediately
join an already-launched wave.

AI Turtle shares AI 2.1 extractor, supply, and first-Barracks cadence, but uses a two-Rifleman
opening and does not launch frontal waves. It prioritizes a Training Centre, an early second Resource
Depot, Entrenchment, support technology, Machine Gunners, and Anti-Tank Guns. It identifies up
to three own-base chokepoints from the static map analysis, caps Machine Gunner production by
planned choke-line staffing, staffs the active enemy-facing lines with Machine Gunners, and places
Anti-Tank Guns on an own-side backline. Its staged intents include the two-Rifleman opening. The profile prioritizes
the main choke first, can defend a second close-spawn choke, and reinforces under-staffed lines.
Staged defenders emit HoldPosition once after reaching their defensive slot rather than repeating
the command on every think.

All three profiles are self-contained policy records in the same registry. Each profile selects
whether to use the proposal economy manager; AI 2.1, Jeff's AI, and AI Turtle enable it. None
inherits behavior from a retired version or resolves through a second profile name.

Jeff's AI is the fast-Tank timing profile developed from the standalone local bot workspace. Its
four oldest home Riflemen and two opening Machine Gunners form a six-slot defensive pocket around
the starting Resource Depot: the two outside Riflemen remain on the close flanks, the middle pair
stands forward, and the Machine Gunners occupy mirrored slots slightly behind that middle pair.
The whole pocket rotates toward a terrain-analyzed central base approach when one exists. Crossroads
uses a map-specific wall-aware rotation for both starts: its pocket faces the southwest approach
corridor around the water barriers rather than the blocked direct diagonal or the map centre. Other
open or side-lane starts retain the same shape and rotate toward the map centre.
Later surplus Riflemen keep the broader building-envelope coverage instead of crowding the opening
pocket. A Machine Gunner below 50% health no longer counts toward the healthy pair and triggers a
replacement. The full Tank tech path starts immediately, Tanks do not wait for Methamphetamines,
and defensive panic does not replace the active armored production plan with infantry. After two
Tanks, one Scout Car takes temporary Factory priority and joins those Tanks as a single
vision-supported attack wave. Expansion, a second Factory, Methamphetamines, and other optional
spending wait until the initial Tank core exists. The profile uses the shared decision and action
layers, receives only fog-filtered observations, and issues ordinary validated player commands for
spending, placement, production, and combat.

On The River, Jeff's first expansion uses the upper-right verified Resource Depot footprint at tile
`(108, 93)` and its footprint-aware rotation `(15, 30)` from the lower-left start. The map spawns
both Workers east of their starting depot rather than rotating the Worker position, so the
lower-left instruction first moves its builder to the rotated upper-right approach tile and queues
the build. Both sides retry only the verified footprint instead of scanning adjacent blocked tiles.
The override is identified by the map dimensions, start tile, and complete twelve-Steel/three-Oil
natural cluster, so the 1v1 map's matching dimensions and start tiles continue to use ordinary
expansion search.

Its local-defense envelope covers every owned building footprint, including incomplete structures,
plus reserved sites for submitted build intents. The four-unit defensive pocket remains anchored to
the starting Resource Depot so later construction cannot drag it out of shape; only surplus
Riflemen use completed core buildings for their standing coverage. When visible enemy material
enters the wider envelope, mobile defenders focus the highest-value visible target at the
threatened footprint. Tanks and Panzerfausts answer armored contacts first, fully entrenched
Riflemen remain in place, and an understrength Rifle-only group does not make a sacrificial
intercept. Lost contact creates only a bounded two-second search incident; reaching the last
contact point without reacquiring the enemy or reaching the timeout returns the units to normal
defensive staging. A containment push may start once its two-Tank, one-Scout-Car core and two nearby
Riflemen are available even if the generic frontal-wave size is not yet filled. The four oldest
Riflemen remain reserved for the home pocket. Escort selection uses only other completed,
free-for-combat Riflemen within twelve tiles of the group, takes at least two
when available and up to half of those candidates, capped at six, and reselects nearby escorts when
assembly times out rather than waiting on a distant reservation. The screen stands two tiles ahead
of the Tanks with two-tile lateral spacing; groups larger than four use a staggered second rank so
the screen covers the Tank frontage without putting all six Riflemen into one line.

Before departure, both Tanks and the Scout Car assemble around the Tanks' center. Exact formation
slots can launch immediately; after eight seconds the compact vehicle core plus two nearby
Riflemen is sufficient, and after twelve seconds the compact vehicle core is a hard upper bound on
assembly even if no usable screen exists. On The River, the opening group then guards its mirrored
rally for thirty seconds and requires five clear seconds after nearby contact before beginning the
crossing, giving forward pressure a chance to meet the grouped force while Smoke is available.
At the more exposed lower-left natural, any defensive incident that commits a Tank also commits its
available Tank partner and two available Riflemen; upper-right keeps its established response.
The group follows cached passability and clearance-aware
six-tile route bounds instead of straight-line hops. A four-second waypoint timeout drops distant
Rifleman laggards and advances only after the Tanks are compact and the Scout Car is close. Formation
commands refresh at most every two seconds while a waypoint is unchanged. On contact, the Tanks
stop as a shared firing core, the Riflemen occupy their spread screen, and the Scout Car remains
behind; a two-second contact memory prevents rapid movement/hold oscillation. A material home-defense
panic recalls every surviving member of the active cohort to the visible local threat and cancels
its outbound route. This active-cohort control bypasses the ordinary new-wave exclusion window so
the launched force remains under formation control without admitting newly produced units. The internal
`jeffs_ai_beta_967078d` profile preserves the beta build's prior frontal controller for local arena
comparisons. `jeffs_ai_pre_defense_envelope` freezes the preceding Jeff defense policy; neither is
exposed in the lobby selector.

After the three-second stationary-range ramp, Jeff issues explicit Tank attacks only against a
visible target that every Tank can reach from its current position. An out-of-range retained target
is cancelled with Hold Position before it can pull one Tank out of the firing core. Anti-Tank Guns,
Tanks, and Panzerfausts lead the target order; killable and lower-HP targets break equal-priority
ties, and each 60-damage Tank shot is allocated only while the target still needs another shot.
Rifle escorts independently cover bounded lateral sectors around their assigned screen slots,
prioritize Panzerfausts and Machine Gunners, never explicitly chase Tanks, and return to their slot
when no soft threat is inside the four-tile leash.

The Scout Car may use Smoke after the Engineering Complex prerequisite and an actual smoke charge
are available and both the cast range and resulting sight lines are safe. The outbound formation
also requires a stable Tank focus; a local-defense incident may react immediately so incoming Tank
pressure does not bypass Smoke. Defensive Smoke is limited to a selected one- or two-Tank
interceptor core; larger defensive Tank groups retain ordinary target acquisition rather than
having their whole volley rewritten around a cloud. Candidate Tanks are restricted to the local
engagement instead of sorting every globally visible Tank. The normal candidate is the healthy rear Tank while the
forward Tank remains exposed. If Jeff's Tanks are already firing on the rear Tank, the forward Tank
becomes the smoke candidate instead. Stale or split pre-command target orders do not veto the cast,
because the same decision replaces them with a coordinated volley. Immediately before the cast,
the controller rejects smoke that overlaps the exposed focus, an existing cloud, or any Tank's line
to the exposed target. Once launched, the smoked Tank is excluded from volley and local-defense
allocation and the Scout is excluded from the subsequent attack order. If only one local enemy Tank
is available, Jeff switches fire to another shared-range target when possible; otherwise it briefly
holds the grouped Tanks while Smoke gives the rifle screen a protected five-second advance window.
If the Scout is just outside range, it may temporarily move to a computed launch point capped at 3.5
tiles ahead of the Tank center and 4.5 tiles laterally. It never follows an unrestricted
out-of-range ability order or leaves that bounded envelope to force a cast.

### Self-play and arena tools

Matchup, arena, balance, `LiveSelfPlay`, real-AI tests, and live-AI performance hosts all use that
driver. `ProfileBackedScript` survives only as a thin `AiController` adapter for synthetic mixed
script fixtures; its economy-only variant is an explicitly named command-filtering wrapper. It
owns no profile decision memory, map cache, pending builds, placement search, combat-stage state,
or cadence. Synthetic `MineOnlyScript` retains its explicitly scoped six-tick harness cadence.

This cutover intentionally changes historical offline output. The old adapter invoked both players
at tick zero and then player 1/2 at ticks 5/4 modulo six, used the default away-from-center build
search, and injected no retreat reflex. Canonical invocations begin at ticks 8/7 and repeat every
nine ticks, traces exist only on those decision ticks, retreat commands remain first even between
decisions, and production placement/stage suppression applies. In the canonical seed-7
`jeffs_ai` versus `ai_2_1` 9,000-tick lane, the first applied commands are player 2 at tick 8 and
player 1 at tick 9; representative first builds are AI 2.1 Barracks `(115,12)` at tick 476 and
Jeff Pump Jack `(9,14)` at tick 513. The lane reaches the tick cap as a draw and its replay verifies
exactly; these values document the new tooling baseline, not a balance guarantee.

The ai-matchup binary runs one fixed-horizon profile-versus-profile match until a starting Resource
Depot objective win or the tick cap. A match with no objective winner at the default 25,000-tick
horizon is a draw.

    cd server
    cargo run --bin ai-matchup -- ai_2_1 ai_turtle --seed 7 --ticks 9000 --json
    cargo run --bin ai-matchup -- ai ai --ticks 25000
    cargo run --bin ai-matchup -- --list-profiles

The ai-arena binary runs side-swapped seed pairs and writes a top-level arena-summary.json plus
per-run replay.json, manifest.json, summary.json, decision-trace.jsonl, and brief.md files. Its
defaults compare AI 2.1 against AI Turtle. The manifest records canonical profile IDs and
fingerprints, rather than a requested/resolved identity pair.

    cd server
    cargo run --bin ai-arena -- --candidate ai_2_1 --baseline ai_turtle --seeds 3 --ticks 9000

The repository term **120 game test** means `scripts/120-game-test.mjs`. It requires profile IDs for
the current AI 2.1, pre-change Jeff, and post-change Jeff in that order. The runner builds ai-arena,
runs all three pairings on The River, Schone Tage, 1v1, and Crossroads with five side-swapped seeds,
caps parallel work, resumes completed seed jobs when an output directory is reused, and writes
Markdown, JSON, and CSV summaries.

    node scripts/120-game-test.mjs ai_2_1 jeffs_ai_pre_defense_envelope jeffs_ai

Per-seed console progress is off by default. For unattended runs, `--background` starts the test in
a hidden detached process and returns immediately; `--status <output-directory>` reads compact
progress from `run-config.json` without attaching to the simulation. Use `--progress` only when
per-seed terminal updates are wanted. Background output is retained in `120-game-test.log`.

Every full run also emits an agent handoff after the release build. Background launch emits it
immediately. The handoff tells an attached coding agent to stop processing, avoid status/log polls,
and resume once at a conservative all-games-hit-the-tick-cap estimate plus ten percent. It is also
persisted as `agent-handoff.json` beside the reports so a resumed task has the exact wake time,
status command, and analysis path. The estimate assumes 90 seconds for one 25,000-tick game and
caps effective parallel speedup at eight to account for arena contention; use
`--expected-game-seconds` to calibrate a materially faster or slower host. When replay verification
is enabled, the estimate conservatively budgets a second full simulation pass for every game.

Scorecards report diagnostic economy, army, building, command, attack, damage, death, and milestone
data. Material values do not break ties. Replay artifacts remain the source of player intent;
decision traces are diagnostic output only.

### Live match horizon and elimination

A normal room with at least two AI seats and no active humans is an AI observation session. Rooms
with one or zero active humans skip the pre-match countdown and start immediately. The session
remains interactive for spectators and follows the normal replay flow, but resolves no later than
tick 25,000. A primary-base elimination on that tick takes precedence; otherwise the result is a
draw.

AI players count as ordinary match players. A human-plus-AI match is a real match, while a lone
human with no AI remains a sandbox. AI-only matches use the same starting-primary-base objective
as self-play. Mixed human/AI matches use the normal live elimination rule, including eliminating an
AI that has no units left even if it still owns buildings.
