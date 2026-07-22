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

rts-ai owns one AiController per AI player, while Game remains AI-free. The room task invokes each
controller before game.tick(), gives it the same fog-filtered snapshot_for(player) and public
start payload available to that player, then enqueues ordinary SimCommands. AI actions therefore
go through the same validation, costs, supply, placement, and fog rules as human commands; the AI
has no simulation authority of its own.

Outbound attacks use public enemy start tiles. Direct attack targets are limited to currently
visible entities. The worker direct-hit retreat reflex projects recent own-worker damage into
ordinary Move commands without reading private simulation state.

rts-ai may depend on the public simulation API, rules, protocol, and contract crates. It must not
import the server shell, lobby internals, transport layer, or private simulation modules. New AI
observations must be added as a public fog-respecting Game or snapshot surface.

### Shared decision core

Each controller runs on a staggered cadence and constructs a constrained snapshot-backed
AiObservation. The generic decision loop applies the selected AiProfile policy and emits ordinary
commands through the shared action helpers. A local per-think budget prevents resource and supply
overcommitment.

The core also owns static map analysis derived only from StartPayload map terrain, start tiles, and
static resource nodes. When nearby steel is split into fields around the City Centre, defensive
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

The economy model is also observation-owned. A resource node is mineable only when it has
resources remaining, is in range of a completed owned City Centre, is unoccupied by a latched
worker or owned Pump Jack, and is not already reserved for the current think. Steel assignments
emit Gather; oil assignments build Pump Jacks through the usual paid-building path. Expansion
planning can still see known-but-not-yet-mineable resources without assigning workers to them.

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

AI 2.1 is the promoted pressure profile. It fully saturates steel, adds up to twelve oil workers,
keeps an eight-supply buffer, opens one Barracks, expands to two City Centres, and reserves four
Machine Gunners for defense. It begins with Rifleman pressure, then transitions into mixed
Tank/Rifleman pressure once its tank-tech resource threshold is met. At a larger resource float it
adds a second Factory. Frontal waves stage in cohorts so newly produced units do not immediately
join an already-launched wave.

AI Turtle shares AI 2.1 worker, oil, supply, and first-Barracks cadence, but uses a two-Rifleman
opening and does not launch frontal waves. During its opening oil hold, it does not train workers
toward suppressed oil assignments. It prioritizes a Training Centre, an early second City
Centre, Entrenchment, support technology, Machine Gunners, and Anti-Tank Guns. It identifies up
to three own-base chokepoints from the static map analysis, caps Machine Gunner production by
planned choke-line staffing, staffs the active enemy-facing lines with Machine Gunners, and places
Anti-Tank Guns on an own-side backline. Its staged intents include the two-Rifleman opening. The profile prioritizes
the main choke first, can defend a second close-spawn choke, and reinforces under-staffed lines.
Staged defenders emit HoldPosition once after reaching their defensive slot rather than repeating
the command on every think.

All three profiles are self-contained policy records in the same registry. Each profile selects
whether to use the proposal economy manager; AI 2.1, Jeff's AI, and AI Turtle enable it. None
inherits behavior from a retired version or resolves through a second profile name.

Jeff's AI is the server-authoritative port of the champion V3 policy developed and evaluated in
the standalone local bot workspace. It keeps the local policy's 40-worker pre-second-Factory cap,
ten-oil-worker cap, Rifleman and Machine Gunner opening, Entrenchment and Tank transition,
Scout-supported five-Tank armored attack threshold, seven defensive/mobile Machine Gunners,
3.25-tile formation spacing, and second-Factory float thresholds. It uses the shared profile
decision and action layers, receives only fog-filtered observations, and issues ordinary validated
player commands for spending, placement, production, and combat.

### Self-play and arena tools

The ai-matchup binary runs one fixed-horizon profile-versus-profile match until a starting City
Centre objective win or the tick cap. A match with no objective winner at the default 25,000-tick
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
