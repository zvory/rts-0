# Hotspot Responsibility Map

Generated for Phase 2 from the Phase 1 baseline in
[`plans/archive/hotspots/baseline.md`](baseline.md). This is triage evidence only: it maps current
responsibilities and cleanup seams, but does not propose runtime edits or change gameplay behavior.

## Reading Basis

- Phase 1 ranked current files from `origin/main` at
  `63a7de9749e3c77e7adf100fec0f7e1188aa6b50`.
- Required capsules read for this pass: `docs/context/server-sim.md`,
  `docs/context/client-ui.md`, `docs/context/protocol.md`,
  `docs/context/balance.md`, and `docs/context/testing.md`.
- Source references below cite current paths and line anchors from this worktree. They are meant to
  make the map auditable, not to freeze exact future line numbers.
- `server/src/lobby/room_task.rs` is mapped read-only because the hotspot plan says adjacent room
  cleanup work already owns that runtime surface.

## Responsibility Maps

### Contract Tests: `tests/client_contracts.mjs`

Primary responsibilities:

- Dependency-free client contract assertions across module exports, pure helpers, DOM-light UI
  helpers, protocol decoding, config mirrors, input helpers, renderer helpers, prediction,
  observer-analysis UI, and launch URL parsing.
- A single shared fake-DOM/Pixi harness for many unrelated client modules.
- A smoke-like contract net for architectural seams documented in `docs/design/client-ui.md`.

Internal sections and clusters:

- Common assertions, fake DOM/storage, fake HUD/settings/overlay helpers, and hotkey service
  setup live near `tests/client_contracts.mjs:195`.
- HUD resource and selection-budget contracts start at `tests/client_contracts.mjs:809` and
  `tests/client_contracts.mjs:838`.
- Settings and command-card descriptors start at `tests/client_contracts.mjs:997` and
  `tests/client_contracts.mjs:1183`.
- Frame profiler, frame entity views, and match-health contracts start at
  `tests/client_contracts.mjs:1473`, `tests/client_contracts.mjs:1791`, and
  `tests/client_contracts.mjs:1851`.
- Client intent/state ownership, control groups, input router, and pointer-lock contracts start at
  `tests/client_contracts.mjs:2386`, `tests/client_contracts.mjs:2664`,
  `tests/client_contracts.mjs:2709`, and `tests/client_contracts.mjs:2810`.
- Protocol, lobby helpers, lobby browser, score helpers, and net contracts start at
  `tests/client_contracts.mjs:4054`, `tests/client_contracts.mjs:4486`,
  `tests/client_contracts.mjs:4582`, `tests/client_contracts.mjs:4806`, and
  `tests/client_contracts.mjs:4818`.
- Lab client/panel, command budget, prediction controller, replay branch staging, config, and
  GameState clusters start at `tests/client_contracts.mjs:5013`,
  `tests/client_contracts.mjs:5619`, `tests/client_contracts.mjs:5675`,
  `tests/client_contracts.mjs:5760`, `tests/client_contracts.mjs:5878`, and
  `tests/client_contracts.mjs:6767`.
- Command composer, camera, fog, audio, combat audio, and observer analysis overlay clusters start
  at `tests/client_contracts.mjs:8719`, `tests/client_contracts.mjs:8754`,
  `tests/client_contracts.mjs:8787`, `tests/client_contracts.mjs:8848`,
  `tests/client_contracts.mjs:9005`, and `tests/client_contracts.mjs:9040`.

Public entry points and collaborators:

- The file is invoked directly by Node as one script; its public output is pass/fail plus
  `client_contracts.mjs: all contract assertions passed`.
- It imports from most client areas, including `net`, lobby views, `GameState`, HUD/config,
  protocol, input, renderer, lab, minimap, replay controls, settings, hotkeys, and observer
  analysis (`tests/client_contracts.mjs:8` through `tests/client_contracts.mjs:180`).
- It also imports script helpers from `scripts/snapshot-codec-bakeoff.mjs` and
  `scripts/client-perf-harness.mjs`, so it is a cross-area contract aggregator rather than one
  domain test.

Cross-file or mirrored contracts:

- Protocol decode and compact-code checks mirror `server/crates/protocol/src/lib.rs` and
  `client/src/protocol.js`.
- Config, command-card, faction, and unit stat checks mirror `server/crates/rules/src/balance.rs`,
  `server/crates/rules/src/faction.rs`, and `client/src/config.js`.
- Client architecture checks assert that browser-local intent moved out of `GameState` and into
  `ClientIntent`/injected collaborators.

Existing tests that protect behavior:

- This file is itself the focused contract suite for many client modules.
- Related mirror guardrails include `node tests/protocol_parity.mjs`,
  `node scripts/check-faction-catalog-parity.mjs`, and
  `node scripts/check-client-architecture.mjs`.

Likely mechanical extraction seams:

- Split by the existing section comments into focused scripts such as HUD contracts, protocol
  decode contracts, lobby contracts, lab contracts, state/input contracts, renderer contracts, and
  audio/observer-analysis contracts.
- Move fake DOM, fake Pixi, fake audio, fake storage, and assertion helpers into a shared
  `tests/client_contracts/` helper module.
- Keep one small top-level runner that imports the split files if CI needs one command name.

Likely design-coupled seams:

