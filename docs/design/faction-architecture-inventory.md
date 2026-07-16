# Faction Architecture Inventory

This inventory captures the active faction boundary for the current pre-alpha faction rollout. It
records what exists today, names the temporary compatibility shims, and points later phases at
focused checks. It does not define the final faction catalog shape.

## Compatibility Boundary

Faction work is pre-alpha and does not preserve compatibility with old replay artifacts, persisted
match-history replay payloads, compact snapshot versions, or old protocol payloads. Later phases
may break those formats when they add explicit faction identity. Preserve current live gameplay
behavior for the existing faction unless a later phase explicitly changes it.

## Active Faction Boundary Sources

This document owns the active faction lifecycle taxonomy and the current catalog-id status table.
`server/crates/rules/src/faction.rs` owns catalog facts, loadouts, and ability availability.
`server/src/lobby/faction_validation.rs` owns lifecycle admission. `docs/design/protocol.md` owns
wire vocabulary and payload shapes, and `docs/design/server-sim.md` owns the `Game` API and sim
boundary wording.

`plans/archive/faction/*` files are historical-only evidence from the earlier faction rollout. They
are not active checker inputs and must not be treated as lifecycle policy unless a future plan
explicitly copies a rule back into an active design document. Guard scripts may check that the
archive exists and is named as historical, but they must not read archived phase files as the source
of current faction truth.

## Catalog Id Statuses

Lifecycle status is explicit and separate from catalog existence:

- `playable`: allowed in normal product match starts where the owning lifecycle path accepts a
  playable faction id.
- `playable-human-only`: allowed for human selection but not for AI, prediction, or replay-capable
  starts until those lifecycle paths explicitly opt in. No current catalog id uses this status.
- `test-fixture-only`: allowed only by explicit fixture/test contexts and never by normal product
  selectors or replayable starts.
- `reserved/future`: named for future work but not admitted by catalog, lobby, replay, AI, or
  prediction paths. No current catalog id uses this status.
- `historical-only`: retained as archive evidence and never treated as active lifecycle policy.

| Faction id or path | Status | Current lifecycle policy |
| --- | --- | --- |
| `kriegsia` | playable | Default faction for missing non-replay requests. Supported by normal human lobby, AI seats, dev starts, self-play defaults, replay/branch records, match-history replay, post-match replay, spectator metadata, and local prediction when version/build metadata is compatible. |
| `ekat` | playable | Human-selectable through normal lobby faction selection. Explicit playable validation accepts it for start/replay-capable contexts, and schema 3 replay records plus replay-branch metadata may carry it. Public AI seat creation has no faction selector and still defaults to `kriegsia`; current local prediction is disabled when the local player is `ekat`. |
| `phase2_empty_fixture` | test-fixture-only | Catalog and loadout exist for explicit Rust/test fixture coverage. It is rejected by normal lobby, AI, replay, branch, dev scenario, self-play, match-history, and post-match paths unless the caller uses the `TestFixture` validation context or a direct lower-level sim test helper that deliberately owns the fixture. |
| `plans/archive/faction/*` | historical-only | Archived phase plans, handoffs, and lifecycle matrices are not active faction policy and are not checker lifecycle inputs. |

## Current Entity Identity

Runtime identity is still global `EntityKind`. The current roster has 24 global kinds: 12 units,
10 buildings, and 2 resource nodes. Server rules own the stable ids in
`server/crates/rules/src/kind.rs`; protocol mirrors expose the same string ids in
`server/crates/protocol/src/lib.rs` and `client/src/protocol.js`.

The current production catalog is in `server/crates/rules/src/defs.rs`:

- Units: Worker, Golem, Rifleman, Machine Gunner, Anti-Tank Gun, Mortar Team,
  Artillery, Scout Car, Scout Plane, Tank, Command Car, and Ekat.
- Buildings: City Centre, Zamok, Depot, Barracks, Training Centre, R&D Complex, Factory, Gun
  Works, Tank Trap, and Pump Jack. Tank Trap construction is server-authoritative after Training
  Centre eligibility and is exposed through the mirrored worker build menu; Pump Jack construction
  is a contextual worker build on live oil patches and is not exposed through the generic build menu.
- Resource nodes: Steel and Oil.

