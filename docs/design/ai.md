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
Owned entities, currently visible allies, currently visible enemies, and remembered contacts are
separate collections. A remembered contact is stale last-seen knowledge, never a current target.

Static resource locations are public, but `AiResourceAmount::Unknown` is retained until a
recipient-scoped resource delta or visible node reveals a quantity. The public frame never exposes
the historical synthetic `remaining = 1` value or the policy-derived `free_for_combat` flag.
Likewise, `AiBuildObservation` means only that this controller inferred an outstanding submitted
build; it is not an accepted, legal, or active-build receipt.

`AiActions` retains at most 256 `AiActionRequest`s in call order for one step. The Phase 3 action
vocabulary covers move, attack-move, direct attack, gather, build, train, research, hold-position,
and Anti-Tank Gun setup. The host translates finalized requests into ordinary `SimCommand`s only
after the strategy returns; canonical controller ordering, all-controller observation before any
enqueue, worker-retreat-first ordering, normal command validation, and replay logging are
unchanged. Rich planners, reservations, budgets, task handles, acceptance results, and uncommon
actions are intentionally not part of this seam yet.

### Shared decision core

Each controller runs on a staggered cadence and constructs the typed `AiFrame`. Built-in profiles
run through the crate-private `LegacyProfileStrategy`, which projects that frame back into the
internal `AiObservation` before the generic decision loop applies the selected `AiProfile` policy.
That compatibility projection alone preserves the historical false `is_ai` value, synthetic
unknown-resource amount, `free_for_combat` derivation, and old sorting/filtering rules; these quirks
are not represented as truthful SDK facts. The direct legacy observation constructor remains only
as a test oracle for field-for-field projection checks. Existing shared action helpers and their
local per-think budget continue to prevent resource and supply overcommitment.

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

Jeff's AI is the fast-Tank timing profile developed from the standalone local bot workspace. It
opens with exactly two entrenched Machine Gunners held close to the main steel line; a defender
below 50% health no longer counts toward that healthy screen and triggers a replacement. The full
Tank tech path starts immediately, Tanks do not wait for Methamphetamines, and defensive panic
does not replace the active armored production plan with infantry. After two Tanks, one Scout Car
takes temporary Factory priority and joins those Tanks as a single vision-supported attack wave.
Expansion, a second Factory, Methamphetamines, and other optional spending wait until the initial
Tank core exists. The profile uses the shared decision and action layers, receives only
fog-filtered observations, and issues ordinary validated player commands for spending, placement,
production, and combat.

### Self-play and arena tools

The test-only schema-1 Jeff live-controller oracle freezes command generation separately from
simulation replay. It drives two production `AiController`s through the same
`CanonicalAiTickDriver` used by the room, in player order on authored `Chokes` with seed
`0x4a45_4646`. It captures both stagger offsets and every empty/result batch before enqueue, then
fingerprints the fog-filtered inputs, recipient events, and post-tick player views while preserving
exact retreat and emitted commands. The normal test compares a 3,600-tick prefix; the
`RTS_FULL_AI_TESTS=1` tier compares the full 9,000-tick fixture. The fixture and candidate policy
are documented in `server/crates/ai/fixtures/README.md` and the testing design.

Matchup, arena, balance, `LiveSelfPlay`, real-AI tests, and live-AI performance hosts all use that
driver. `ProfileBackedScript` survives only as a thin `AiController` adapter for synthetic mixed
script fixtures; its economy-only variant is an explicitly named command-filtering wrapper. It
owns no profile decision memory, map cache, pending builds, placement search, combat-stage state,
or cadence. Synthetic `WorkerRushScript` and `MineOnlyScript` retain their explicitly scoped
six-tick harness cadence.

This cutover intentionally changes historical offline output. The old adapter invoked both players
at tick zero and then player 1/2 at ticks 5/4 modulo six, used the default away-from-center build
search, and injected no retreat reflex. Canonical invocations begin at ticks 8/7 and repeat every
nine ticks, traces exist only on those decision ticks, retreat commands remain first even between
decisions, and production placement/stage suppression applies. In the canonical seed-7
`jeffs_ai` versus `ai_2_1` 9,000-tick lane, the first applied commands are player 2 at tick 8 and
player 1 at tick 9; representative first builds are AI 2.1 Barracks `(115,12)` at tick 476 and
Jeff Pump Jack `(9,14)` at tick 513. The lane reaches the tick cap as a draw and its replay verifies
exactly; these values document the new tooling baseline, not a balance guarantee.

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
