## 8. AI opponents (optional, `server/crates/ai`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). The lobby also has a host-only
`setQuickstart` toggle labeled "Debug mode", which causes the next match to begin
with 99,999 steel and 99,999 oil for every player plus a prebuilt human-only army/base loadout.
They are capped with humans at
`MAX_PLAYERS = 4` (the hardcoded map has enough ordered `baseSites` for four starts plus neutral
expansions). AI players are seated after the humans in the lobby player list; their colors come
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

**Strategy (deliberately "very basic").** Each controller, on a staggered cadence
(`DECISION_INTERVAL` ticks), builds a constrained snapshot-backed `AiObservation` and delegates RTS
decisions to `rts_ai::ai_core::decision::decide_profile`. Live lobby AIs randomly choose from the
server-side live profile pool at match start without a lobby protocol or UI change. Each controller
keeps its chosen profile for the whole match. It does not micro, scout,
or choose hidden enemy unit positions. A local per-think budget in the shared action layer prevents
it from over-committing resources/supply it does not have.

**Shared AI core.** `rts_ai::ai_core` has deterministic profile data (`profiles.rs`) and a generic
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
Centre; AT teams train from the follow-up Gun Works (`steelworks` kind) after AT Gun Crews
research.
`rifle_flood_full_saturation` saturates the observed main-base steel line before assigning oil
workers, so the oil timing follows the map's current steel patch count instead of a hardcoded worker
number. At 50 supply it independently pivots into the tank tech path and becomes eligible to expand
off a completed Training Centre instead of waiting for a finished Vehicle Works; expansion site
selection prefers oil coverage before extra steel output.
`tech_to_tanks` is a steel-first fast-tech profile: it keeps worker production active while saving
for the Vehicle Works step, delays oil workers until at least eight workers are already mining steel,
uses ready combat units to clear visible threats in its home resource line before attacking out,
and treats a single completed tank as a valid minimum attack wave.
All profiles share a defensive panic mode. Visible enemy units near the AI's base, home resource
line, or workers temporarily suspend expansion, worker training, and non-defensive tech spending
only when their steel+oil value is at least 75% of the AI's own local unit value. While panicking,
the AI classifies the visible local threat by weapon DPS: tank-dominated pressure (75%+ of visible
local DPS) prioritizes AT teams, infantry-dominated pressure prioritizes Machine Gunners, mixed
pressure asks for a support mix, and no-DPS pressure falls back to Riflemen. Support panic only uses
already-completed support tech: Machine Gunners need a Training Centre and AT teams need a
Gun Works plus AT Gun Crews research. It may pull workers onto oil for those support counters; if
the relevant support tech is absent, production falls back to Riflemen and panic mode does not
create tech buildings.
If the pressure persists through the panic window, the AI asks for an additional Barracks before
resuming its normal profile once the threat has cleared.
`steel_expansion_tanks` is a defensive economic support profile: it saves for a second City
Centre near a neutral steel expansion before building any non-Depot tech structure. Valid
expansion sites must cover the full local resource line, then are ranked by own distance divided
by nearest living enemy-start distance so similarly close naturals prefer the base farther from
enemies. Once that expansion City Centre is planned, it builds Barracks and Training Centre tech, staffs
oil, produces Machine Gunners before Gun Works and then AT teams from Gun Works toward a one-for-one support mix, and keeps those
support units staged in a short line on the enemy-facing side of its main-base steel cluster
instead of launching outbound attack waves.
After 50 supply used, it switches to a Vehicle Works tech path, stops Machine Gunner / AT team
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