Temporary compatibility shim policy: direct global kind checks are approved in the current rules
catalog, protocol adapters, setup/loadout code, AI, dev scenarios, command execution, production,
economy, combat, world-query helpers, and tests. New production files that introduce direct
current-faction kind checks should either use the faction catalog API or be added to
`scripts/check-faction-assumptions.mjs` with a short reason. High-risk approved files have a direct
special-case count ratchet so newly added current-faction checks inside those large files are also
visible during review. The ratchet is an inventory review tool, not approval to expand Kriegsia,
Ekat, or fixture special cases casually.

## Current Economy Shape

Steel, Oil, and Supply are the only player resources in snapshots, compact snapshots, replay
analysis, match history replay artifacts, and the HUD. Start-map resources are still only Steel and
Oil nodes. Current Kriegsia starting values are `STARTING_STEEL = 75`, `STARTING_OIL = 0`, and
`STARTING_WORKERS = 6`.

Compact snapshots encode resources as fixed scalar slots: tick, Steel, Oil, Supply used, and Supply
cap. Spectator/replay `player_resources` use the same Steel/Oil/Supply fields. Later generic
resource work must be a separate plan; this faction plan keeps the current resource payload shape.
Approved direct-resource modules are listed in `docs/design/balance.md` under the faction economy
contract. Future direct Steel/Oil/Supply references outside those owners should either route
through catalog-aware cost/loadout helpers or update that approved inventory deliberately.

## Current Starting Loadout

The standard Kriegsia match start is defined by the `kriegsia.standard` faction loadout in
`server/crates/rules/src/faction.rs` and assembled by `server/crates/sim/src/game/setup.rs`: each
player gets one completed City Centre, six Workers in a ring, nearby Steel/Oil resource clusters,
starting Steel and Oil, and supply from the City Centre. Unknown non-empty faction ids receive no
catalog loadout, starting entities, starting Steel/Oil, or Kriegsia supply credit; lifecycle owners
must validate before building a `Game`.

The standard Ekat match start is defined by the `ekat.standard` faction loadout: one completed
Zamok and one Ekat hero, with no starting Steel/Oil or Supply requirement. Zamok trains Golems for
the current Ekat economy and recovery slice. The
`phase2_empty_fixture.scout_depot` loadout starts one Depot and one Scout Car for explicit
fixture tests only; catalog existence does not make that id product-playable.

Replay starts reconstruct from recorded per-player `PlayerStartingLoadout` records. Replay
validators reject missing loadouts, records for unknown players, faction mismatches, empty loadout
ids, and loadout ids that do not exist in the player's faction catalog. Global starting Steel/Oil
constructors remain compatibility helpers for tests rather than replay/lifecycle reconstruction
APIs.

## Current Tech Tree

Workers can place City Centre and Supply Depot immediately, and can place Pump Jacks contextually on
live oil patches with no tech requirement. Barracks also has no building prerequisite; Training
Centre requires a completed City Centre and Barracks; R&D Complex, Factory, and Gun Works require a
completed City Centre and Training Centre; Tank Trap requires a completed Training Centre. City
Centre trains Workers. Barracks trains Riflemen immediately and Machine
Gunners after the Training Centre requirement is met. Factory trains Scout Cars immediately, then
Tanks and Command Cars after Tank Production research. Gun Works trains
Mortar Teams immediately, Anti-Tank Guns after Medium Guns research, and Artillery after Heavy
Guns research.

Research unlocks live in `server/crates/sim/src/game/upgrade.rs` and client descriptors in
`client/src/config.js`. Training Centre researches Methamphetamines. R&D Complex researches
Medium Guns, Heavy Guns, Artillery Fire Control, Tank Production, Mortar Autocast,
and Smoke Plus; Heavy Guns requires completed Medium Guns research, and Artillery Fire Control
requires completed Heavy Guns research. Tank Production unlocks both Tanks and Command Cars. The
current Ekat tech tree starts with Zamok training Golems;
Golem-converted tech buildings are still planned work.

## Current Ability Surface

Current ability ids are Charge (legacy no-op compatibility only), Smoke, Mortar Fire, Point Fire,
Breakthrough, Ekat Teleport, Ekat Line Shot, Ekat Magic Anchor, and Ekat Consume Golem. The Rust
faction catalog in
`server/crates/rules/src/faction.rs` is authoritative for ability id, carrier, target mode,
range, cooldown/charge, resource cost, queueability, autocast support, and command-card affordance
metadata. `client/src/config.js` is the checked client projection, while
`server/crates/sim/src/game/services/ability_orders.rs`, `server/crates/sim/src/game/ability.rs`,
and `server/crates/sim/src/rules/projection.rs` own execution and projection hooks. Protocol
ability vocabulary and compact codes remain mirrored through `server/crates/protocol/src/lib.rs`
and `client/src/protocol.js`.

