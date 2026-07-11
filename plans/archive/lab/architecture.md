# Lab Architecture Hypotheses

This document is a planning artifact, not an approved implementation phase list. It captures the
current product target and the architectural hypotheses that should be tested before cutting
implementation phases.

Current implementation note: lab setup import/export now uses checkpoint-backed setup containers,
and legacy setup JSON is intentionally rejected. Treat older scenario-shaped examples below as
historical planning context unless they have been rewritten to checkpoint-backed setup language.

Related subplans:

- [Room architecture requirements](room/requirements.md) defines the high-level goal that rooms,
  replays, dev scenarios, and labs evolve toward shared room primitives with explicit policies
  instead of duplicated special cases.
- [Session controls](session-controls/plan.md) retires the legacy quickstart/debug preset path, makes
  lab vision per-operator, and adds room-local lab time/timeline controls without introducing lab
  flags or fine-grained permissions.

## Product Contract

The lab is a privileged version of the real game. It should run a real map, a real `Game`, real
players, real teams, real fog projection, real command validation, and the normal client renderer
and HUD wherever possible.

The MVP is scenario setup and omnipotent control:

- Create or join a lab room that starts a real game on any selectable map.
- Create player/team slots for a scenario without going through normal lobby ceremony.
- Spawn, remove, reposition, and reassign existing real units and buildings for those players.
- Set enough player state to make staged scenarios useful, including resources and completed
  research.
- Select and inspect units for any player.
- Issue real gameplay commands as the owning player, through normal command validation.
- Switch the room's lab fog projection between full-world vision, one team's vision, and a union
  of selected teams' vision.
- Save and load legible JSON scenarios.

Visual rig iteration, scratch art hot reload, animation iteration, particle iteration, and balance
number hot reload are explicitly v2+ work. The MVP should not depend on the old `/dev/unit-lab`
canvas preview or on a new visual asset pipeline.

Normal game rules are the default. Privileged controls such as god mode, inert units, unlimited
resources, disabled damage, and frozen cooldowns should be explicit lab toggles, not silent changes
to the simulation. The normal lobby now points experimentation at the lab, and the legacy
quickstart/debug preset path has been retired from active protocol, client, tests, and
source-of-truth docs. Debug-style prebuilt setups should return only as explicit, hand-authored lab
scenarios or presets.

The landed collaborator model is intentionally small: every direct `/lab` URL joiner receives the
omnipotent operator role for that room. `ReadOnly` remains in the protocol for future explicit
viewer modes, and `operatorId` remains compatibility metadata naming the original joiner rather
than the sole mutation authority.

The lab is allowed in production. That means the privilege boundary is room-local, not auth-local:
public users may create their own lab rooms, but lab operations must not affect normal rooms,
global server state, arbitrary files, or other users' rooms.

## Architectural North Star

The lab should be a real room mode around `Game`, not a parallel simulator. The closest existing
foundation is the dev scenario and replay infrastructure:

- `RoomTask` already owns exactly one `Game` in `Phase::InGame`.
- dev scenario rooms already run scripted real `Game` states and can send full-world snapshots.
- replay rooms already have speed, seek, keyframe, and viewer-vision primitives that can be reused
  later for timeline control.
- `Match` already composes the normal renderer, HUD, input, minimap, replay controls, observer
  overlays, and lifecycle teardown from a `StartPayload`.

The lab should therefore compose the normal match screen with lab-specific panels and policies.
It should not become a second client app that paints its own game.

## Ownership Boundaries

### Simulation

`server/crates/sim/src/game/mod.rs` remains the public seam for authoritative state. Lab setup
mutations must enter through typed `Game` lab APIs. `lobby/` must not reach into entity stores,
player state, fog internals, or map internals directly.

The sim owns:

- validating whether a lab spawn, delete, move, owner change, resource change, or research change
  can produce a coherent game state;
- applying accepted mutations;
- recomputing derived state such as supply, spatial indexes, construction ownership, and fog;
- enforcing normal command validation when the operator issues a command as a real player;
- producing snapshots through existing projection methods or lab-specific projection helpers.

Lab APIs must keep the `Game::tick()` path panic-free. Bad entity ids, stale player ids, invalid
coordinates, impossible unit kinds, and invalid research ids should return structured errors or
be ignored intentionally, never panic.

### Room And Lifecycle

`server/src/lobby/room_task.rs` should own the lab room lifecycle, collaborator roles, room-local
lab settings, scenario load/save requests, and snapshot fanout policy. It should call the public
`Game` lab API for mutations and normal `Game::enqueue`-style command flow for gameplay orders.