- Do not split protocol and config mirror assertions away from their server parity context unless
  the new location still runs with the parity guardrails.
- Do not turn the file into browser-dependent tests; its value is low-overhead Node coverage.
- Do not weaken the client architecture assertions that catch cross-area import and intent-state
  regressions.

Ownership conflicts:

- None for test organization, but any split should coordinate with client architecture/test-suite
  selection rules because this file is a high-degree co-change hub.

### Room Runtime: `server/src/lobby/room_task.rs`

Primary responsibilities:

- Own one room task's lifecycle, connected humans, AI lobby seats, mode, match state, room-time
  state, pause state, lab state, replay branch state, match-history metadata, and drain accounting.
- Route client room events into lobby, live match, replay, dev scenario, lab, and branch-staging
  flows.
- Construct and send start payloads, room-time messages, observer analysis, lab state/results,
  branch messages, game-over messages, and room-owned snapshot notices.
- Drive live/dev/replay ticks through the public `Game` API and room projection/fanout helpers.

Internal sections and clusters:

- Room/player data types and mode enums are at `server/src/lobby/room_task.rs:44`,
  `server/src/lobby/room_task.rs:149`, and `server/src/lobby/room_task.rs:171`.
- Lab session helpers start at `server/src/lobby/room_task.rs:222`; lab op conversion and
  validation helpers start at `server/src/lobby/room_task.rs:299`.
- Dev scenario driver helpers start at `server/src/lobby/room_task.rs:524`.
- `RoomTask` state fields are declared at `server/src/lobby/room_task.rs:554`; the implementation
  starts at `server/src/lobby/room_task.rs:625`.
- Event dispatch starts at `server/src/lobby/room_task.rs:846`.
- Drain, joining, and replay-join prompts are clustered around
  `server/src/lobby/room_task.rs:999`, `server/src/lobby/room_task.rs:1156`, and
  `server/src/lobby/room_task.rs:1278`.
- Lobby seat/team/faction/AI/map/spectator controls are clustered around
  `server/src/lobby/room_task.rs:1408`.
- Gameplay command routing and command receipts start at `server/src/lobby/room_task.rs:1863`;
  live pause starts at `server/src/lobby/room_task.rs:1935`.
- Dev/replay/branch/lab join flows start at `server/src/lobby/room_task.rs:2086`,
  `server/src/lobby/room_task.rs:2120`, `server/src/lobby/room_task.rs:2205`, and
  `server/src/lobby/room_task.rs:2261`.
- Match launch, branch launch, lab session launch, and dev session launch start at
  `server/src/lobby/room_task.rs:2473`, `server/src/lobby/room_task.rs:2630`,
  `server/src/lobby/room_task.rs:2733`, and `server/src/lobby/room_task.rs:2857`.
- Tick handlers for live, dev, and replay flows start at `server/src/lobby/room_task.rs:3292`,
  `server/src/lobby/room_task.rs:3376`, and `server/src/lobby/room_task.rs:3441`.
- Lab requests, replay vision/seek, branch staging controls, and end-match handling start at
  `server/src/lobby/room_task.rs:3579`, `server/src/lobby/room_task.rs:3545`,
  `server/src/lobby/room_task.rs:3880`, and `server/src/lobby/room_task.rs:4142`.
- Tests start at `server/src/lobby/room_task.rs:4519` and cover public summary, faction/team/AI
  lobby behavior, replay, room time, lab, live spectators, branch staging, match history, drain,
  and replay-viewer returns.

Public entry points and collaborators:

- `RoomTask::new` constructs room-owned state (`server/src/lobby/room_task.rs:625`).
- `RoomTask::handle_event` is the main room-event dispatcher (`server/src/lobby/room_task.rs:846`).
- Collaborators include connection delivery, crash replay, dev replay, faction validation, start
  payload building, participants/issuers, projection policy, replay branch state, session policy,
  snapshot fanout, tick control, DB persistence, drain handles, `rts_ai::AiController`, and
  `rts_sim::game::Game` (`server/src/lobby/room_task.rs:1` through
  `server/src/lobby/room_task.rs:39`).

Cross-file or mirrored contracts:

- Must preserve the `Game` public API seam and avoid direct sim internals.
- Must preserve protocol tags/fields for `StartPayload`, `ServerMessage`, room-time, lab,
  replay-analysis, command receipt, branch, and lobby messages.
- Must preserve fog/projection policy: per-recipient snapshots and observer analysis are room-owned
  and fan out through projection helpers.
- Match-history writes are server-only and env/policy gated.

Existing tests that protect behavior:

- The large in-file test module starts at `server/src/lobby/room_task.rs:4519`.
- Related integration suites include server/lobby live Node tests and protocol/client contract
  tests when wire messages change.

Likely mechanical extraction seams:

- A room-control-plane split could extract pure helpers and small state owners for live pause,
  drain warnings, lab operation conversion/state, branch staging message construction, and
  replay-viewer room-time controls.
- The in-file tests could split by room mode without moving runtime code first.
- Start-payload resend helpers and observer-analysis fanout helpers have clearer collaborator
  boundaries than the central event loop.

Likely design-coupled seams:

- Do not mechanically move `handle_event`, phase transitions, or tick handlers without an explicit
  room-runtime cleanup plan; they encode room lifecycle ordering.
- Do not split any path that would bypass `SessionPolicy`, `ProjectionPolicy`, `Participants`, or
  the public `Game` API seam.
