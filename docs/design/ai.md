## 8. AI opponents (optional, server/crates/ai)

Computer opponents are opt-in: a room has none until its host adds one in the lobby. Hosts can
add, remove, move, and select AI seats only during the lobby phase. AI seats count toward the
normal four-player cap and toward any lower active-seat cap imposed by the selected map.

AI players are seated after human players, use colors from the tail of PLAYER_PALETTE, persist
through rematches, and are removed only when the room empties of humans. They are always ready.
When several seats use the same profile, their lobby names receive deterministic numeric suffixes.

### Canonical profiles

The player-facing lobby supports four AI profile IDs:

- ai_2_2 — AI 2.2, the default weighted composition: 50% Rush AI and 50% Pressure AI.
- jeffs_ai — Jeff's AI, the server-authoritative port of the locally evaluated V3 champion policy.
- rush_ai — Rush AI, an immediate infantry attack with forward construction.
- pressure_ai — Pressure AI, the AI 2.1 economy and technology with independent forward staging and a value-based response.

`ai_turtle` is deprecated and internal-only. It remains registered for offline self-play,
diagnostics, and observer-only AI sessions, but it is not exposed in the lobby and the server
replaces it with `ai_2_2` whenever a room has an active human player. The replacement is enforced
again at the authoritative match-start seam, so stale or crafted clients cannot put Turtle into a
human match.

The convenience inputs ai and default resolve to ai_2_2. The lobby exposes AI 2.2, Jeff's AI, Rush AI, and Pressure AI. AI 2.1 remains a concrete internal profile (`ai_2_1`) for offline tools and
observer-only sessions, but is hidden and rejected as a direct selection in human player lobbies.
Unsupported or internal IDs fall back to AI 2.2 when adding a seat and are ignored when changing a
seat; stale internal seats are normalized to AI 2.2 when humans join and again at match start.