The room owns:

- `RoomMode::Lab` and any lab-specific `Phase` state;
- the original operator connection id plus per-connection lab roles;
- lab scenario identity and dirty state;
- per-connection lab vision choices plus the default vision for future joins/imports;
- accepted lab operation log entries;
- room-local lab timeline keyframes and accepted operation/issue-as replay entries;
- best-effort autosave or scenario export triggers;
- translating team-based UI choices into current player ids for fog projection.

The room should not know how to mutate an entity. It should know only how to validate that the
request came from a lab operator role, bound the payload, call `Game`, and send a result.

The landed room capability model makes persistence and export explicit room-policy choices.
Normal matches are eligible for match-history rows and durable replay artifacts, replay branches
keep only transient post-match replay capture, dev scenarios suppress replay/history writes, and
labs keep scenario import/export plus a room-local operation log without public storage.

### Wire Protocol

`server/crates/protocol/src/lib.rs`, `server/src/protocol.rs`, `client/src/protocol.js`, and
`docs/design/protocol.md` remain mirrored. Lab messages should use typed protocol DTOs instead of
free-form debug JSON.

A single top-level lab envelope is probably better than adding a dozen top-level message tags:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ClientMessage {
    Lab {
        #[serde(rename = "requestId")]
        request_id: u32,
        op: LabClientOp,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum LabClientOp {
    LoadScenario { source: LabScenarioSource },
    SaveScenario { name: String },
    ResetScenario,
    Spawn { spec: LabSpawnSpec },
    Delete { entity_ids: Vec<u32> },
    MoveEntities { entity_ids: Vec<u32>, x: f32, y: f32 },
    SetOwner { entity_ids: Vec<u32>, owner: u32 },
    SetPlayerResources { player: u32, steel: u32, oil: u32 },
    SetResearch { player: u32, upgrades: Vec<String> },
    IssueCommandAs { player: u32, command: Command },
    SetVision { vision: LabVisionRequest },
    SetFlag { target: LabFlagTarget, flag: LabFlag, enabled: bool },
}
```

Server responses should be explicit:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ServerMessage {
    LabState { state: LabState },
    LabResult {
        #[serde(rename = "requestId")]
        request_id: u32,
        result: LabResult,
    },
}
```

`LabState` is room/control metadata, not the world snapshot. The normal `snapshot` message should
continue to carry world state.

### Client App Shell

The client should add a lab route and lab app-shell module that starts a real `Match` with a lab
start payload, then mounts lab panels around it. `Match` should not import lab panels directly.

The app shell owns:

- joining or creating a lab room;
- map/scenario picker entry state;
- creating `Match` with `payload.lab` metadata;
- mounting and destroying `LabPanel`;
- passing lab collaborators into `Match`, input, HUD, and replay controls by dependency injection.

### Client Control Policy

Omnipotent control should be explicit client state, not a fake `playerId`. The current selection
path already uses `state.isOwnOwner()` as an ownership predicate in some places; the lab should
make that policy first-class instead of special-casing every control surface.

Hypothesis:

```js
export class LabControlPolicy {
  canInspect(entity) { return entity.owner !== 0; }
  canSelect(entity) { return entity.owner !== 0; }
  commandIssuerForSelection(selection, commandKind) { /* owner or rejection */ }
  partitionSelectionByOwner(selection) { /* Map<owner, ids> */ }
}
```

Mixed-owner selections are allowed for inspection and batch setup operations. Mixed-owner gameplay
orders should be rejected by default unless the UI explicitly partitions the selection and sends
one command per owning player. The server should enforce the same rule even if the client UI gets it
wrong.

### Renderer, Fog, And HUD

Renderer, minimap, fog, and HUD modules should remain views over the normal `GameState` plus small
lab collaborators. They should not know how to call scenario storage or mutate the sim.

The HUD should keep normal command cards where they are authentic. Lab-only controls belong in
lab panels or contextual lab action bars, not inside the normal command model unless the action is
a real gameplay command.

UI hiding for screenshots and demo capture is a later shared shell feature. The lab architecture
should not block it, but it is not part of the MVP.

### Scenario Storage

Scenario storage is server-owned when the server saves scenarios. The scenario format should be
legible JSON and versioned from day one.

Production access makes arbitrary server disk writes a real boundary. MVP storage can start with:

- import/export JSON in the browser for all environments;
- bundled read-only scenarios checked into the repo;
- local-dev server-side save under a known lab scenario directory;
- a later DB or moderated storage path if production server-side saving becomes useful.

Scenario load should go through a typed `LabScenarioStore`; HTTP handlers and WebSocket handlers
should not read arbitrary paths supplied by the browser.

### Replay And Timeline

Lab room-time controls now use the neutral room-time message family for shared pause, resume,
speed, one-tick step, relative seek, and absolute timeline seek. Timeline history is room-local and
in-memory: labs record a baseline keyframe, periodic cloned `Game` keyframes, accepted lab
operations, and issue-as commands in authoritative tick order, then rebuild seeks from the nearest
retained keyframe. If an operator seeks into the past and accepts a new lab operation or issue-as
command, future entries are truncated instead of creating branch UI.

Normal gameplay commands already have a command log in `Game`. Lab operations keep their own room
timeline stream so keyframe rebuilds can replay both normal commands and privileged lab mutations
in tick order. Replay branch-from-lab, durable rewind artifacts, and true timeline editing remain
future work.

## Durable Primitives

### `RoomMode::Lab`

Hypothesis:

```rust
pub(super) enum RoomMode {
    Normal,
    DevSelfPlay(DevSelfPlayConfig),
    DevScenario(DevScenarioConfig),
    Replay { artifact: ReplayArtifactV1 },
    ReplayBranch { seed: ReplayBranchSeed },
    Lab(LabRoomConfig),
}

pub(super) struct LabRoomConfig {
    pub scenario: Option<LabScenarioSource>,
    pub map: Option<String>,
    pub seed: u32,
}
```

`RoomMode::Lab` should create a real `Game` with lab-selected players and map data. It should not
reuse `DevScenarioConfig`, because dev scenarios are scripted tests and the lab is an operator
workspace.

### `LabSession`

Hypothesis:

```rust
struct LabSession {
    public_id: String,
    operator_id: u32,
    viewer_roles: HashMap<u32, LabStartRole>,
    viewer_vision_modes: HashMap<u32, LabVisionMode>,
    scenario_id: Option<String>,
    dirty: bool,
    op_log: Vec<LabOpLogEntry>,
    default_vision_mode: LabVisionMode,
    timeline: LabTimeline,
    flags: LabFlagState,
}
```

`LabSession` belongs to the room task. It is room control state, not simulation state. Simulation
flags that affect damage, deaths, cooldowns, or command execution should be mirrored into `Game`
through typed lab APIs.

### Checkpoint-Backed Lab Setup

Checkpoint-backed lab setup should be snapshot-like, but it should not literally be the over-the-wire
`Snapshot`. Snapshots are recipient projections with fog filtering, transient events, compact
network fields, and client convenience data. A saved setup is authoritative setup data.

Sketch:

```json
{
  "schemaVersion": 1,
  "kind": "labCheckpointScenario",
  "name": "two_tank_lines",
  "map": {
    "name": "River Crossing",
    "seed": 12345
  },
  "players": [
    {
      "id": 1,
      "teamId": 1,
      "factionId": "kriegsia",
      "name": "Blue",
      "color": "#4d8dff",
      "isAi": false
    },
    {
      "id": 2,
      "teamId": 2,
      "factionId": "kriegsia",
      "name": "Red",
      "color": "#e34d4d",
      "isAi": false
    }
  ],
  "playerState": [
    {
      "playerId": 1,
      "steel": 1000,
      "oil": 1000,
      "upgrades": ["tank_unlock"]
    }
  ],
  "entities": [
    {
      "stableId": "blue-tank-1",
      "kind": "tank",
      "owner": 1,
      "x": 2400,
      "y": 1600,
      "facing": 0,
      "hp": 400
    }
  ],
  "lab": {
    "vision": { "mode": "all" },
    "flags": []
  }
}
```

Entity ids may be stable within the JSON for debugging, but import should be allowed to remap them
to current runtime ids and return an id map. The format should start with the minimum useful fields
and grow only when a concrete scenario needs more state.

### `LabOp`

`LabOp` is the authoritative operation stream for privileged mutations.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum LabOp {
    Spawn(LabSpawnSpec),
    Delete { entity_ids: Vec<u32> },
    MoveEntities { entity_ids: Vec<u32>, positions: Vec<LabEntityPosition> },
    SetOwner { entity_ids: Vec<u32>, owner: u32 },
    SetPlayerResources { player: u32, steel: u32, oil: u32 },
    SetResearch { player: u32, upgrades: Vec<String> },
    SetEntityFlags { entity_ids: Vec<u32>, flags: Vec<LabEntityFlag> },
    SetGlobalFlags { flags: Vec<LabGlobalFlag> },
}

pub struct LabOpLogEntry {
    pub tick: u32,
    pub request_id: u32,
    pub operator: u32,
    pub op: LabOp,
}
```

Every accepted privileged mutation should be loggable. Rejected requests should return errors but
do not need to enter the replayable op log.

### `LabVisionMode`

The product-level concept is teams, even though current snapshot projection often works from
player ids. The room should translate team choices to current player ids when building snapshots.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum LabVisionMode {
    All,
    Team { team_id: TeamId },
    Teams { team_ids: Vec<TeamId> },
    None,
}
```

`All` can use full-world projection. `Team` and `Teams` should use a union of the selected teams'
real player vision, not omniscient vision for those players. `None` is useful for screenshots and
testing hidden-information UI, but can be deferred if it adds friction.

### `LabFlags`

God mode is not one flag. It is a collection of explicit toggles that can be global, per-player, or
per-entity.

Initial candidates:

- `invulnerable`: damage does not reduce HP.
- `preventDeath`: lethal outcomes clamp at 1 HP.
- `damageDisabled`: weapons and abilities do no damage.
- `inert`: selected units do not acquire, attack, move, gather, build, or train.
- `cooldownsFrozen`: ability cooldown timers stop changing.
- `unlimitedResources`: resource checks pass for selected players.
- `unlimitedSupply`: supply checks pass for selected players.

Most of these are not MVP-critical. The architecture should reserve a typed flag path so they do
not get added later as scattered debug booleans.

### `LabSetupStore`

Hypothesis:

```rust
pub trait LabSetupStore {
    fn list(&self) -> Result<Vec<LabScenarioSummary>, LabScenarioError>;
    fn load(&self, source: LabScenarioSource) -> Result<LabCheckpointScenarioV1, LabScenarioError>;
    fn save(&self, setup: &LabCheckpointScenarioV1) -> Result<LabScenarioSummary, LabScenarioError>;
}
```

The first implementation does not need an async trait unless storage actually needs it. The
important boundary is that path validation, schema validation, version upgrades, and environment
storage policy live here instead of in `RoomTask`.

## Game API Sketch

The sim should expose a narrow lab entry point rather than a generic debug backdoor:

```rust
impl Game {
    pub fn new_lab(
        players: Vec<PlayerInit>,
        map_name: &str,
        seed: u32,
    ) -> Result<Self, LabError>;

    pub fn export_lab_checkpoint_scenario(
        &self,
        metadata: LabScenarioMetadata,
    ) -> Result<LabCheckpointScenarioV1, LabError>;

    pub fn restore_lab_checkpoint_scenario(setup: LabCheckpointScenarioV1) -> Result<Self, LabError>;

    pub fn apply_lab_op(&mut self, op: LabOp) -> Result<LabOpResult, LabError>;

    pub fn issue_lab_command_as(
        &mut self,
        player: u32,
        command: SimCommand,
    ) -> Result<(), LabError>;
}
```

`issue_lab_command_as` should preserve normal command validation. It may wrap `enqueue`, but it
should still reject or no-op commands that the named player could not issue in a real game.

`apply_lab_op` should dispatch internally to focused helpers. It should not become a dumping ground
where unrelated systems mutate each other's fields. If an operation needs to touch production,
supply, fog, pathing, or ability runtime state, the helper should live near the owning system and
return a clear result.

## Client API Sketch

Transport builders should mirror the Rust protocol:

```js
export const C = Object.freeze({
  // existing tags...
  LAB: "lab",
});

export function lab(requestId, op) {
  return { t: C.LAB, requestId, ...op };
}

export const LabOps = Object.freeze({
  spawn: (spec) => ({ op: "spawn", spec }),
  delete: (entityIds) => ({ op: "delete", entityIds }),
  moveEntities: (entityIds, x, y) => ({ op: "moveEntities", entityIds, x, y }),
  setOwner: (entityIds, owner) => ({ op: "setOwner", entityIds, owner }),
  issueCommandAs: (player, command) => ({ op: "issueCommandAs", player, command }),
  setVision: (vision) => ({ op: "setVision", vision }),
});
```

`LabPanel` should talk to an injected `LabClient` service instead of owning `Net` directly:

```js
export class LabClient {
  constructor({ net, onResult, onState }) {}
  spawn(spec) {}
  delete(entityIds) {}
  issueCommandAs(playerId, command) {}
  setVision(vision) {}
  saveScenario(name) {}
  loadScenario(source) {}
  destroy() {}
}
```

`Match` should receive lab control through options:

```js
new Match(net, payload, toast, devWatch, audio, statusBadge, diagnostics, {
  lab: payload.lab,
  controlPolicy: new LabControlPolicy(payload.lab),
  labClient,
});
```

This keeps lab orchestration in the app shell while allowing selection, command composition, HUD
affordances, and input routing to consult the same lab control policy.

## MVP Implementation Status

The MVP slice validated these architecture choices:

1. Add a `RoomMode::Lab` that creates a real `Game` from a selected map and a default two-team
   player template.
2. Add `StartPayload.lab` metadata and a client `/lab` route that starts normal `Match` with lab
   mode enabled.
3. Add per-operator lab vision controls: all vision, one team, and selected-team union. One
   operator changing vision does not change another operator's projection.
4. Add spawn/delete/move/set-owner operations for existing unit and building kinds.
5. Add omnipotent selection and issue-command-as-owner for single-owner selections.
6. Add JSON import/export for scenarios with map, players, teams, resources, upgrades, and
   entities.
7. Promote later direct lab joiners to the same operator role as the first joiner, while preserving
   `ReadOnly` as a future explicit viewer role.
8. Add shared lab room-time controls for pause/resume, speed, one-tick step, relative seek, and
   absolute timeline seek using room-local keyframes and recorded lab entries.

This slice replaces the most important debug-mode workflows: set up a map, stage two
sides, issue real orders, observe with chosen fog, and save the setup.

The current lab still deliberately excludes hand-authored preset libraries, optional lab flags,
presence/permissions beyond the all-operators model, durable public scenario libraries,
branch-from-lab, visual hot reload, and `/dev/scenario` migration. Those should each get a
follow-up design rather than broadening the current lab operation envelope.

## Verification Strategy

The implementation phases should use focused checks before relying on the full PR gate:

- Protocol parity for any lab messages or start payload changes.
- `Game` unit tests for every accepted and rejected lab op.
- Scenario round-trip tests proving load/export preserves map, players, teams, resources,
  upgrades, and entity placement.
- Room-task tests proving only connections with the operator role can mutate the lab, collaborator
  operators are attributed in the operation log, preserved read-only roles are rejected, and lab
  requests do not affect normal or replay rooms.
- Client architecture check for new lab modules and dependency injection boundaries.
- Client contract tests for control policy, mixed-owner rejection, and lab protocol builders.
- Manual browser smoke: create lab, select map, spawn opposing units, issue move/attack commands,
  switch each operator to a different vision mode, pause/step/resume shared time, seek the lab
  timeline, save JSON, and reload JSON.

## Restart And Recovery Hypothesis

Best-effort restart survival should be scenario-based at first, not tick-perfect. The useful MVP
behavior is:

- accepted setup operations can dirty an autosave/export scenario;
- the operator can save the current setup to JSON at any point;
- after a server restart, the scenario can be loaded and restaged quickly;
- room-local timeline/keyframes support rewind while the room is alive, but are not durable restart
  artifacts;
- live mid-fight state, exact cooldowns, projectile state, and command queues are not guaranteed
  across a server restart.

If durable rewind or branch-from-lab becomes a near-term target, the lab timeline should be
persisted beside periodic `Game::clone_for_replay_keyframe()` keyframes or a serializable lab
checkpoint. That is later work.

## Risks And Open Hypotheses

- Scenario JSON should not literally reuse `Snapshot`, but it should stay close enough that fields
  are easy to compare during debugging.
- Public production access conflicts with naive server-side save-to-disk. Import/export can ship
  first; persistent public scenario libraries need a storage and moderation decision.
- Teams are product-visible, but units are owned by players. The lab UI can expose teams while the
  server APIs still use owner player ids for command validation.
- Mixed-owner gameplay orders need a strict policy. Default rejection is safer than pretending a
  single real player issued the whole command.
- Spawn and move operations need placement rules. MVP should preserve normal map and collision
  validity unless an explicit later flag allows impossible setups.
- Lab flags can quickly leak into normal sim behavior if they are scattered. They should be typed,
  room-scoped, and absent from normal rooms by construction.
- Branch-from-lab needs an explicit product model for what becomes durable, who can share it, and
  how future-history truncation maps to public replay artifacts.
- The old unit design lab can stay alive as a separate visual scratchpad until visual iteration is
  redesigned as v2+ on top of the real lab.