## Current Client Command Cards

The command-card renderer is local JS in `client/src/hud_command_card.js`; faction-sensitive
build, train, research, and ability descriptors are driven by the checked catalog mirror in
`client/src/config.js`. Kriegsia and Ekat command ids are namespaced by faction, unknown valid ids
fail closed to an empty catalog, and fixture catalogs remain test-only. `node tests/hud_command_card.mjs`
is the focused command-card guard, and `node scripts/check-faction-catalog-parity.mjs` compares
client-exposed descriptor data against the Rust catalog dump.

## Current AI Coupling

AI is Kriegsia-only through the public lobby seat flow. The AI decision layer assumes Workers gather
Steel directly and build Pump Jacks for Oil, City Centres anchor bases and expansions,
Barracks/Factory/Gun Works drive production, Tanks influence oil demand, and Steel/Oil/Supply
budgets are fixed fields. Public `addAi` requests
do not accept a faction id and always create Kriegsia AI seats; non-Kriegsia AI support needs an
explicit AI phase.

## Current Prediction And WASM Coupling

Prediction/WASM assumes the current start payload, global entity ids, current command set,
Steel/Oil/Supply scalar resources, and compact snapshot version. Local prediction is currently
supported only for local Kriegsia players; local Ekat or fixture players disable prediction with the
stable `unsupported-local-faction` reason until the WASM simulation and adapter support those
contracts intentionally.

## Lifecycle Paths

This section is the maintained source of truth for match creation, playback, replay branch,
spectator, dev scenario, self-play, AI, prediction, match-history, and post-match replay paths.
Later phases must update this section whenever they touch one of those lifecycle paths.

| Path | Faction source | Allowed factions | AI behavior | Prediction behavior | Replay/branch behavior | Tests or checks |
| --- | --- | --- | --- | --- | --- | --- |
| Normal lobby start | `LobbyPlayer.factionId` and `PlayerInit.faction_id`, defaulted by `lobby::faction_validation` | `kriegsia` and `ekat`; fixture and unknown ids reject | No AI assignment by `setFaction`; AI seats are separate | Enabled only for local Kriegsia when build/version metadata is compatible | Schema 2 records player faction id plus per-player loadout record | `tests/faction_integration.mjs`, `tests/prediction_controller.mjs`, `server/src/lobby/faction_validation.rs` tests |
| AI add/remove/start | AI `PlayerInit.faction_id`, created by public `addAi` | Public AI seats default to `kriegsia`; no public Ekat selector | Kriegsia-only through public lobby controls | Not applicable | Schema 2 records AI faction and per-player loadout if match starts | `tests/ai_integration.mjs`, `tests/server_integration.mjs` |
| Fixture/dev faction start | Explicit Rust test/dev harness only | `phase2_empty_fixture` only in `TestFixture` validation or direct lower-level tests | Rejected unless a later phase explicitly adds fixture AI | Disabled when local fixture player is unsupported | Fixture ids stay in explicit test artifacts only | `server/crates/sim/src/game/setup/tests.rs`, `tests/prediction_controller.mjs`, `scripts/check-faction-assumptions.mjs` |
| Replay playback | `ReplayArtifactV1.players[].faction_id` and `playerLoadouts[]` in artifact schema 3 | Recorded playable ids `kriegsia` or `ekat`; missing, unknown, and fixture ids reject | From artifact only | Disabled for replay viewers | Schema 3 restores a checkpoint-backed start state; schema 2 and older artifacts reject; never lobby state | `server/crates/sim/src/game/replay.rs` tests, `server/src/lobby/room_task.rs` replay tests |
| Replay branch staging/launch | Branch seed seats copy recorded `factionId` from replay players | Recorded playable ids `kriegsia` or `ekat`; unsupported seat faction ids reject before live launch | From recorded branch seed only | Disabled unless supported by branch schema/WASM | Reconstruct from branch seed and cloned keyframe | `server/src/lobby/room_task.rs` tests, `tests/protocol_parity.mjs` |
| Dev scenarios | Scenario definition plus validation/defaulting | Current bundled scenarios default to Kriegsia; explicit playable ids may be accepted by an owning scenario | Not applicable unless scenario declares AI | Enabled only for local Kriegsia | Not replayed unless scenario recording exists | `server/crates/sim/src/game/setup/dev_scenarios/tests.rs`, `docs/context/testing.md` |
| Self-play | Self-play `PlayerInit.faction_id`, validated by `lobby::faction_validation` | Current bundled self-play defaults to Kriegsia; explicit Ekat needs a self-play script that owns it | Kriegsia-only in current live AI scripts | Not applicable | Artifact schema 3 records a checkpoint-backed start plus faction ids and per-player loadouts | `server/crates/ai/src/selfplay` tests |
| Match history replay | Stored schema-3 match artifact | Recorded playable ids `kriegsia` or `ekat`; missing, unknown, and fixture ids reject | From artifact only | Disabled for replay viewers | Load from persisted schema 3; schema 2 and older artifacts are incompatible | `server/src/main.rs` replay compatibility tests, `docs/design/match-history.md` |
| Spectator/no-fog view | Live match start payload or replay schema | Match factions from start/replay metadata | Not applicable | Disabled | Preserve recorded faction metadata | `tests/server_integration.mjs` |
| Post-match replay | Captured schema-3 match artifact | Recorded playable ids `kriegsia` or `ekat`; missing, unknown, and fixture ids reject | From artifact only | Disabled for replay viewers | Load from captured checkpoint-backed start state with command log | `tests/server_integration.mjs` |