- Do not change branch/live seat mapping or spectator projection while doing size cleanup.

Ownership conflicts:

- Active room-runtime cleanup owns this area. Later hotspot work should rank room-task cleanup but
  defer implementation until that owner is clear or explicitly folded into a new plan.

### Command Service: `server/crates/sim/src/game/services/commands.rs`

Primary responsibilities:

- Drain pending `SimCommand`s, validate authority/ownership/budget/visibility/tech/placement, and
  convert accepted requests into order-planner requests or direct ability/build/train/cancel/rally
  mutations.
- Protect the tick path from hostile or stale command input by deduping and capping unit ids,
  rejecting non-finite or invalid targets, and treating stale ids as no-ops.
- Bridge between client-facing command vocabulary, rules/faction data, order planner facts, entity
  state, movement coordinator, spatial index, fog, smoke, ability runtime, mortar/artillery stores,
  and player notices.

Internal sections and clusters:

- `apply_commands` is the service entry point at
  `server/crates/sim/src/game/services/commands.rs:63`.
- Input shaping and command-budget helpers start at
  `server/crates/sim/src/game/services/commands.rs:544`.
- Planner config/fact construction starts at `server/crates/sim/src/game/services/commands.rs:625`.
- Planner action execution starts in the nested `planned_actions` module at
  `server/crates/sim/src/game/services/commands.rs:738`.
- Target validators for immediate/planned commands start around
  `server/crates/sim/src/game/services/commands.rs:1047`.
- Planner/sim enum conversion helpers start at
  `server/crates/sim/src/game/services/commands.rs:1139`.
- Ability ordering and artillery point-fire logic start at
  `server/crates/sim/src/game/services/commands.rs:1212` and
  `server/crates/sim/src/game/services/commands.rs:1354`.
- Build, train, rally, and cancel handlers start at
  `server/crates/sim/src/game/services/commands.rs:1702`,
  `server/crates/sim/src/game/services/commands.rs:1782`,
  `server/crates/sim/src/game/services/commands.rs:1872`, and
  `server/crates/sim/src/game/services/commands.rs:1897`.
- The in-file tests start at `server/crates/sim/src/game/services/commands.rs:1972` and cover
  construction, tech gates, queued commands, command budget, support setup, artillery, smoke,
  command hardening, tank traps, and helper fixtures.

Public entry points and collaborators:

- Public surface is intentionally narrow: `pub(crate) fn apply_commands`.
- Collaborators are `order_planner`, `order_execution`, `ability_orders`, `construction`,
  `MoveCoordinator`, `SpatialIndex`, `Fog`, `SmokeCloudStore`, `AbilityRuntime`, mortar/artillery
  stores, `TeamRelations`, rules/faction data, and protocol notices/events.

Cross-file or mirrored contracts:

- Command tags and payloads must stay aligned with protocol DTOs and client command composers.
- Client-visible budget constants are mirrored by `client/src/config.js`.
- Unit/building costs, supply, unlocks, and ability metadata are rules-owned, not command-service
  owned.
- Any change to command acceptance semantics can affect replay determinism, prediction, command
  receipts, and client command cards.

Existing tests that protect behavior:

- In-file unit tests from `server/crates/sim/src/game/services/commands.rs:1972`.
- Broader sim behavior tests in `server/crates/sim/src/game/tests.rs`.
- Client command-composer/HUD contracts in `tests/client_contracts.mjs`.

Likely mechanical extraction seams:

- Extract command input guards and budget validation into a small pure helper module.
- Extract planner fact/adaptor code and enum conversions near the `order_planner` boundary.
- Extract ability-specific dispatch and artillery point-fire helpers once tests remain local to the
  command-service group.
- Split in-file tests by command family before moving production code if review load is the main
  pain.

Likely design-coupled seams:

- Keep the `apply_commands` orchestration order cohesive until there is a stronger service design;
  ordering affects notices, resources, queued-order replacement, and replay determinism.
- Do not move command semantics into rules constants or client mirrors.
- Do not split planner and execution without preserving the existing architecture-check allowlist.

Ownership conflicts:

- None obvious, but this file is central to combat, movement, support units, Ekat abilities,
  command budget, and tank-trap work. Cleanup should be narrow and heavily tested.

### Protocol Mirror: `server/crates/protocol/src/lib.rs` and `client/src/protocol.js`

Primary responsibilities:

- Server Rust DTOs define inbound `ClientMessage`, outbound `ServerMessage`, lobby/start/snapshot
  support structs, replay/lab/branch messages, compact snapshot transport metadata, protocol
  contract metadata, and compact JSON/MessagePack snapshot encoders.
- Client JS mirror defines tags, vocabularies, compact code tables, binary frame parsing, compact
  snapshot decoding, message builders, and command builders.

Internal sections and clusters:

- Rust client messages start at `server/crates/protocol/src/lib.rs:113`.
- Rust server messages and reliable control-plane structs start at
  `server/crates/protocol/src/lib.rs:672`.
- Rust compact snapshot constants/codecs start at `server/crates/protocol/src/lib.rs:870`.
- Rust protocol contract and compact slot schema metadata start at
  `server/crates/protocol/src/lib.rs:986`.
- Rust compact snapshot serializers and MessagePack writer start at
  `server/crates/protocol/src/lib.rs:1724`.
