# Faction Architecture Inventory

This inventory captures the current single-faction assumptions before faction identity is added.
It is intentionally a Phase 0 guardrail: it records what exists today, names the temporary
compatibility shims, and points later phases at focused checks. It does not define the final
faction catalog shape.

## Compatibility Boundary

Faction work is pre-alpha and does not preserve compatibility with old replay artifacts, persisted
match-history replay payloads, compact snapshot versions, or old protocol payloads. Later phases
may break those formats when they add explicit faction identity. Preserve current live gameplay
behavior for the existing faction unless a later phase explicitly changes it.

## Current Entity Identity

Runtime identity is still global `EntityKind`. The current roster has 18 global kinds: 9 units,
7 buildings, and 2 resource nodes. Server rules own the stable ids in
`server/crates/rules/src/kind.rs`; protocol mirrors expose the same string ids in
`server/crates/protocol/src/lib.rs` and `client/src/protocol.js`.

The current production catalog is in `server/crates/rules/src/defs.rs`:

- Units: Worker, Rifleman, Machine Gunner, Anti-Tank Gun, Mortar Team, Artillery, Scout Car, Tank,
  and Command Car.
- Buildings: City Centre, Depot, Barracks, Training Centre, R&D Complex, Factory, and Gun Works.
- Resource nodes: Steel and Oil.

Temporary compatibility shim policy: direct global kind checks are approved in the current rules
catalog, protocol adapters, setup/loadout code, AI, dev scenarios, command execution, production,
economy, combat, world-query helpers, and tests. New production files that introduce direct
current-faction kind checks should either use the future faction catalog API or be added to
`scripts/check-faction-assumptions.mjs` with a short reason.

## Current Economy Shape

Steel, Oil, and Supply are the only player resources in snapshots, compact snapshots, replay
analysis, match history replay artifacts, and the HUD. Start-map resources are still only Steel and
Oil nodes. Current starting values are `STARTING_STEEL = 75`, `STARTING_OIL = 0`,
`STARTING_WORKERS = 4`, with quickstart resources set to `99_999` Steel and `99_999` Oil.

Compact snapshots encode resources as fixed scalar slots: tick, Steel, Oil, Supply used, and Supply
cap. Spectator/replay `player_resources` use the same Steel/Oil/Supply fields. Later generic
resource work must be a separate plan; this faction plan keeps the current resource payload shape.

## Current Starting Loadout

The standard match start is hardcoded in `server/crates/sim/src/game/setup.rs`: each player gets one
completed City Centre, four Workers in a ring, nearby Steel/Oil resource clusters, starting Steel
and Oil, and supply from the City Centre. Debug quickstart adds human-only extra buildings, combat
units, resources, debug path overlays, and an inert enemy mortar corner for inspection.

Replay starts currently reconstruct from starting Steel/Oil plus a `ReplayStartingLoadoutMode`
(`Standard` or `DebugHuman`). This is an approved temporary shim until faction loadouts replace
global starting values.

## Current Tech Tree

Workers build City Centre, Depot, Barracks, Training Centre, R&D Complex, Factory, and Gun Works.
City Centre trains Workers. Barracks trains Riflemen and Machine Gunners. Factory trains Scout
Cars, Tanks, and Command Cars. Gun Works trains Mortar Teams, Anti-Tank Guns, and Artillery.

Research unlocks live in `server/crates/sim/src/game/upgrade.rs` and client descriptors in
`client/src/config.js`: Methamphetamines, Anti-Tank Gun Unlock, Tank Unlock, Artillery Unlock,
Command Car Unlock, and Mortar Autocast.

## Current Ability Surface

Current ability ids are Charge, Smoke, Mortar Fire, Point Fire, and Breakthrough. Ability carriers,
costs, cooldowns, target modes, projection, and execution are split across
`server/crates/sim/src/game/ability.rs`, `server/crates/sim/src/rules/projection.rs`,
`server/crates/sim/src/game/services/ability_orders.rs`, `server/crates/protocol/src/lib.rs`, and
`client/src/config.js`. Phase 6 owns registry-backed parity; Phase 0 only records the current
special cases and keeps protocol/client command-card coverage in place.

## Current Client Command Cards

The client command-card descriptors are local JS data in `client/src/hud_command_card.js` and
`client/src/config.js`. The representative descriptor catalog currently covers empty selection,
Worker main/build cards, mixed ability units, City Centre training, Factory training, Gun Works
training, and R&D research. Until Phase 2 adds a generated or mechanically checked catalog mirror,
`node tests/hud_command_card.mjs` is the focused current-faction descriptor guard.

## Current AI Coupling

AI is current-faction-only. The AI decision layer assumes Workers gather Steel/Oil, City Centres
anchor bases and expansions, Barracks/Factory/Gun Works drive production, Tanks influence oil
demand, and Steel/Oil/Supply budgets are fixed fields. New factions must be rejected for AI slots
until an explicit AI phase adds support.

## Current Prediction And WASM Coupling

Prediction/WASM assumes the current start payload, global entity ids, current command set,
Steel/Oil/Supply scalar resources, and compact snapshot version. Non-default factions may disable
prediction until the WASM simulation and adapter support the faction contracts intentionally.

## Lifecycle Paths

`plans/faction/lifecycle-matrix.md` is the maintained source of truth for match creation,
playback, replay branch, spectator, dev scenario, self-play, quickstart, AI, prediction,
match-history, and post-match replay paths. Later phases must update the matrix whenever they touch
one of those lifecycle paths.

## Phase 0 Checks

- `scripts/check-faction-assumptions.mjs` validates this inventory's anchors and ratchets the
  current files that may contain direct current-faction kind or ability special cases.
- `server/crates/rules/src/defs.rs` tests lock the current production catalog.
- `server/crates/sim/src/game/setup/tests.rs` locks the current standard and debug start loadouts.
- `tests/protocol_parity.mjs` locks protocol kind, ability, upgrade, compact snapshot, and resource
  code parity.
- `tests/hud_command_card.mjs` locks representative current command-card descriptors.
