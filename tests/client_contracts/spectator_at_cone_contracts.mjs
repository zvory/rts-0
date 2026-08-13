// tests/client_contracts/spectator_at_cone_contracts.mjs
// Spectator-perspective anti-tank cone projection contracts.

import { assert } from "./assertions.mjs";
import { KIND, LAB_ROLE, SETUP } from "../../client/src/protocol.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { createControlPolicyProjection } from "../../client/src/control_policy_projection.js";
import { buildRendererFeedbackView } from "../../client/src/renderer/feedback_view_model.js";

const visibleEntities = [
  {
    id: 301,
    owner: 2,
    kind: KIND.ANTI_TANK_GUN,
    x: 320,
    y: 256,
    facing: 0,
    setupState: SETUP.DEPLOYED,
  },
  {
    id: 302,
    owner: 2,
    kind: KIND.ANTI_TANK_GUN,
    x: 384,
    y: 256,
    facing: 0,
    setupState: SETUP.PACKED,
  },
  {
    id: 303,
    owner: 3,
    kind: KIND.ANTI_TANK_GUN,
    x: 448,
    y: 256,
    facing: 0,
    setupState: SETUP.DEPLOYED,
  },
  {
    id: 304,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 512,
    y: 256,
    facing: 0,
    setupState: SETUP.DEPLOYED,
  },
];

const state = {
  playerId: 1,
  players: [
    { id: 1, teamId: 1 },
    { id: 2, teamId: 2 },
    { id: 3, teamId: 1 },
  ],
  selectedEntities() { return []; },
};

const spectatorView = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    observerView: { mode: "player", playerId: 1 },
  },
  { entities: visibleEntities },
);
assert(
  spectatorView.enemyAntiTankGunThreats().map((entity) => entity.id).join(",") === "301,303,304",
  "live and replay spectators render every deployed anti-tank cone in the current projection",
);

const playerOneSpectatorView = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    playerResources: [{ id: 1 }],
    observerView: { mode: "player", playerId: 1 },
  },
  { entities: visibleEntities },
);
assert(
  playerOneSpectatorView.enemyAntiTankGunThreats().map((entity) => entity.id).join(",") === "301,303,304",
  "a player-vision spectator projection still renders every deployed anti-tank gun it receives",
);

const playerOneSnapshotDuringSwitch = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    playerResources: [{ id: 1 }],
    observerView: { mode: "player", playerId: 2 },
  },
  { entities: visibleEntities },
);
assert(
  playerOneSnapshotDuringSwitch.enemyAntiTankGunThreats().map((entity) => entity.id).join(",") === "301,303,304",
  "a pending vision switch does not change the observer cone treatment before the new snapshot arrives",
);

const playerTwoSpectatorView = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    playerResources: [{ id: 2 }],
    observerView: { mode: "player", playerId: 1 },
  },
  { entities: visibleEntities },
);
assert(
  playerTwoSpectatorView.enemyAntiTankGunThreats().map((entity) => entity.id).join(",") === "301,303,304",
  "switching authoritative snapshots keeps observer cones independent of enemy relationships",
);

const rememberedDuringPlayerOneView = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    playerResources: [{ id: 1 }],
  },
  {
    entities: [],
    rememberedEnemyAntiTankGunThreats: [{
      id: 306,
      owner: 2,
      x: 320,
      y: 256,
      facing: 0,
    }],
  },
);
assert(
  rememberedDuringPlayerOneView.enemyAntiTankGunThreats()[0]?.threatMemory === true,
  "a single-player spectator projection renders that player's server-authored stale threat memory",
);

const sameMemoryDuringPlayerTwoView = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    playerResources: [{ id: 2 }],
  },
  {
    entities: [],
    rememberedEnemyAntiTankGunThreats: [{
      id: 306,
      owner: 2,
      x: 320,
      y: 256,
      facing: 0,
    }],
  },
);
assert(
  sameMemoryDuringPlayerTwoView.enemyAntiTankGunThreats()[0]?.threatMemory === true,
  "spectators retain any server-projected anti-tank memory independently of team relationship",
);

const unionSpectatorView = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    playerResources: [{ id: 1 }, { id: 2 }],
  },
  { entities: visibleEntities },
);
assert(
  unionSpectatorView.enemyAntiTankGunThreats().map((entity) => entity.id).join(",") === "301,303,304",
  "multi-player union and omniscient projections show every deployed anti-tank cone",
);

const labSpectatorView = buildRendererFeedbackView(
  {
    ...state,
    spectator: true,
    playerResources: [{ id: 1 }],
  },
  {
    entities: visibleEntities,
    controlPolicy: createControlPolicyProjection(createLabControlPolicy({
      metadata: { role: LAB_ROLE.OPERATOR },
    })),
  },
);
assert(
  labSpectatorView.enemyAntiTankGunThreats().length === 1,
  "Lab operators use the same authoritative player-view threat projection",
);