- Client tags/vocabularies/code tables start at `client/src/protocol.js:5`.
- Client frame and MessagePack parsing starts at `client/src/protocol.js:391`.
- Client compact snapshot decoding starts at `client/src/protocol.js:572`.
- Client message builders and command builders start at `client/src/protocol.js:1130` and
  `client/src/protocol.js:1248`.

Public entry points and collaborators:

- Rust public entries include DTO types, `protocol_contract`, `encode_snapshot_frame`,
  `serialize_compact_snapshot`, and `serialize_messagepack_compact_snapshot`.
- Client public entries include constants, `parseServerFrame`, `decodeServerMessage`, `msg`, and
  `cmd`.
- Collaborators are `server/src/protocol.rs`, `server/crates/sim/src/protocol.rs`,
  `server/crates/contract/src/lib.rs`, `client/src/config.js`, client consumers, and parity tests.

Cross-file or mirrored contracts:

- Every tag, field, code, compact slot, version, and enum vocabulary must stay aligned between Rust,
  JS, and `docs/design/protocol.md`.
- `PLAYER_PALETTE` also crosses protocol/config/lobby surfaces and is guarded separately.
- Compact snapshots are a transport optimization; semantic `Snapshot` remains the source of truth.

Existing tests that protect behavior:

- Rust protocol tests start at `server/crates/protocol/src/lib.rs:2649`.
- JS protocol contracts start at `tests/client_contracts.mjs:4054`.
- `node tests/protocol_parity.mjs` guards exported protocol contract parity.

Likely mechanical extraction seams:

- Extract Rust compact codec implementation and MessagePack writer into a protocol submodule while
  keeping public functions re-exported from `lib.rs`.
- Extract generated/declared contract tables into a dedicated contract metadata module.
- On the client, split binary frame parsing from compact snapshot semantic decoding, preserving the
  same exported names.

Likely design-coupled seams:

- Do not split Rust and JS mirrors independently. Any cleanup should be a group-level protocol
  mirror plan with parity tests first.
- Do not change compact snapshot version, field order, or optional slot behavior as part of a size
  cleanup.
- Do not move sim/rules kind conversion into `rts-protocol`; adapters own that dependency boundary.

Ownership conflicts:

- None specific, but protocol is a shared contract surface. Future extraction needs explicit review
  and parity verification.

### Balance Mirror: `server/crates/rules/src/balance.rs` and `client/src/config.js`

Primary responsibilities:

- Rust `balance.rs` holds authoritative shared scalar constants and small stat accessors for
  movement tolerances, setup timings, support ranges, ability/economy numbers, body dimensions,
  resources, supply, and stat lookup wrappers.
- Client `config.js` mirrors the UI/render/fog subset: palette, body dimensions, ability display
  numbers, command budget, per-kind UI stats, ability descriptors, upgrade descriptors, resource
  amounts, faction catalogs, and camera constants.

Internal sections and clusters:

- Rust timing, movement, weapon/support, ability, economy, resource, supply, and body constants run
  from `server/crates/rules/src/balance.rs:7` through
  `server/crates/rules/src/balance.rs:162`.
- Rust `UnitStats`, `BuildingStats`, and stat accessors start at
  `server/crates/rules/src/balance.rs:167`.
- Client timing and palette constants start at `client/src/config.js:8` and
  `client/src/config.js:14`.
- Client body/range/ability scalar mirrors start at `client/src/config.js:45` and
  `client/src/config.js:72`.
- Client `STATS`, `ABILITIES`, and `UPGRADES` start at `client/src/config.js:129`,
  `client/src/config.js:200`, and `client/src/config.js:311`.
- Client resource amounts, worker buildables, faction catalogs, and helper exports start at
  `client/src/config.js:376`, `client/src/config.js:382`, `client/src/config.js:416`, and
  `client/src/config.js:493`.

Public entry points and collaborators:

- Rust entries are exported constants plus `unit_stats`, `building_stats`, and
  `unit_radius_tiles`.
- Client entries are exported constants, descriptors, catalogs, and helpers consumed by HUD,
  renderer, input placement, minimap/fog, command cards, wiki/parity checks, and tests.
- Collaborators include `server/crates/rules/src/defs.rs`, `server/crates/rules/src/faction.rs`,
  `server/src/config.rs`, `server/crates/sim/src/config.rs`, HUD/config client modules, and wiki
  generation.

Cross-file or mirrored contracts:

- Rust is authoritative; client mirrors only player-visible UI/render/fog values.
- Client-visible changes require Rust, client mirror, docs/design balance, wiki checks, and faction
  catalog parity.
- Server-only damage/internal timing values should not be mirrored unless the client needs them.

Existing tests that protect behavior:

- `node scripts/check-faction-catalog-parity.mjs`.
- `node scripts/check-wiki.mjs`.
- Config mirror assertions in `tests/client_contracts.mjs:5878`.

Likely mechanical extraction seams:

- Group Rust constants by domain in submodules or tables only if public exports stay stable.
- Generate or validate more of the client mirror from Rust metadata before attempting large manual
  rearrangements.
- Split client catalog descriptors by units, abilities, upgrades, and factions behind stable
  re-exports.

Likely design-coupled seams:

- Do not split or rename exported constants without checking every Rust compatibility shim and JS
  import.