## Current Guardrail Checks

- `scripts/check-faction-assumptions.mjs` validates this inventory's anchors and ratchets the
  current files that may contain direct current-faction kind or ability special cases.
- `server/crates/rules/src/defs.rs` tests lock the current production catalog.
- `server/crates/sim/src/game/setup/tests.rs` locks the current standard and fixture start loadouts.
- `tests/protocol_parity.mjs` locks protocol kind, ability, upgrade, compact snapshot, and resource
  code parity.
- `tests/hud_command_card.mjs` locks representative current command-card descriptors.
- `node scripts/check-faction-catalog-parity.mjs` compares every client-exposed catalog with the
  Rust dump across catalog ids, loadout ids, train and research keys, builder/gatherer/production
  anchors, ability command-card metadata, costs, and playable selector ids. It verifies that all
  Rust catalogs are dumpable, that fixture catalogs stay mirrored for tests, and that
  fixture/future catalogs are not exposed as playable lobby options. Every real-faction descriptor
  exposed in `client/src/config.js` must be compared against the Rust dump by this gate.

## Guardrail Map For Future Faction Work

When faction behavior changes, update the owning source and its guard in the same change:

- Faction catalog facts, loadouts, and ability availability: update
  `server/crates/rules/src/faction.rs`, the Rust catalog dump, `client/src/config.js`, and
  `scripts/check-faction-catalog-parity.mjs`.
- Lifecycle admission for lobby, AI, replay, branch, dev scenario, self-play, match
  history, and fixture contexts: update `server/src/lobby/faction_validation.rs`, this inventory's
  lifecycle table, and focused server or Node tests for the touched path.
- Wire vocabulary, payload fields, compact codes, default ids, and replay schema shape: update
  `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs` if it adapts the payload,
  `client/src/protocol.js`, `docs/design/protocol.md`, and `tests/protocol_parity.mjs`.
- Client command cards, hotkeys, UI faction selectors, prediction compatibility, and client-visible
  mirrors: update the relevant `client/src/*` module, `docs/design/client-ui.md`,
  `tests/hud_command_card.mjs`, `tests/hotkey_profiles.mjs`, `tests/client_contracts.mjs`, or
  `tests/prediction_controller.mjs` as applicable.
- Server simulation starts, loadouts, setup helpers, AI scripts, self-play artifacts, and replay
  reconstruction: update `docs/design/server-sim.md` plus the focused Rust tests under the touched
  crate.
- Guard wiring: keep `tests/run-all.sh`, `tests/select-suites.mjs`, and
  `docs/design/testing.md` aligned so faction-sensitive files select the assumption checker and
  catalog parity checker.
- Archive policy: archived plans under `plans/archive/faction/*` are evidence only. Do not import
  lifecycle policy, status tables, checker allowlists, or source paths from archived plan files;
  copy any still-current rule into this inventory or another active design doc before using it.
