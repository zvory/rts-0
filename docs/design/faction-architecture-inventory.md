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
current-faction kind checks should either use the faction catalog API or be added to
`scripts/check-faction-assumptions.mjs` with a short reason. High-risk approved files have a direct
special-case count ratchet so newly added current-faction checks inside those large files are also
visible during review.

## Current Economy Shape

Steel, Oil, and Supply are the only player resources in snapshots, compact snapshots, replay
analysis, match history replay artifacts, and the HUD. Start-map resources are still only Steel and
Oil nodes. Current starting values are `STARTING_STEEL = 75`, `STARTING_OIL = 0`,
`STARTING_WORKERS = 4`, with quickstart resources set to `99_999` Steel and `99_999` Oil.

Compact snapshots encode resources as fixed scalar slots: tick, Steel, Oil, Supply used, and Supply
cap. Spectator/replay `player_resources` use the same Steel/Oil/Supply fields. Later generic
resource work must be a separate plan; this faction plan keeps the current resource payload shape.
Approved direct-resource modules are listed in `docs/design/balance.md` under the faction economy
contract. Future direct Steel/Oil/Supply references outside those owners should either route
through catalog-aware cost/loadout helpers or update that approved inventory deliberately.

## Current Starting Loadout

The standard Kriegsia match start is defined by the `kriegsia.standard` faction loadout in
`server/crates/rules/src/faction.rs` and assembled by `server/crates/sim/src/game/setup.rs`: each
player gets one completed City Centre, four Workers in a ring, nearby Steel/Oil resource clusters,
starting Steel and Oil, and supply from the City Centre. Unknown non-empty faction ids receive no
catalog loadout, starting entities, starting Steel/Oil, or Kriegsia supply credit; lifecycle owners
must validate before building a `Game`. Debug quickstart adds human-only extra buildings, combat
units, resources, debug path overlays, and an inert enemy mortar corner for inspection.

Replay starts reconstruct from recorded per-player `PlayerStartingLoadout` records. Replay
validators reject missing loadouts, records for unknown players, faction mismatches, empty loadout
ids, and loadout ids that do not exist in the player's faction catalog. Global starting Steel/Oil
constructors remain compatibility helpers for tests and debug starts rather than replay/lifecycle
reconstruction APIs.

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
`client/src/config.js`. Phase 6 should make the faction-aware Rust ability registry the
authoritative source for ability id, carrier, target mode, cooldown/charge, cost, and command-card
affordance metadata, then project that to command validation and client parity checks instead of
adding another parallel table. Until then, the split metadata remains intentionally documented and
guarded by catalog parity, protocol parity, and command tests.

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
- `node scripts/check-faction-catalog-parity.mjs` compares the client-exposed default catalog with
  the Rust dump and verifies that all Rust catalogs are dumpable while fixture/future catalogs stay
  explicitly unsupported on the client surface. This checked mirror remains the Phase 10 client
  catalog path: every real-faction descriptor exposed in `client/src/config.js` must be compared
  against the Rust dump by this gate.