- Do not make the client mirror authoritative.
- Do not combine server-only balance with client-visible presentation descriptors.

Ownership conflicts:

- None specific, but balance changes are gameplay-facing and require patch notes. A pure cleanup
  should avoid numeric changes entirely.

### Client Match Shell: `client/src/match.js`

Primary responsibilities:

- Compose the live/replay/lab match screen: state, renderer, HUD, input router, minimap, camera,
  prediction, health metrics, audio, settings, pause/give-up controls, observer analysis, replay
  controls, and lab tool hooks.
- Own lifecycle and teardown for browser resources between matches.
- Handle snapshots/events, net reports, prediction overlays, frame loop recovery, room-time state,
  and combat/notice audio.

Internal sections and clusters:

- Imports show app-shell composition across client areas at `client/src/match.js:1`.
- Audio constants and sound maps start at `client/src/match.js:46`.
- `Match` construction and collaborator wiring starts at `client/src/match.js:95`.
- Ping/net report timers start at `client/src/match.js:343`.
- Prediction overlay, adapter lifecycle, and command receipt handling run from
  `client/src/match.js:422` through `client/src/match.js:582`.
- Spectator/lab tool/menu/pause/pointer-lock/settings controls run from
  `client/src/match.js:594` through `client/src/match.js:879`.
- Camera bounds, interpolation, event handling, notices, and combat audio run from
  `client/src/match.js:879` through `client/src/match.js:1095`.
- The frame loop, freeze/stop/room-time/destroy lifecycle runs from
  `client/src/match.js:1111` through `client/src/match.js:1189`.

Public entry points and collaborators:

- Public surface is the `Match` class.
- Collaborators are `Net`, `GameState`, `Renderer`, `HUD`, `Input`, `MatchInputRouter`, `Camera`,
  `Fog`, `Minimap`, `PredictionController`, `SimWasmPredictionAdapter`, `MatchHealth`,
  `FrameProfiler`, `LivePauseOverlay`, observer/replay/lab controls, settings panels, and audio.

Cross-file or mirrored contracts:

- Consumes start payload capabilities/diagnostics; must not infer affordances from room mode names.
- Sends command, pause, give-up, net report, and replay/lab actions through injected transport and
  policy seams.
- Must preserve teardown because `Match.destroy()` owns listener/GPU cleanup across rematches.

Existing tests that protect behavior:

- `tests/client_contracts.mjs` covers frame recovery, match health, prediction toggles,
  pointer-lock bridge, replay/lab launch parsing, observer analysis, and client intent boundaries.
- Browser smoke and client architecture checks protect rendered lifecycle indirectly.

Likely mechanical extraction seams:

- Extract net-report/ping management, combat audio event handling, and settings action wiring into
  app-shell collaborators.
- Keep `Match` as composition shell with injected collaborators; smaller owned helpers should
  receive explicit dependencies rather than cross-importing unrelated client areas.

Likely design-coupled seams:

- Do not move prediction/state/render frame ordering without replay/prediction tests.
- Do not move lab UI ownership into `Match`; lab transport/panel stay app-owned and injected.
- Do not weaken teardown sequencing.

Ownership conflicts:

- None specific, but this file is the client app-shell hub. Extraction should satisfy
  `node scripts/check-client-architecture.mjs`.

### Client HUD: `client/src/hud.js`

Primary responsibilities:

- Render the DOM HUD: resource bar, replay resource rows, control-group tabs, selected-unit panel,
  and 3x3 command card.
- Convert command-card descriptor intents into gameplay commands or browser-local `ClientIntent`
  state.
- Handle command-card affordability, cooldown clocks, ability previews, producer round-robin, and
  local missing-resource audio.

Internal sections and clusters:

- HUD commentary and imports are at `client/src/hud.js:1`.
- Pure helpers `playerHasCompletedKind` and `groupCooldownClocks` start at
  `client/src/hud.js:35` and `client/src/hud.js:59`.
- `HUD` construction starts at `client/src/hud.js:104`; `update` and `destroy` start at
  `client/src/hud.js:159` and `client/src/hud.js:168`.
- Resource, control-group, selected-panel, and command-card rendering start at
  `client/src/hud.js:194`, `client/src/hud.js:284`,
  `client/src/hud.js:379`, and `client/src/hud.js:400`.
- Descriptor dispatch and ability preview logic starts at `client/src/hud.js:513`.
- Producer/unit/research helpers run through `client/src/hud.js:641` to
  `client/src/hud.js:744`.
- Tooltip, cooldown, button, and resource icon helpers run from `client/src/hud.js:790` through
  `client/src/hud.js:990`.

Public entry points and collaborators:

- Public exports are `HUD`, `playerHasCompletedKind`, `groupCooldownClocks`, and selected-panel
  helper re-exports.
- Collaborators are `protocol.js`, `config.js`, `hud_command_card.js`,
  `hud_selection_panel.js`, resource icons, `ClientIntent`, command issuer, hotkey profiles, audio,
  control policy, and `GameState`.

Cross-file or mirrored contracts:

- Command IDs/descriptors must stay aligned with server/faction/rules metadata and client hotkey
  catalog.
- HUD must use injected `ClientIntent` instead of pushing browser-local command state into
  `GameState`.
- Command surface permissions must flow through control policy.

Existing tests that protect behavior:

