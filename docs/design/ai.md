## 8. AI opponents (optional, `game/ai.rs`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). The lobby also has a host-only
`setQuickstart` toggle labeled "Debug mode", which causes the next match to begin
with 99,999 steel and 99,999 oil for every player plus a prebuilt human-only army/base loadout.
They are capped with humans at
`MAX_PLAYERS = 4` (the hardcoded map has enough ordered `baseSites` for four starts plus neutral
expansions). AI players are seated after the humans in the lobby player list; their colors come
from the tail of `PLAYER_PALETTE` so they never collide with human colors. They persist across rematches and are cleared only when the room
empties of humans.

**Where it runs.** `Game` holds one `AiController` per AI player and drives them at the top of
`tick()`, *before* commands are applied. Each controller pushes ordinary `SimCommand`s onto the same
pending queue as translated human client input, so every AI action goes through the identical
validation / cost / supply / placement path in `services/commands.rs` — the AI has **no special authority**
over the simulation and can't cheat economy or placement rules. Because the controller is
server-side (not a network client) it reads authoritative own/resource state directly, but enemy
entities are filtered through that player's authoritative fog grid. To stay fair, outbound attacks
target enemy **start tiles**, which are public via the `start` payload; direct attacks only target
currently visible enemy units/buildings during local defense.

**Strategy (deliberately "very basic").** Each controller, on a staggered cadence
(`DECISION_INTERVAL` ticks), builds a constrained live `AiObservation` and delegates RTS decisions
to `game::ai_core::decision::decide_profile`. Live lobby AIs randomly choose one server-side
profile at match start, without a lobby protocol or UI change: `tech_to_tanks` (tank rush),
`rifle_flood_fast` (proxy rush), or `rifle_flood_full_saturation` (the previous rifle saturation
strategy). Each controller keeps its chosen profile for the whole match. It does not micro, scout,
or choose hidden enemy unit positions. A local per-think budget in the shared action layer prevents
it from over-committing resources/supply it does not have.

**Shared AI core.** `game::ai_core` has deterministic profile data (`profiles.rs`) and a generic
ranked decision loop (`decision.rs`) that emits ordinary `SimCommand`s through shared action helpers.
The first code-defined profiles are `rifle_flood_fast`, `rifle_flood_full_saturation`,
`tech_to_tanks`, and `steel_expansion_tanks`; they parameterize worker targets, supply buffers,
building/tech goals, production priorities, resource timing, expansion timing, and attack
thresholds without providing their own `think()` functions.
`rifle_flood_fast` sends exactly one reserved worker toward a hidden edge-biased proxy point near
the nearest public enemy start tile immediately, before it can afford the barracks. The transit
target stays at least 18 tiles from the enemy start, prefers map-edge footprints, and avoids the
direct own-base-to-enemy-base scouting line. If the worker was already committed when the barracks
becomes affordable, the AI places the barracks near that worker's current position rather than
waiting for the ideal edge point; if it can afford the barracks immediately, it uses the hidden
edge target as the build site. It trains only one extra home worker and sends riflemen as
individual pressure units instead of waiting for escalating waves. Pure rifle attack profiles use
AI-only raid movement: launched riflemen receive plain `Move` orders to a point deeper than the
public enemy start tile, so they ignore buildings encountered on the way, while the AI reissues
direct `Attack` commands against visible enemy units with workers first. Local home-defense
responses reserve only the defenders assigned to that threat; unrelated moving raiders keep
reacting to visible units on their own route, including scout cars, instead of continuing past
them. After a direct raid fight is cleared, raiders already away from home immediately resume the
deeper raid move instead of waiting for the next outbound wave cadence. If no enemy units are
visible and raiders have reached within 4 tiles of the center of the
enemy main-base steel line, they fall back to attacking visible buildings so a unitless opponent
can still be finished. Once the first completed Barracks has had enough time to produce seven
Riflemen, using the current Rifleman training time from shared unit stats, the profile stops
treating the rush as a pure all-in: it resumes worker production toward main steel saturation,
starts oil workers after a steel floor, adds a home
Barracks, builds a Training Centre, and switches production toward Machine Gunners / AT teams with
Riflemen as the fallback until support tech is ready. Machine Gunners come online with the Training
Centre; AT teams require the follow-up Steelworks.
`rifle_flood_full_saturation` saturates the observed main-base steel line before assigning oil
workers, so the oil timing follows the map's current steel patch count instead of a hardcoded worker
number. At 50 supply it independently pivots into the tank tech path and becomes eligible to expand
off a completed Training Centre instead of waiting for a finished Factory; expansion site
selection prefers oil coverage before extra steel output.
`tech_to_tanks` is a steel-first fast-tech profile: it keeps worker production active while saving
for the factory step, delays oil workers until at least eight workers are already mining steel,
uses ready combat units to clear visible threats in its home resource line before attacking out,
and treats a single completed tank as a valid minimum attack wave.
All profiles share a defensive panic mode. Visible enemy units near the AI's base, home resource
line, or workers temporarily suspend expansion, worker training, and non-defensive tech spending
only when their steel+oil value is at least 75% of the AI's own local unit value. While panicking,
the AI classifies the visible local threat by weapon DPS: tank-dominated pressure (75%+ of visible
local DPS) prioritizes AT teams, infantry-dominated pressure prioritizes Machine Gunners, mixed
pressure asks for a support mix, and no-DPS pressure falls back to Riflemen. Support panic only uses
already-completed support tech: Machine Gunners need a Training Centre and AT teams need a
Steelworks. It may pull workers onto oil for those support counters; if the relevant support tech
is absent, Barracks production falls back to Riflemen and panic mode does not create tech buildings.
If the pressure persists through the panic window, the AI asks for an additional Barracks before
resuming its normal profile once the threat has cleared.
`steel_expansion_tanks` is a defensive economic support profile: it saves for a second City
Centre near a neutral steel expansion before building any non-Depot tech structure. Valid
expansion sites must cover the full local resource line, then are ranked by own distance divided
by nearest living enemy-start distance so similarly close naturals prefer the base farther from
enemies. Once that expansion City Centre is planned, it builds Barracks and Training Centre tech, staffs
oil, produces Machine Gunners before Steelworks and then AT teams toward a one-for-one support mix, and keeps those
support units staged in a short line on the enemy-facing side of its main-base steel cluster
instead of launching outbound attack waves.
After 50 supply used, it switches to a Factory tech path, stops Machine Gunner / AT team
production, trains tanks, and launches outbound tank groups only once at least three tanks are
ready. After the expansion City Centre is complete, its worker resource assignment is locally bounded so
main-base workers do not walk to expansion patches, and expansion workers do not walk back to
main-base patches.
The live lobby AI uses this shared core through `AiController`, which only owns live identity,
profile id, cadence, and persistent decision memory. Profiles are still not client-selectable.

Spectators never count toward win/elimination and receive a neutral final scoreboard result.

**Win/elimination.** AI players count as match players: a 1-human + N-AI match is a real match
(it resolves to a winner), while a lone human with no AI remains a never-ending sandbox. They have
one special elimination rule: an AI with no units left is defeated even if it still owns buildings,
because it has no player input path back into the game. The lobby's `match_player_count` is humans
**+** AIs.

---