`server/crates/ai/src/compositions.rs` defines compositions as weighted lists of registered concrete
profiles. Add another member and its positive integer weight to extend a composition; nested
compositions are deliberately not supported. Each controller chooses one member on its first host
invocation, before worker reflexes or strategy initialization, and keeps it for the whole match.
Selection uses a separate RNG seeded by the host's match seed and player ID, so controller order
and simulation RNG consumption cannot affect the choice. The normal and prepared host paths carry
that seed in `AiThinkContext` (and the synthetic fixture's `PlayerView`). No seed is added to the
wire or strategy observation. Equal weights mean equal probability, not forced alternation.

The lobby/request identity stays AI 2.2. Controller diagnostics expose the selected concrete
profile separately; mixed decision traces identify both the composition and member. Arena
manifests record the weighted member list, and their fingerprints include member policy
fingerprints and selector source. Explicit AI 2.1 and Rush requests preserve their existing behavior.

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
Entity records retain the public `occupied_trench_id` from that same recipient snapshot so
strategies can recognize active entrenchment without inspecting enemy research or private state.
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
production repeat, research, gather, move, attack-move, attack-move routes, direct attack, Hold Position, and Anti-Tank
Gun setup. `UnitGroup`
canonicalizes tactical unit IDs into a sorted, deduplicated, non-empty set; caller-ordered worker,
resource-node, and producer candidate lists are not normalized. `attack_move_route` reserves a
group once and emits an initial unqueued AttackMove followed by queued waypoints in order. Its
whole route must fit the local action capacity before any request or reservation changes.

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

`PullbackMemory` is an optional tactical helper shared by Jeff, Pressure, and approaching Rush
Riflemen. Its pure `plan` accepts explicit live squad facts and currently visible enemy-unit facts;
its `step` adapter reads those facts from `AiFrame` and emits SDK Moves. The default policy pulls a
member at or below 15 HP three tiles away from the nearest visible enemy when a visible enemy is
targeting it, then excludes it from normal orders for 45 ticks. At least one squad member must
remain above the threshold. Destinations are map-clamped; the helper does not establish terrain
safety or reachability. Callers choose squad membership and keep route and combat ownership.
They suppress normal orders for returned `active` IDs and restore their own movement for
`released` IDs. The adapter remembers new retreats only after local emission succeeds; this does
not imply simulation acceptance. Jeff retains his existing squad-planner invocation and early-return
semantics, including resuming ordinary advancement when its caller sees no enemies.

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
a narrow compatibility policy, while all other placement work is shared. General route queries,
dynamic pathing, firing lanes, target legality, line of fire, and hidden occupancy remain outside
the SDK because no Phase 4 consumer required them.

`AiFrame::base_sites()` exposes cached public base landmarks from the existing static map
analysis: a site for each original player start and each non-home resource cluster. Start sites
absorb their mapped home cluster; empty-resource starts still have a main landmark. IDs are
`Start(player_id)` or `ResourceCluster(cluster_id)` and remain stable while public map inputs are
unchanged. Cluster sites are inferred geography, not authoritative build footprints.
`WorldQueries::ranked_bases(player_id)` and `base(player_id, rank)` rank all sites relative to that
player's original main: main is always 1, then increasing static ground path length in world
pixels, with stable site-ID ties. The existing cached start-distance fields route around blocked
terrain with cardinal/diagonal costs of 1/1.4 tiles and forbid diagonal corner cutting. Distances
ignore dynamic units and buildings and do not model a specific unit's footprint. Each site's
`ground_distances_from_starts` contains the reachable players' distances; unreachable sites sort
last by stable ID and have `AiRankedBase::distance_from_main = None`. This is geographic order,
not acquisition order or ownership. Other players' starting sites remain in the list. Depletion, destroyed
Depots, and fog never renumber sites; a Lab map edit can rebuild them.
`known_base_depots(site_id)` associates known Resource Depots with the nearest site within twelve
tiles, using stable-ID ties. It reports owner and owned/currently visible/remembered evidence,
including last-seen tick for remembered contacts. Current evidence supersedes memory of the same
entity. Empty evidence means unknown occupancy, never confirmed vacancy. Construction sites count
as Depot evidence; none of these queries reads hidden state. The canonical host attaches the cached
landmarks before either strategy initialization or step. Pressure uses these queries to order its
expansions; other built-in profiles retain their existing placement policies.

### Shared decision core

Each controller runs on a staggered cadence and constructs the typed `AiFrame`. Legacy built-in profiles
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

Expansion sites must be more than ten tiles (center to center) from any owned or currently
visible Resource Depot, including unfinished depots and the controller’s pending Depot builds.
Patches inside a known Depot’s mining range are also excluded, so occupied sites are skipped
while the search continues through other resource locations.

### Pressure AI

`pressure_ai` owns a copy of the AI 2.1 opening and Tank technology, with independent
expansion, infantry research, production, and staging. The legacy home
response, defensive panic production override, and dedicated Machine Gunner policy are disabled
for Pressure. Other profiles retain them. Construction clearance still has priority.

The single Pressure response controller borrows mobile Riflemen, Panzerfausts, and Tanks nearest
to observed attackers until their full purchase value (Steel plus Oil) reaches at least 150% of
the remembered enemy unit value, or all available units are committed. It triggers only on visible
enemy units on Pressure's half of the map: the half-plane containing its original main, split by the perpendicular bisector
between the two starting bases. Sightings on the enemy half never create or refresh a contact,
even during offensive wave fights. When an enemy crosses that boundary, its last friendly-half
position remains remembered for 30 seconds; the response never follows refreshed positions into
enemy territory. All enemy units spotted on the friendly half contribute to the incident value;
new recruits and replacements join as needed. Response orders are AttackMove to the nearest
remembered attacker position. Each contact keeps its last visible position for 900 ticks (30 s)
after it leaves vision. When all contacts expire, surviving responders return to the current
staging line, including units borrowed from launched waves. Units already committed remain in the
response until that timeout; the controller shares a value budget across simultaneous contacts.

`server/crates/ai/src/pressure.rs` intentionally copies Rush's terrain route, stable-slot, arrival,
and cohort policy instead of calling Rush code. Its constants and persistent state are independent.
Before 4:00 the line sits 35 tiles outside the enemy start, perpendicular to the routed approach.
At 4:00 it moves to 25 tiles from the opponent's geographically ranked second base, toward
Pressure's original main. It is perpendicular to the straight main-to-target axis. At 8:00 it
moves to the opponent's third base with the same 25-tile distance and perpendicular geometry.
These SDK ranks describe public geography, not knowledge that an enemy Depot exists there.
Pressure snapshots these landmarks on its first decision for the opponent, so depletion cannot
redefine base two or three between timed phase changes.
Missing ranked sites fall back toward the enemy main without duplicate queued destinations.

Riflemen and Panzerfausts occupy the front; Tanks stand three tiles behind at three-tile spacing.
Units attack-move to clear slots, engaging enemies en route, and Hold Position after arriving.
Later lines may span blocked terrain: their center and orientation remain exact. Blocked slots and slots disconnected from Pressure’s original main
are skipped using cached static ground connectivity without diagonal corner cutting; ordinary
unit movement routes around terrain. The opening wave unlocks at 90% Methamphetamines research (or completion). Before 4:00
it launches once at least one unit arrives, with five arrivals required thereafter. From 4:00
all new waves require at least ten arrivals; from 8:00 they require twenty. The research gate
still applies if the opening wave has not launched. En-route units do not count.

A second-base wave receives AttackMove to base two followed by queued AttackMove to the main.
A third-base wave receives AttackMove to base three, then queued AttackMoves to base two and the
main. Waves launched before a time boundary retain their route. Only unlaunched recruits move
to the new staging line; launched units keep their route unless borrowed by the response controller. Pressure retains ordinary attack-move combat after launch; Rush's extractor-raiding
flow is not part of this copied staging policy. Eligible offensive Riflemen additionally use the
shared pull-back helper while staging or attacking outside the active response. Response ownership
has priority over pull-back orders. Active retreats cannot receive staging or launch orders. Released staging members resume their slots immediately; released launched members
receive their original wave route again, including its queued lower-base objectives. A later phase
boundary does not replace that saved route. Construction-clearance ownership continues to take priority. Tanks and Panzerfausts also participate
in the response.

Pressure includes a Training Centre in its initial tech path so its infantry research does not
wait for the Tank transition. On the first decision at or after two minutes (3,600 ticks), it queues
Panzerfausts on an owned Training Centre, including an unfinished or currently researching one.
Research waits for authoritative completion, prerequisites, and payment. Once the upgrade completes,
Pressure stops issuing new Rifleman orders, disables Rifleman repeat, and enables Panzerfaust repeat
on every completed Barracks, including replacements. Already paid or queued Riflemen finish normally.
Tank production remains under the existing policy; Pressure disables dedicated home Machine
Gunner production. New Panzerfausts join
Pressure's infantry staging line and its normal arrival-counted waves. This policy and its state
are independent of Rush.

Pressure expands up to five Resource Depots. It uses the SDK's player-relative ranked base sites,
skips unreachable sites, original player starts, and sites with known owned/allied/enemy or remembered Depot evidence,
and searches legal footprints covering the selected cluster's available resources. Unknown
occupancy remains unknown; ordinary authoritative placement validates each build. Blocked or
exhausted sites yield to the next usable ranked site. A map with no usable site does not halt Tank
production while saving for an impossible expansion.

The second Vehicle Works retains its three-completed-Depot gate and float above 500 Steel and
325 Oil. After both Vehicle Works complete, fourth and fifth Depots are surplus purchases at the
full 450 Steel / 100 Oil cost. Pressure keeps queuing Tanks from resources above that purchase or
a travelling Depot's committed cost; it does not pause Tank production to save for these bases.
Once five Depots are complete, the cap rises to four Vehicle Works: the same float threshold
allows one additional Vehicle Works at a time, subject to the ordinary pending-build and placement
limits. Completed Works, foundations, and Works planned in the current decision count toward the
cap; losing a Works opens a replacement slot.
All completed Vehicle Works then enable standing Tank production. Existing queues finish first;
new manual Tank orders stop so they do not compete with repeat production. New and replacement
Works receive the repeat allocation once, including after a later base loss. Extra Works beyond
two require five currently completed Depots; lost bases can be replaced up to the five-base target.

### Rush AI

`rush_ai` runs `RushStrategy` through the typed SDK and canonical controller cadence. The four
starting Riflemen join the same staging system as later infantry, facing the first living enemy
start. The starting Engineer advances toward that start between builds, stopping outside a 30-tile
exclusion radius. Movement and full construction footprints use a two-tile buffer; straight
construction approaches retain at least one tile of clearance beyond the exclusion radius. At the
standoff line, remaining buildings can be placed beside or behind the Engineer rather than
advancing closer. A near-boundary pathing correction runs before construction/travel handling. As
resources permit, the Engineer builds a Barracks, a Training Centre, then two more Barracks,
completing one building before starting the next. Failed sites are retried. Rush disables the
shared worker-retreat reflex; a lost Engineer is not replaced.

Every completed Barracks uses an attack-move rally to the staging line. Initially it alternates
Riflemen and Machine Gunners through standing production allocations. When the first Training Centre completes, all
Barracks disable Rifleman repeat without cancelling existing paid or queued Riflemen; later
Barracks start with Machine Gunner repeat only. Methamphetamines is queued on the first decision
that observes the Training Centre scaffold, followed by Entrenchment. Panzerfausts is queued on
the first decision at or after two minutes (3,600 simulation ticks), behind earlier research. Once
it completes, each Barracks alternates Machine Gunners and Panzerfausts. The home Resource Depot
auto-builds Steel Mines and Pump Jacks to sustain income. There is no expansion or additional tech
path.

Rush enables the shared wounded-unit pull-back only for Riflemen still approaching their first
staging slot. Active retreats temporarily suppress slot movement, which resumes immediately when
the retreat ends. Reaching staging hands ownership back to the existing staging and launched flow
controllers, even if a unit subsequently moves away. Machine Gunners and Panzerfausts do not use
the helper.

Rush picks a staging point 20 world-space tiles from the enemy start along a shortest octile route
through the public terrain map, without cutting blocked diagonal corners. The formation is
perpendicular to the smoothed local approach direction. Riflemen and Machine Gunners receive
stable, spaced slots from the main line's centre outward. Panzerfausts use a separate parallel
line three tiles behind it. Blocked slots are skipped without placing front infantry into the
Panzerfaust line. Only owned and currently visible buildings affect slot avoidance. Units attack-move
to their slots, engaging enemies en route, and Hold Position after arriving. The first assembled wave
attack-moves to the enemy start as soon as Methamphetamines reaches 90% research progress (or is already complete); if
nobody has arrived yet, the trigger stays unlocked until arrivals. Subsequent waves require at
least five units actually at their slots, rather than counting en-route reinforcements. Each wave
launches with one group command. Launched units are never added to a later wave or pulled back to
staging, and later production gathers into a fresh cohort.

`AiActions::queue_research` emits an ordinary research queue request without requiring or
reserving its eventual cost. Unlike the affordable `research` helper, it permits construction-
time/unpaid queues; authoritative simulation validation and payment still apply.

Launched Rush infantry raid known enemy extractors. The flow seeds reachable firing positions
around recipient-visible or remembered Pump Jacks and Steel Mines, prioritizing any safely
reachable Pump Jack over Steel Mines, then choosing the shorter safe route within that priority.
Each weapon range uses its own shared field; the AI accounts for the extractor footprint and keeps
a firing-distance margin. Visible extractors receive ordinary explicit Attack commands only from
a clear, in-range firing position. Remembered extractors are scouting destinations, never direct
attack targets. With no known extractors, the enemy start remains the exploration objective.

Observed armed enemy combat units contribute circular exclusion zones using public base range
and nominal DPS, plus the range bonus for visible trench occupation. Hidden upgrades are not
inferred. Known all-ground building blockers obstruct the route; extractors and Tank Traps remain
walkable for infantry, matching authoritative obstacle classifications. Safe firing positions
inside enemy coverage are excluded. Move waypoints suppress opportunistic combat while seeking a
safe extractor. A unit already in a fight can escape along the least-exposure route into a region
with a safe extractor approach. If no safe approach exists, the weighted field chooses a weaker
crossing, moves toward that fight without stopping on buildings, and explicitly attacks a visible
combat defender once it is in range. If no defender is currently visible, it uses AttackMove toward
the weak approach to reacquire contact. There is no separate damage-triggered re-flow or game combat-rule change.

The flow is reevaluated from fog-filtered observations on the normal AI cadence. Orders are
bounded to one per second while following the same movement mode, with immediate mode/objective
changes. Last-seen combat contacts expire after five seconds; observed noncombat contacts clear
immediately. Fields are shared by units with the same base weapon range. Only launched units are
controlled; staging cohorts, the 90% Methamphetamines opening wait, research order, and the
Engineer's 30-tile exclusion radius remain independent of raid target selection.

### Profile behavior

AI 2.0 resolves only to the `ai_2_0_tank_pressure` profile. The retired
`ai_2_0_rifle_tank` profile is not registered or accepted as an exact selectable profile. Defensive
panic does not override an already-active tech transition, so tank pressure continues its Factory
path during pressure.

AI 2.1 is the original behavior copied into Pressure, the non-Rush AI 2.2 component. It fills in-range Steel and Oil extractor slots over time,
keeps an eight-supply buffer, opens one Barracks, expands to two Resource Depots, and reserves four
Machine Gunners for defense. It begins with Rifleman pressure, then transitions into mixed
Tank/Rifleman pressure once its tank-tech resource threshold is met. After its first Vehicle Works (Factory) completes, it seeks a third Resource Depot using the
existing expansion resource/supply trigger. The third-base step does not require surviving
opening Riflemen; the second-base step retains its four-Rifleman requirement. It requires three
completed Resource Depots before adding its second Vehicle Works at the existing larger resource
float; the Vehicle Works cap remains two. The internal `ai_2_1_pre_third_base` profile freezes
AI 2.1 from main commit `69d1e29a6` for comparison. Frontal waves stage in cohorts so newly produced units do not immediately
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

The three legacy profiles are self-contained policy records in the same registry. Each profile selects
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
Later surplus Riflemen fill three lateral columns before adding deeper ranks. Their destinations
reserve at least 2.75 tiles of separation from other home Rifle slots and reject disconnected
terrain and blocked firing lanes. A Machine Gunner below 50% health no longer counts toward the healthy pair and triggers a
replacement. The full Tank tech path starts immediately, Tanks do not wait for Methamphetamines,
and defensive panic does not replace the active armored production plan with infantry. After two
Tanks, one Scout Car takes temporary Factory priority and joins those Tanks as a single
vision-supported attack wave. For active `jeffs_ai`, once the first Tank exists or is paid in the
Factory queue, the second Resource Depot becomes the next protected oil purchase. Before any
Depot build intent or affordability requirement, two Riflemen beyond the home four advance to
separate posts at the selected natural. The Barracks can fill this six-Rifle minimum without the
usual surplus-steel reserve. While the site is being secured, only the exact Resource Depot cost
is held back; resources above that reserve remain available for Tank and other production.
Construction requires
both guards at their posts and three seconds without visible enemy contact within eleven tiles;
contact or a missing guard resets that interval. The builder uses that same secured footprint.
Once the Depot is complete, the first surplus Tank outside the containment wave and home reserve
forms behind the guards and in front of the Depot. The containment wave keeps its original rally,
the reserved Tank stays at the starting base, and guards may leave their posts to answer contact
near the expansion before returning when the area clears.
On Crossroads, the expansion guards and staged Tank face the same verified route around the water
walls as the starting-base pocket instead of the blocked direct line between player starts.
On either mirrored River start, an armored incident at the mapped natural reinforces to at least
two Tanks and two Riflemen; other maps keep the ordinary value-sized incident response.
The pair stays as the expansion garrison, is replaced after losses, and is excluded from attack
escorts and home incident reassignment. A second Factory, Methamphetamines, and other optional
spending retain their opening gates. The profile uses the shared decision and action
layers, receives only fog-filtered observations, and issues ordinary validated player commands for
spending, placement, production, and combat.

On The River, Jeff's first expansion uses the upper-right verified Resource Depot footprint at tile
`(108, 93)` and its footprint-aware rotation `(15, 30)` from the lower-left start. The map spawns
both Workers east of their starting depot rather than rotating the Worker position, so the
lower-left instruction first moves its builder to the rotated upper-right approach tile and queues
the build. Both sides retry only the verified footprint instead of scanning adjacent blocked tiles.
The override is identified by the map dimensions, start tile, and complete twelve-Steel/three-Oil
natural cluster, so the Classic map's matching dimensions and start tiles continue to use ordinary
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

Before departure, both Tanks and the Scout Car assemble around the Tanks' center. Assembly uses an
explicit `Assemble` intent: its repositioning and hold commands supersede cached staging and recent
attack suppression in the live controller. The formation planner still owns the two-second retry
cadence; ordinary defensive staging remains deduplicated. Assembly does not count as launching an
attack and leaves no staging-cache entry that could block defensive retasking when assembly ends.
Spectator decision traces label this phase `assemble`. Exact formation
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
defaults compare AI 2.2 against AI Turtle. The manifest records canonical profile IDs and
fingerprints, rather than a requested/resolved identity pair.

    cd server
    cargo run --bin ai-arena -- --candidate ai_2_1 --baseline ai_turtle --seeds 3 --ticks 9000

The repository term **120 game test** means `scripts/120-game-test.mjs`. It requires profile IDs for
the current AI 2.2, pre-change Jeff, and post-change Jeff in that order. The runner builds ai-arena,
runs all three pairings on The River, Schone Tage, Classic, and Crossroads with five side-swapped seeds,
caps parallel work, resumes completed seed jobs when an output directory is reused, and writes
Markdown, JSON, and CSV summaries.

    node scripts/120-game-test.mjs ai_2_2 jeffs_ai_pre_defense_envelope jeffs_ai

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