- HUD resource and selection budget sections in `tests/client_contracts.mjs:809` and
  `tests/client_contracts.mjs:838`.
- Command card descriptor contracts in `tests/client_contracts.mjs:1183`.
- Client architecture checks guard cross-area imports.

Likely mechanical extraction seams:

- Extract resource-row rendering, control-group tab rendering, command-card DOM/button rendering,
  and command intent dispatch as separate injected helpers.
- Continue using `hud_command_card.js` and `hud_selection_panel.js` as established local patterns.

Likely design-coupled seams:

- Do not mix HUD rendering cleanup with command semantics or balance numbers.
- Do not bypass the command issuer/control policy/ClientIntent seams.
- Tooltip/cooldown rendering depends on descriptor shape and hotkey resolution; split with tests.

Ownership conflicts:

- None specific.

### Client State Model: `client/src/state.js`

Primary responsibilities:

- Hold start payload data, two-snapshot interpolation buffers, latest resources/events/upgrades,
  selection/control groups, diagnostic flags, transient projectile/effect buffers, remembered
  buildings, attack reveals, prediction overlays, optimistic production/rally overlays, and map
  queries.
- Provide relation helpers, entity interpolation, selection mutation, control-group mutation, and
  terrain/passability access to renderer, HUD, minimap, and input.

Internal sections and clusters:

- File purpose and invariants are documented at `client/src/state.js:1`.
- Constructor and primary data layout start at `client/src/state.js:62`.
- Player/team/owner helpers run from `client/src/state.js:173` through
  `client/src/state.js:215`.
- Snapshot application starts at `client/src/state.js:245`.
- Transient mortar/artillery/smoke/recoil effect methods run from
  `client/src/state.js:348` through `client/src/state.js:582`.
- Interpolation/entity lookup and prediction overlays run from
  `client/src/state.js:608` through `client/src/state.js:760`.
- Resource deltas/deaths and attack reveal normalization run from
  `client/src/state.js:782` through `client/src/state.js:861`.
- Selection and control groups run from `client/src/state.js:884` through
  `client/src/state.js:1043`.
- Bounds/terrain/passability and player normalization run from
  `client/src/state.js:1061` through `client/src/state.js:1096`.

Public entry points and collaborators:

- Public surface is `GameState`.
- Collaborators are `config.js`, `command_budget.js`, `ProgressExtrapolator`, and `protocol.js`.
- Consumers include renderer, HUD, minimap, input, prediction display, fog, and match shell.

Cross-file or mirrored contracts:

- Snapshot shape must match protocol decoder output.
- Selection budget behavior must match command-service budget limits and HUD display assumptions.
- Browser-local command/placement/lab intent must remain outside `GameState`.

Existing tests that protect behavior:

- GameState contracts in `tests/client_contracts.mjs:6767`.
- Client boundary baseline around `tests/client_contracts.mjs:2386` verifies intent state is not
  owned by `GameState`.

Likely mechanical extraction seams:

- Extract transient visual-effect buffers and live-effect query helpers.
- Extract selection/control-group helpers if command-budget admission remains shared and covered.
- Extract prediction/optimistic overlay application behind stable `GameState` methods.

Likely design-coupled seams:

- Do not change interpolation order, snapshot application semantics, or prediction smoothing during
  a size cleanup.
- Do not reintroduce browser-local command intent into the model.

Ownership conflicts:

- None specific.

### Styling: `client/styles.css`

Primary responsibilities:

- Global app styling across lobby/browser, branch staging, game canvas, HUD chrome, lab panel,
  settings, live pause, selection panel, command card, replay controls/analysis, toast/countdown,
  game-over, responsive rules, and match history.

Internal sections and clusters:

- Variables and global app/body styles start at `client/styles.css:22`.
- Buttons and lobby shell/browser/team/seat controls start at `client/styles.css:254` and
  `client/styles.css:304`.
- Replay branch staging starts at `client/styles.css:1324`.
- Game screen, canvas, overlay chrome, and lab panel start at `client/styles.css:1561` and
  `client/styles.css:1635`.
- Settings and give-up/live-pause controls start at `client/styles.css:1935`,
  `client/styles.css:2248`, and `client/styles.css:2283`.
- HUD resources, minimap, selected panel, control groups, and command card start at
  `client/styles.css:2316`, `client/styles.css:2388`,
  `client/styles.css:2405`, and `client/styles.css:2717`.
- Replay controls/analysis start at `client/styles.css:2988`.
- Toast, countdown, game-over, responsive rules, and match history start at
  `client/styles.css:3416`, `client/styles.css:3435`,
  `client/styles.css:3479`, `client/styles.css:3617`, and `client/styles.css:3671`.

Public entry points and collaborators:

- Public surface is DOM class/id contract, not exported code.
- Collaborators are generated DOM in `client/src/lobby*.js`, HUD/selection/command-card modules,
  lab panel modules, replay controls, observer analysis overlay, match history, and app shell.

Cross-file or mirrored contracts:

- Selectors must match DOM producers. Class/id churn is effectively a UI contract change.
- Responsive constraints interact with HUD layout, command card, replay overlay, and lab window.

Existing tests that protect behavior:

- Client smoke/browser tests cover visible behavior indirectly.
- `tests/client_contracts.mjs` has DOM-light checks for many class-producing modules, but not a
  complete CSS selector contract.

Likely mechanical extraction seams:

- Split by existing section comments into screen/surface files if the app can load them without a
  build step, or keep one CSS file and only introduce section-local cleanup PRs.
- Extract repeated panel/button primitives carefully after visual screenshot checks.

Likely design-coupled seams:

- Do not split CSS without checking the no-build static serving model.
- Do not do broad selector renames as a cleanup; they are behavior-affecting.
- Visual regressions are easy here, so manual browser smoke should accompany any CSS split.

Ownership conflicts:

- None specific.

### Representative Large Sim Tests: `server/crates/sim/src/game/tests.rs`

Primary responsibilities:

- Broad integration-like sim tests for `Game`: replay keyframes, ability runtime, Ekat abilities,
  artillery, fog/projection, smoke, mortar, command replay determinism, dev scenarios, scoring,
  observer analysis, AI identity, resources/mining, tank traps, and deterministic one-player
  sandbox behavior.

Internal sections and clusters:

- Fixtures and legacy snapshot helpers start at `server/crates/sim/src/game/tests.rs:10`.
- Ability runtime/Ekat clusters start at `server/crates/sim/src/game/tests.rs:217` and
  `server/crates/sim/src/game/tests.rs:553`.
- Artillery clusters start at `server/crates/sim/src/game/tests.rs:1537`.
- Team fog, smoke, mortar, and smoke ability clusters run from
  `server/crates/sim/src/game/tests.rs:1921` through
  `server/crates/sim/src/game/tests.rs:3160`.
- Command replay/movement determinism clusters start at
  `server/crates/sim/src/game/tests.rs:3466`.
- Projection, scoring, observer analysis, AI identity, mining, tank-trap, resource snapshot, and
  determinism clusters run from `server/crates/sim/src/game/tests.rs:4253` through
  `server/crates/sim/src/game/tests.rs:5399`.

Public entry points and collaborators:

- Public surface is the Rust test module compiled with `rts-sim`.
- Collaborators are `Game`, `PlayerInit`, maps, snapshots, ability/projectile stores, command
  enqueueing, projection options, and rules constants.

Cross-file or mirrored contracts:

- These tests exercise the `Game` API seam and protocol snapshot/event semantics but should not
  depend on server room machinery.
- Many tests protect fog/projection invariants; moving them requires preserving recipient-specific
  visibility assertions.

Existing tests that protect behavior:

- This file is one of the broad sim safety nets.
- Focused service tests live beside services such as `commands.rs`, movement, and combat.

Likely mechanical extraction seams:

- Split by domain into `ability`, `artillery`, `fog_projection`, `mortar_smoke`,
  `movement_replay`, `scoring_observer`, `mining_resources`, and `tank_trap` test modules.
- Move shared fixtures first, then split clusters without rewriting assertions.

Likely design-coupled seams:

- Do not use test splitting to rewrite sim setup APIs.
- Keep projection and command replay tests close enough to shared fixtures that determinism remains
  easy to audit.

Ownership conflicts:

- None specific.

### Representative AI Self-Play Tests: `server/crates/ai/src/selfplay/tests.rs`

Primary responsibilities:

- Drive scripted/profiled self-play harnesses, validate snapshot invariants, record observations
  and events, write failure/success replay artifacts, run profile matchups, and cover resource
  regression/pending-build tracker/live-AI team behavior.

Internal sections and clusters:

- The harness type and run loop start at `server/crates/ai/src/selfplay/tests.rs:50`.
- Snapshot validation, observation/event recording, and artifact writing run from
  `server/crates/ai/src/selfplay/tests.rs:209` through
  `server/crates/ai/src/selfplay/tests.rs:333`.
- Replay artifact schema test starts at `server/crates/ai/src/selfplay/tests.rs:351`.
- Matchup config/finalization and profile helpers run from
  `server/crates/ai/src/selfplay/tests.rs:382` through
  `server/crates/ai/src/selfplay/tests.rs:652`.
- Resource regression helpers start at `server/crates/ai/src/selfplay/tests.rs:736`.
- Profile-backed and scripted self-play tests start at
  `server/crates/ai/src/selfplay/tests.rs:832`, `server/crates/ai/src/selfplay/tests.rs:875`,
  `server/crates/ai/src/selfplay/tests.rs:905`, and
  `server/crates/ai/src/selfplay/tests.rs:973`.
- Pending-build tracker and deterministic scripted run tests start at
  `server/crates/ai/src/selfplay/tests.rs:1216` and
  `server/crates/ai/src/selfplay/tests.rs:1269`.
- Live AI team behavior and full real-AI test start at
  `server/crates/ai/src/selfplay/tests.rs:1321` and
  `server/crates/ai/src/selfplay/tests.rs:1485`.

Public entry points and collaborators:

- Public surface is the Rust test module compiled with `rts-ai`.
- Collaborators include AI profiles/controllers, sim `Game`, snapshots/start payloads, replay
  artifact schema, environment-gated artifact output, and self-play diagnostic structs.

Cross-file or mirrored contracts:

- Replay artifact schema must stay aligned with dev replay loaders and room replay viewers.
- AI profile behavior depends on sim rules, resources, and team/faction identity.

Existing tests that protect behavior:

- This file is the broad AI self-play safety net.
- Long AI coverage is gated by `RTS_FULL_AI_TESTS=1`/`tests/run-all.sh --full-ai`; default coverage
  keeps a smaller subset practical.

Likely mechanical extraction seams:

- Split harness/artifact helpers from profile-matchup tests.
- Split resource regression and pending-build tracker tests into domain files.
- Keep real-AI long tests clearly separated from default quick self-play tests.

Likely design-coupled seams:

- Do not change time budgets, artifact behavior, or environment gates while splitting tests.
- Do not rewrite profile behavior as part of test organization.

Ownership conflicts:

- None specific.

## Architectural Group Map

| Stable group | Current files and ownership notes |
| --- | --- |
| Room runtime | `server/src/lobby/room_task.rs`, `server/src/lobby/session_policy.rs`, `server/src/lobby/participants.rs`, `server/src/lobby/tick_control.rs`, `server/src/lobby/snapshot_fanout.rs`, `server/src/lobby/projection.rs`. Runtime cleanup is active elsewhere; keep this group read-only for this hotspot plan until reassigned. |
| Server backend shell | `server/src/main.rs`, `server/src/dev_scenarios.rs`, HTTP/WebSocket routing, room registry, deployment drain wiring. Treat as backend orchestration, not sim ownership. |
| Command service | `server/crates/sim/src/game/services/commands.rs`, `server/crates/sim/src/game/command.rs`, `server/crates/sim/src/game/commands.rs`, `order_planner`, `order_execution`, and command-service tests. |
| Sim core | `server/crates/sim/src/game/mod.rs`, `systems.rs`, `setup.rs`, entity/map/fog/snapshot/building-memory/pathfinding modules, plus broad `game/tests.rs`. Use the public `Game` API as the boundary. |
| Sim movement service | `server/crates/sim/src/game/services/movement/**`, `move_coordinator.rs`, `pathing.rs`, movement tests. Keep movement physics/pathing separate from command acceptance. |
| Sim combat/support | Combat, mortar, smoke, artillery, ability runtime, projection, and related tests. Use this group for support-weapon extraction candidates. |
| AI | `server/crates/ai/src/**`, especially self-play, decision tests, profile actions, and harness artifacts. Keep long/self-play gates explicit. |
| Protocol mirror | `server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `server/crates/sim/src/protocol.rs`, `server/crates/contract/src/lib.rs`, `client/src/protocol.js`, and protocol parity/contracts. Cleanup should be group-level. |
| Balance mirror | `server/crates/rules/src/balance.rs`, `server/crates/rules/src/defs.rs`, `server/crates/rules/src/faction.rs`, `server/src/config.rs`, `server/crates/sim/src/config.rs`, `client/src/config.js`, wiki/faction parity checks. Cleanup should preserve Rust authority and client mirror bounds. |
| Client match shell | `client/src/match.js`, `main.js`, `app.js`, match health/perf/frame recovery/replay/lab room capability modules. This group composes collaborators. |
| Client model | `client/src/state.js`, `client/src/client_intent.js`, `client/src/command_budget.js`, `client/src/command_composer.js`, prediction controller/adapters, progress extrapolator. |
| Client HUD/UI | `client/src/hud.js`, `hud_command_card.js`, `hud_selection_panel.js`, command card descriptors, hotkeys, lobby views, scoreboard, minimap, lab panel, match history, settings panels. |
| Client input | `client/src/input/**`, `client/src/replay_camera_input.js`, `client/src/camera.js`. Keep command-free camera gestures separate from gameplay command composition. |
| Client renderer | `client/src/renderer/**`, `client/src/fog.js`, feedback/art helpers/rigs. Rendering extraction needs browser/pixel smoke, not only Node contracts. |
| Styling | `client/styles.css` and DOM class/id producers across lobby, game, HUD, lab, replay, settings, and match history. Selector names are UI contracts. |
| Contract tests | `tests/client_contracts.mjs`, `tests/protocol_parity.mjs`, `tests/minimap_input_contracts.mjs`, `tests/select-suites.mjs`, live Node suites. Split by contract area before altering assertions. |
| Tooling | `scripts/check-client-architecture.mjs`, `scripts/client-perf-harness.mjs`, docdrift/hotspot tooling. Keep generated or bulky artifacts out of hotspot rankings. |

## Extraction Guidance For Phase 3

Rank these first:

1. Split `tests/client_contracts.mjs` by existing section boundaries with shared helper modules.
   This is the clearest mechanical payoff: low runtime risk and a large reduction in per-review
   context.
2. Split command-service tests and pure command input guards before moving command orchestration.
   This gives leverage on `commands.rs` without changing gameplay semantics.
3. Consider extracting `client/src/hud.js` resource/control-group/command-card DOM helpers where
   existing tests already cover behavior.
4. Consider splitting `server/crates/sim/src/game/tests.rs` into domain modules, especially
   ability/artillery/fog/mortar/movement/scoring/resource/tank-trap clusters.
5. Consider a client-state helper split for transient visual effects or selection/control groups
   only after preserving `GameState` public methods.

Defer or require a separate design plan:

- `server/src/lobby/room_task.rs`, because active room-runtime cleanup owns the surface.
- Protocol mirror cleanup, unless the plan changes Rust and JS mirrors together and preserves
  parity.
- Balance mirror cleanup, unless the plan keeps Rust authoritative and verifies all client-visible
  mirrors.
- CSS splitting, unless the no-build serving model and manual browser smoke are part of the plan.
