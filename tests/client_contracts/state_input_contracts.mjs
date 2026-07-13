// tests/client_contracts/state_input_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertApprox,
  assertDeepEqual,
  assertHasGetter,
  assertHasMethod,
} from "./assertions.mjs";
import { GameState } from "../../client/src/state.js";
import {
  ARTILLERY_SHELL_DELAY_TICKS,
  COLORS,
  BASE_COMMAND_SUPPLY_CAP,
  RESOURCE_AMOUNTS,
  STATS,
} from "../../client/src/config.js";
import { commandWithinBudget } from "../../client/src/command_budget.js";
import {
  HUD,
  groupCooldownClocks,
  playerHasCompletedKind,
} from "../../client/src/hud.js";
import {
  buildCommandCardDescriptors,
  factionCommandId,
} from "../../client/src/hud_command_card.js";
import {
  DEFAULT_FACTION_ID,
  ABILITY,
  ABILITY_OBJECT_KIND,
  EVENT,
  KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  SETUP,
  STATE,
  TERRAIN,
  cmd,
} from "../../client/src/protocol.js";
import {
  Input,
  footprintValidAgainstEntities,
} from "../../client/src/input/index.js";
import { buildSelectionScene } from "../../client/src/input/selection_projection.js";
import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import {
  footprintPlacementBlocker,
  movementBodyClass,
  pointHitsOrientedVehicle,
  placementPolicyForBuilding,
} from "../../client/src/input/placement.js";
import {
  buildTankTrapLineSites,
  tankTrapBuildCommands,
  tankTrapLineTiles,
  validTankTrapLineSites,
} from "../../client/src/input/tank_trap_line.js";
import { armPostQuickCastSelectionGuard } from "../../client/src/input/quick_cast_selection_guard.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Minimap } from "../../client/src/minimap.js";
import { _drawBuilding } from "../../client/src/renderer/buildings.js";
import {
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawAntiTankGunSetupPreview,
} from "../../client/src/renderer/feedback.js";

import { RecordingGraphics } from "./pixi_fakes.mjs";

function publishSelectionTestScene(input, entities = input.state?.entitiesInterpolated?.(1) || []) {
  const sourceMap = input.state?.map || {};
  const map = {
    width: Number.isFinite(sourceMap.width) ? sourceMap.width : 64,
    height: Number.isFinite(sourceMap.height) ? sourceMap.height : 64,
    tileSize: Number.isFinite(sourceMap.tileSize) ? sourceMap.tileSize : 32,
  };
  const width = input.dom?.clientWidth || 640;
  const height = input.dom?.clientHeight || 480;
  input.selectionScene = buildSelectionScene({
    entities,
    tileSize: map.tileSize,
    projection: createOrthographicProjectionSnapshot({
      x: 0,
      y: 0,
      zoom: 1,
      worldW: map.width * map.tileSize,
      worldH: map.height * map.tileSize,
      viewW: width,
      viewH: height,
    }, 1920),
  });
}

function commandCardCtx({
  selection = [],
  entities = selection,
  resources = { steel: 1000, oil: 1000 },
  optimisticProduction = [],
  upgrades = [],
  playerId = 1,
  commandCardMode = null,
  commandTarget = null,
  spectator = false,
  commandSurfaceEnabled = undefined,
  factionId = DEFAULT_FACTION_ID,
  state = null,
  controlPolicy = null,
} = {}) {
  return {
    spectator,
    ...(typeof commandSurfaceEnabled === "boolean" ? { commandSurfaceEnabled } : {}),
    playerId,
    factionId,
    selection,
    state,
    resources,
    optimisticProduction,
    upgrades,
    commandCardMode,
    commandTarget,
    controlPolicy,
    groupCooldownClocks,
    playerHasCompleteKind: (kind) => playerHasCompletedKind(entities, playerId, kind),
  };
}

function defaultFactionCommandId(family, subject) {
  return factionCommandId(DEFAULT_FACTION_ID, family, subject);
}

function commandButtons(card) {
  return card.slots.filter(Boolean);
}

function buttonByAction(card, action) {
  return commandButtons(card).find((button) => button.action === action);
}

function buttonByLabel(card, label) {
  return commandButtons(card).find((button) => button.label === label);
}

// GameState
// ---------------------------------------------------------------------------
{
  const start = {
    playerId: 1,
    tick: 0,
    map: {
      width: 4,
      height: 4,
      tileSize: 32,
      terrain: new Array(16).fill(0),
      resources: [
        { id: 200, kind: KIND.STEEL, x: 64, y: 96 },
        { id: 201, kind: KIND.OIL, x: 96, y: 96 },
      ],
    },
    players: [
      { id: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
      { id: 2, teamId: 7, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
      { id: 3, teamId: 7, name: "C", color: "#0000ff", startTileX: 3, startTileY: 3 },
    ],
  };
  const state = new GameState(start);
  assert(state instanceof GameState, "GameState constructor should return an instance");
  assert(state.playerId === 1, "GameState.playerId");
  assert(state.startInfo === start, "GameState.startInfo");
  assert(state.map.width === 4, "GameState.map");
  assert(state.map.resources.length === 2, "GameState keeps start payload resources");
  assert(state.resourceById.get(200).kind === KIND.STEEL, "GameState indexes resources by id");
  assert(state.resourceById.get(200).remaining === RESOURCE_AMOUNTS[KIND.STEEL], "steel defaults to full known amount");
  assert(state.resourceById.get(201).remaining === 962, "oil defaults to full known amount");
  assert(Array.isArray(state.players), "GameState.players");
  assert(state.playerById(1)?.teamId === 1, "GameState defaults missing teamId to singleton FFA");
  assert(state.teamIdForPlayer(2) === 7, "GameState.teamIdForPlayer returns explicit team");
  assert(state.isOwnOwner(1), "GameState.isOwnOwner detects local owner");
  assert(state.isAllyOwner(2) === false, "GameState.isAllyOwner treats singleton FFA as non-allied");
  assert(state.isEnemyOwner(2), "GameState.isEnemyOwner detects another team");
  assert(state.isNeutralOwner(0), "GameState.isNeutralOwner detects neutral owner");
  const allyState = new GameState({ ...start, playerId: 2 });
  assert(allyState.isAllyOwner(3), "GameState.isAllyOwner detects shared team");
  assert(!allyState.isEnemyOwner(3), "GameState.isEnemyOwner excludes shared team");
  const localTeamState = new GameState({
    ...start,
    players: [
      { id: 1, teamId: 7, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
      { id: 2, teamId: 7, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
      { id: 3, teamId: 8, name: "C", color: "#0000ff", startTileX: 3, startTileY: 3 },
    ],
  });
  assert(localTeamState.isAllyOwner(2), "GameState.isAllyOwner classifies local-player allies from start.players");
  assert(localTeamState.isEnemyOwner(3), "GameState.isEnemyOwner classifies hostile teams from start.players");
  assert(!localTeamState.isEnemyOwner(2), "GameState.isEnemyOwner excludes local-player allies");
  assertHasMethod(state, "applySnapshot", "GameState");
  assertHasMethod(state, "entitiesInterpolated", "GameState");
  assertHasGetter(state, "prevRecvTime", "GameState");
  assertHasGetter(state, "currRecvTime", "GameState");
  assert(state.prevRecvTime === null, "prevRecvTime null before snapshots");
  assert(state.currRecvTime === null, "currRecvTime null before snapshots");
  assert(state.resources !== undefined, "GameState.resources");
  assert(Array.isArray(state.events), "GameState.events");
  assert(!("resourceMiningPreview" in state), "GameState no longer exposes resource preview shims");
  assert(!("antiTankGunSetupPreview" in state), "GameState no longer exposes support preview shims");
  assert(!("updateResourceMiningPreview" in state), "GameState no longer exposes preview update shims");
  assert(state.selection instanceof Set, "GameState.selection");
  assert(state.diagnostics.movementPaths === MOVEMENT_PATH_DIAGNOSTICS.NONE, "GameState defaults movement path diagnostics to none");
  assert(state.debugPathOverlaysAvailable === false, "GameState hides waypoint diagnostics by default");
  assert(state.debugPathOverlaysEnabled === false, "GameState leaves waypoint diagnostics off by default");
  assertHasMethod(state, "setSelection", "GameState");
  assertHasMethod(state, "addToSelection", "GameState");
  assertHasMethod(state, "clearSelection", "GameState");
  assertHasMethod(state, "selectedEntities", "GameState");
  assertHasMethod(state, "entityById", "GameState");
  assert(!("commandCardMode" in state), "GameState no longer exposes command-card intent shims");
  assert(!("openWorkerBuildMenu" in state), "GameState no longer exposes command-card methods");
  assert(!("placement" in state), "GameState no longer exposes placement intent shims");
  assert(!("beginPlacement" in state), "GameState no longer exposes placement methods");

  const debugState = new GameState({
    ...start,
    diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.OWNER_ONLY },
    map: {
      ...start.map,
      resources: start.map.resources.map((resource) => ({ ...resource })),
    },
  });
  assert(debugState.debugPathOverlaysAvailable === true, "GameState exposes advertised waypoint diagnostics");
  assert(debugState.debugPathOverlaysEnabled === true, "GameState enables advertised waypoint diagnostics");
  assert(debugState.showAllDebugPathOverlays === false, "owner-only waypoint diagnostics stay selection scoped");

  const fullDiagnosticState = new GameState({
    ...start,
    diagnostics: { movementPaths: MOVEMENT_PATH_DIAGNOSTICS.ALL },
    map: {
      ...start.map,
      resources: start.map.resources.map((resource) => ({ ...resource })),
    },
  });
  assert(fullDiagnosticState.showAllDebugPathOverlays === true, "full waypoint diagnostics may draw every projected path");

  // Snapshot buffering
  const t0 = performance.now();
  state.applySnapshot({
    tick: 0,
    steel: 10,
    oil: 5,
    supplyUsed: 2,
    supplyCap: 10,
    entities: [{ id: 1, owner: 1, kind: "worker", x: 10, y: 20, hp: 40, maxHp: 40, state: "idle" }],
    resourceDeltas: [{ id: 200, remaining: 1498 }],
    trenches: [{ id: 300, x: 96, y: 128, radiusTiles: 0.375 }],
    events: [],
  });
  assert(state.currRecvTime !== null, "currRecvTime set after first snapshot");
  assert(state.prevRecvTime === null, "prevRecvTime still null after one snapshot");
  assert(state.resources.steel === 10, "resources updated");
  assert(state.trenches[0]?.id === 300, "trenches updated");
  assert(state.entityById(200).kind === KIND.STEEL, "static resources are available as local entities");
  assert(state.entityById(200).remaining === 1498, "resourceDeltas update known resource state");

  state.applySnapshot({
    tick: 1,
    steel: 12,
    oil: 5,
    supplyUsed: 2,
    supplyCap: 10,
    entities: [{ id: 1, owner: 1, kind: "worker", x: 15, y: 25, hp: 40, maxHp: 40, state: "idle" }],
    events: [{ e: "death", id: 200, x: 64, y: 96, kind: KIND.STEEL }],
  });
  assert(state.prevRecvTime !== null, "prevRecvTime set after two snapshots");
  assert(state.resourceById.get(200).remaining === 0, "visible resource death tombstones known resource");
  assert(state.entityById(200) === undefined, "depleted resources are not exposed as local entities");
  assert(state.entityById(201).remaining === 962, "untouched resources keep their last-known amount");
  const artilleryState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  artilleryState.applySnapshot({
    tick: 10,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [{ id: 10, owner: 1, kind: KIND.ARTILLERY, x: 300, y: 340, hp: 100, maxHp: 100, state: STATE.IDLE, weaponFacing: 0 }],
    events: [
      { e: EVENT.ARTILLERY_TARGET, from: 10, x: 320, y: 352, radiusTiles: 3, delayTicks: ARTILLERY_SHELL_DELAY_TICKS },
      { e: EVENT.ARTILLERY_IMPACT, x: 336, y: 368, radiusTiles: 3 },
    ],
  });
  assert(artilleryState.liveArtilleryTargets(performance.now()).length === 1, "artillery target event creates a live marker");
  assert(artilleryState.liveArtilleryLaunches(performance.now()).length === 1, "artillery target event creates launch dust");
  assert(artilleryState.weaponRecoil(10, KIND.ARTILLERY, performance.now()) > 0, "artillery target event starts firing-gun recoil");
  assert(artilleryState.liveArtilleryImpacts(performance.now()).length === 1, "artillery impact event creates a live explosion");
  assert(
    artilleryState.visibleTiles.length === 0,
    "artillery visual events do not stamp or extend client fog visibility",
  );

  const artilleryRevealState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  artilleryRevealState.applySnapshot({
    tick: 11,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [],
    events: [{
      e: EVENT.ATTACK,
      from: 99,
      to: 99,
      reveal: {
        owner: 2,
        kind: KIND.ARTILLERY,
        x: 512,
        y: 544,
        facing: 0,
        weaponFacing: 0,
        setupState: SETUP.DEPLOYED,
      },
    }],
  });
  assert(artilleryRevealState.entityById(99)?.shotReveal === true, "artillery self-attack event creates a fog shot reveal");
  assert(artilleryRevealState.liveMuzzleFlashes(performance.now()).length === 0, "artillery self-reveal does not draw a tracer");
  assert(artilleryRevealState.weaponRecoil(99, KIND.ARTILLERY, performance.now()) > 0, "artillery self-reveal still recoils the gun");

  const overpenEventState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  overpenEventState.applySnapshot({
    tick: 12,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [{ id: 22, owner: 2, kind: KIND.WORKER, x: 166, y: 108, hp: 30, maxHp: 40, state: STATE.IDLE }],
    events: [{ e: EVENT.OVERPENETRATION, to: 22 }],
  });
  assert(overpenEventState.liveMuzzleFlashes(performance.now()).length === 0, "overpenetration event does not draw a tracer");
  assert(overpenEventState.weaponRecoil(22, KIND.WORKER, performance.now()) === 0, "overpenetration event does not trigger weapon recoil");

  // Interpolation clamps alpha to [0,1]
  const entsNeg = state.entitiesInterpolated(-0.5);
  const entsOver = state.entitiesInterpolated(1.5);
  const entsMid = state.entitiesInterpolated(0.5);
  const midWorker = entsMid.find((e) => e.id === 1);
  assert(entsMid.length === 2 && midWorker, "entitiesInterpolated returns units and live known resources");
  assert(!entsMid.some((e) => e.id === 200), "entitiesInterpolated omits depleted resources");
  assert(midWorker.x >= 10 && midWorker.x <= 15, "interpolation works for moving units");
  assert(!("facing" in midWorker), "entitiesInterpolated does not add missing facing");

  const angleState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  angleState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 10, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "move", facing: 0 },
      {
        id: 11,
        owner: 1,
        kind: "tank",
        x: 0,
        y: 0,
        hp: 100,
        maxHp: 100,
        state: "move",
        facing: (170 * Math.PI) / 180,
        weaponFacing: (170 * Math.PI) / 180,
      },
      { id: 13, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle", facing: 0.5 },
      { id: 14, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle" },
    ],
    events: [],
  });
  angleState.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 10,
    entities: [
      { id: 10, owner: 1, kind: "worker", x: 10, y: 20, hp: 40, maxHp: 40, state: "move", facing: Math.PI / 2 },
      {
        id: 11,
        owner: 1,
        kind: "tank",
        x: 0,
        y: 0,
        hp: 100,
        maxHp: 100,
        state: "move",
        facing: (-170 * Math.PI) / 180,
        weaponFacing: (-170 * Math.PI) / 180,
      },
      { id: 12, owner: 1, kind: "worker", x: 5, y: 5, hp: 40, maxHp: 40, state: "idle", facing: 1.25 },
      { id: 13, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle" },
      { id: 14, owner: 1, kind: "worker", x: 0, y: 0, hp: 40, maxHp: 40, state: "idle", facing: 0.75 },
    ],
    events: [],
  });
  const angleEnts = angleState.entitiesInterpolated(0.5);
  const quarterTurn = angleEnts.find((e) => e.id === 10);
  const wrapTurn = angleEnts.find((e) => e.id === 11);
  const newFacing = angleEnts.find((e) => e.id === 12);
  const missingCurrentFacing = angleEnts.find((e) => e.id === 13);
  const missingPriorFacing = angleEnts.find((e) => e.id === 14);
  assertApprox(quarterTurn.x, 5, 0.001, "x interpolation still works");
  assertApprox(quarterTurn.y, 10, 0.001, "y interpolation still works");
  assertApprox(quarterTurn.facing, Math.PI / 4, 0.001, "facing interpolates between snapshots");
  assertApprox(
    Math.abs(wrapTurn.facing),
    Math.PI,
    0.001,
    "facing interpolation uses the short path across angle wrap",
  );
  assertApprox(
    Math.abs(wrapTurn.weaponFacing),
    Math.PI,
    0.001,
    "weaponFacing interpolation uses the short path across angle wrap",
  );
  assertApprox(newFacing.facing, 1.25, 0.001, "missing prior entity keeps current facing");
  assert(!("facing" in missingCurrentFacing), "missing current facing does not add a field");
  assertApprox(missingPriorFacing.facing, 0.75, 0.001, "missing prior facing keeps current facing");

  // Selection resolves against current snapshot
  state.setSelection([1, 999]);
  const sel = state.selectedEntities();
  assert(sel.length === 1 && sel[0].id === 1, "selectedEntities drops stale ids");

  const budgetSelectionState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  const budgetRiflemen = Array.from({ length: 30 }, (_, index) => ({
    id: 300 + index,
    owner: 1,
    kind: KIND.RIFLEMAN,
    x: index * 4,
    y: 0,
    hp: 40,
    maxHp: 40,
    state: STATE.IDLE,
  }));
  const budgetTanks = Array.from({ length: 5 }, (_, index) => ({
    id: 400 + index,
    owner: 1,
    kind: KIND.TANK,
    x: index * 12,
    y: 20,
    hp: 100,
    maxHp: 100,
    state: STATE.IDLE,
  }));
  const budgetExtraTank = {
    id: 455,
    owner: 1,
    kind: KIND.TANK,
    x: 100,
    y: 20,
    hp: 100,
    maxHp: 100,
    state: STATE.IDLE,
  };
  const budgetCommandCar = {
    id: 450,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    x: 80,
    y: 20,
    hp: 80,
    maxHp: 80,
    state: STATE.IDLE,
  };
  budgetSelectionState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 80,
    entities: budgetRiflemen.concat(budgetTanks, budgetExtraTank, budgetCommandCar),
    events: [],
  });
  budgetSelectionState.setSelection(budgetRiflemen.map((entity) => entity.id));
  assert(
    budgetSelectionState.selection.size === BASE_COMMAND_SUPPLY_CAP,
    "selection budget admits 24 one-supply units by command supply",
  );
  assert(
    budgetSelectionState.selectionBudgetOverflow?.cap === BASE_COMMAND_SUPPLY_CAP,
    "selection overflow records budget state for the HUD",
  );
  budgetSelectionState.setSelection(budgetTanks.map((entity) => entity.id));
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "400,401,402",
    "selection budget admits three eight-supply tanks without a Command Car",
  );
  budgetSelectionState.setSelection(budgetTanks.map((entity) => entity.id).concat([budgetExtraTank.id, budgetCommandCar.id]));
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "450,400,401,402,403,404",
    "selection budget offsets Command Car supply before filling normal candidates",
  );
  budgetSelectionState.setSelection(budgetTanks.slice(0, 4).map((entity) => entity.id));
  budgetSelectionState.addToSelection([budgetRiflemen[0].id]);
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "400,401,402",
    "shift-add ignores overflow without replacing the existing selection",
  );
  budgetSelectionState.addToSelection([budgetCommandCar.id, budgetTanks[4].id]);
  assert(
    Array.from(budgetSelectionState.selection).join(",") === "400,401,402,450,404",
    "shift-add can admit a Command Car bonus and then later candidates",
  );
  budgetSelectionState.setControlGroup(0, budgetRiflemen.map((entity) => entity.id));
  assert(
    budgetSelectionState.controlGroups[0].length === BASE_COMMAND_SUPPLY_CAP,
    "control-group save admits 24 one-supply units",
  );
  assert(
    budgetSelectionState.selectionBudgetOverflow?.cap === BASE_COMMAND_SUPPLY_CAP,
    "control-group save records overflow for ignored one-supply units",
  );
  budgetSelectionState.setControlGroup(1, budgetTanks.map((entity) => entity.id));
  assert(
    budgetSelectionState.controlGroups[1].join(",") === "400,401,402",
    "control-group save ignores over-budget Tanks",
  );
  budgetSelectionState.addToControlGroup(1, [budgetRiflemen[0].id]);
  assert(
    budgetSelectionState.controlGroups[1].join(",") === "400,401,402",
    "control-group add ignores overflow without trimming existing legal members",
  );
  budgetSelectionState.addToControlGroup(1, [budgetCommandCar.id, budgetTanks[4].id]);
  assert(
    budgetSelectionState.controlGroups[1].join(",") === "400,401,402,450,404",
    "control-group add can admit one Command Car bonus and then later candidates",
  );
  const secondBudgetCommandCar = {
    id: 451,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    x: 96,
    y: 20,
    hp: 80,
    maxHp: 80,
    state: STATE.IDLE,
  };
  budgetSelectionState.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 80,
    entities: budgetRiflemen.concat(budgetTanks, budgetExtraTank, budgetCommandCar, secondBudgetCommandCar),
    events: [],
  });
  budgetSelectionState.setControlGroup(2, budgetTanks.map((entity) => entity.id).concat([budgetCommandCar.id, secondBudgetCommandCar.id]));
  assert(
    budgetSelectionState.controlGroups[2].join(",") === "450,451,400,401,402,403,404",
    "control-group save stacks multiple Command Car bonuses",
  );
  budgetSelectionState.controlGroups[3] = budgetTanks.map((entity) => entity.id).concat(budgetCommandCar.id);
  const recalledLateCar = budgetSelectionState.selectControlGroup(3);
  assert(
    recalledLateCar.join(",") === "450,400,401,402,403,404",
    "control-group recall pre-admits a Command Car stored late in old runtime order",
  );
  assert(
    budgetSelectionState.controlGroups[3].join(",") === "450,400,401,402,403,404",
    "control-group recall rewrites old over-budget runtime groups to legal admitted order",
  );
  budgetSelectionState.controlGroups[4] = budgetTanks.map((entity) => entity.id);
  const recalledOverBudgetTanks = budgetSelectionState.selectControlGroup(4);
  assert(
    recalledOverBudgetTanks.join(",") === "400,401,402",
    "control-group recall filters old over-budget Tank groups before selection",
  );
  assert(
    budgetSelectionState.selectionBudgetOverflow?.cap === BASE_COMMAND_SUPPLY_CAP,
    "control-group recall records overflow feedback for old over-budget groups",
  );
  const controlGroupCommands = [];
  const controlGroupToasts = [];
  const guardedControlGroupIssuer = {
    issueCommand(command) {
      const budget = commandWithinBudget(budgetSelectionState, command);
      if (!budget.ok) {
        controlGroupToasts.push(budget);
        return { sent: false, blocked: "commandBudget", budget };
      }
      controlGroupCommands.push(command);
      return { sent: true };
    },
  };
  guardedControlGroupIssuer.issueCommand(cmd.move(Array.from(budgetSelectionState.selection), 10, 20));
  guardedControlGroupIssuer.issueCommand(cmd.move(budgetTanks.map((entity) => entity.id), 10, 20));
  assert(controlGroupCommands.length === 1, "legal recalled control-group command is sent through the budget guard");
  assert(controlGroupToasts.length === 1, "over-budget command restored from stale group data is blocked before send");

  // Command-card submenu is local-only and is closed by mode-changing intent actions.
  const commandCardIntent = new ClientIntent();
  commandCardIntent.openWorkerBuildMenu();
  assert(commandCardIntent.commandCardMode === "workerBuild", "worker build submenu opens");
  assert(commandCardIntent.closeCommandCardMenu() === true, "closeCommandCardMenu reports an open submenu");
  assert(commandCardIntent.closeCommandCardMenu() === false, "closeCommandCardMenu reports when no submenu was open");
  commandCardIntent.openWorkerBuildMenu();
  commandCardIntent.beginCommandTarget("attack");
  assert(commandCardIntent.commandCardMode === null, "command targeting closes the worker build submenu");
  assert(commandCardIntent.commandTarget === "attack", "command targeting mirrors the composer target");
  const queuedIssue = commandCardIntent.issueCommandTarget({ shiftKey: true });
  assert(queuedIssue.keepArmed && commandCardIntent.commandTarget === "attack", "Shift-issued command remains armed");
  commandCardIntent.releaseCommandTargetShift();
  assert(commandCardIntent.commandTarget === null, "Shift release clears a Shift-preserved command target");
  commandCardIntent.openWorkerBuildMenu();
  commandCardIntent.beginPlacement(KIND.DEPOT);
  assert(commandCardIntent.commandCardMode === null, "build placement closes the worker build submenu");

  // Control groups are local-only, own controllable entities only, and budgeted like selection.
  const cgState = new GameState({ ...start, map: { ...start.map, resources: [] } });
  const ownControllables = Array.from({ length: 14 }, (_, i) => ({
    id: 100 + i,
    owner: 1,
    kind: i === 12 ? KIND.BARRACKS : KIND.WORKER,
    x: i * 10,
    y: 0,
    hp: 40,
    maxHp: 40,
    state: "idle",
  }));
  cgState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 20,
    entities: [
      ...ownControllables,
      { id: 160, owner: 2, kind: KIND.WORKER, x: 0, y: 20, hp: 40, maxHp: 40, state: "idle" },
      { id: 161, owner: 0, kind: KIND.STEEL, x: 0, y: 40, remaining: 100 },
    ],
    events: [],
  });
  assert(Array.isArray(cgState.controlGroups) && cgState.controlGroups.length === 10, "GameState has ten control groups");
  assertHasMethod(cgState, "setControlGroup", "GameState");
  assertHasMethod(cgState, "addToControlGroup", "GameState");
  assertHasMethod(cgState, "selectControlGroup", "GameState");
  assertHasMethod(cgState, "controlGroupEntities", "GameState");
  cgState.setControlGroup(0, [100, 160, 101, 161, 112, 113, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111]);
  assert(
    cgState.controlGroups[0].join(",") === "100,101,112,113,102,103,104,105,106,107,108,109,110,111",
    "control groups store own units/buildings only in selection order within budget",
  );
  cgState.addToControlGroup(0, [110, 111, 112, 113]);
  assert(cgState.controlGroups[0].length === 14, "adding duplicates to a budgeted control group is stable");
  cgState.setControlGroup(1, [100, 101]);
  cgState.addToControlGroup(1, [101, 102, 103]);
  assert(cgState.controlGroups[1].join(",") === "100,101,102,103", "adding to a control group dedupes existing ids");
  cgState.selectControlGroup(1);
  assert(Array.from(cgState.selection).join(",") === "100,101,102,103", "selectControlGroup recalls live group ids");
  cgState.applySnapshot({
    tick: 1,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 20,
    entities: ownControllables.filter((e) => e.id !== 101),
    events: [{ e: "death", id: 101, x: 10, y: 0, kind: KIND.WORKER }],
  });
  assert(cgState.controlGroups[1].join(",") === "100,102,103", "dead entities disappear from control groups");

  const teamSelectionState = new GameState({
    ...start,
    map: { ...start.map, width: 12, height: 12, resources: [] },
    players: [
      { id: 1, teamId: 1, name: "A", color: "#ff0000", startTileX: 1, startTileY: 1 },
      { id: 2, teamId: 1, name: "B", color: "#00ff00", startTileX: 2, startTileY: 2 },
      { id: 3, teamId: 2, name: "C", color: "#0000ff", startTileX: 3, startTileY: 3 },
    ],
  });
  const ownWorker = { id: 201, owner: 1, kind: KIND.WORKER, x: 32, y: 32, hp: 40, maxHp: 40, state: STATE.IDLE };
  const allyWorker = { id: 202, owner: 2, kind: KIND.WORKER, x: 64, y: 32, hp: 40, maxHp: 40, state: STATE.IDLE };
  const enemyWorker = { id: 203, owner: 3, kind: KIND.WORKER, x: 96, y: 32, hp: 40, maxHp: 40, state: STATE.IDLE };
  teamSelectionState.applySnapshot({
    tick: 0,
    steel: 100,
    oil: 100,
    supplyUsed: 1,
    supplyCap: 10,
    entities: [ownWorker, allyWorker, enemyWorker],
    events: [],
  });
  const selectionInput = Object.create(Input.prototype);
  selectionInput.state = teamSelectionState;
  selectionInput.dom = { clientWidth: 400, clientHeight: 300 };
  publishSelectionTestScene(selectionInput);
  selectionInput._commitClickSelection({ x: allyWorker.x, y: allyWorker.y }, false, false);
  assert(
    Array.from(teamSelectionState.selection).join(",") === String(allyWorker.id),
    "single-click can select an allied entity for inspection",
  );
  selectionInput._commitBoxSelection({ x0: 0, y0: 0, x1: 120, y1: 64 }, false);
  assert(
    Array.from(teamSelectionState.selection).join(",") === String(ownWorker.id),
    "box selection skips allied and enemy units",
  );
  selectionInput._commitClickSelection({ x: allyWorker.x, y: allyWorker.y }, true, false);
  assert(
    Array.from(teamSelectionState.selection).join(",") === `${ownWorker.id},${allyWorker.id}`,
    "shift-click can add an allied inspection target to the current selection",
  );
  teamSelectionState.setControlGroup(2, teamSelectionState.selection);
  assert(
    teamSelectionState.controlGroups[2].join(",") === String(ownWorker.id),
    "mixed own/allied selections save only own entities into control groups",
  );
  const allyOnlyCard = buildCommandCardDescriptors(commandCardCtx({
    playerId: 1,
    selection: [allyWorker],
    entities: [ownWorker, allyWorker],
  }));
  assert(commandButtons(allyOnlyCard).length === 0, "allied-only inspection selection exposes no command buttons");
  const mixedCard = buildCommandCardDescriptors(commandCardCtx({
    playerId: 1,
    selection: [ownWorker, allyWorker],
    entities: [ownWorker, allyWorker],
  }));
  const holdIntent = buttonByAction(mixedCard, "holdPosition")?.intent;
  assert(
    holdIntent?.unitIds?.join(",") === String(ownWorker.id),
    "mixed own/allied command card emits commands only for own entity ids",
  );

  const budgetInputState = new GameState({
    ...start,
    map: { ...start.map, width: 12, height: 12, resources: [] },
  });
  const boxRiflemen = Array.from({ length: 30 }, (_, index) => ({
    id: 5100 + index,
    owner: 1,
    kind: KIND.RIFLEMAN,
    x: 8 + index * 4,
    y: 140,
    hp: 45,
    maxHp: 45,
    state: STATE.IDLE,
  }));
  const doubleClickRiflemen = Array.from({ length: 30 }, (_, index) => ({
    id: 5200 + index,
    owner: 1,
    kind: KIND.MACHINE_GUNNER,
    x: 8 + index * 4,
    y: 180,
    hp: 55,
    maxHp: 55,
    state: STATE.IDLE,
  }));
  const budgetInputTanks = Array.from({ length: 5 }, (_, index) => ({
    id: 5300 + index,
    owner: 1,
    kind: KIND.TANK,
    x: 8 + index * 16,
    y: 220,
    hp: 292,
    maxHp: 292,
    state: STATE.IDLE,
  }));
  const lateBoxCommandCar = {
    id: 5400,
    owner: 1,
    kind: KIND.COMMAND_CAR,
    x: 96,
    y: 220,
    hp: 225,
    maxHp: 225,
    state: STATE.IDLE,
  };
  budgetInputState.applySnapshot({
    tick: 0,
    steel: 0,
    oil: 0,
    supplyUsed: 0,
    supplyCap: 80,
    entities: boxRiflemen.concat(doubleClickRiflemen, budgetInputTanks, lateBoxCommandCar),
    events: [],
  });
  const budgetSelectionInput = Object.create(Input.prototype);
  budgetSelectionInput.state = budgetInputState;
  budgetSelectionInput.dom = selectionInput.dom;
  publishSelectionTestScene(budgetSelectionInput);
  budgetSelectionInput._commitBoxSelection({ x0: 0, y0: 124, x1: 140, y1: 156 }, false);
  assert(
    Array.from(budgetInputState.selection).length === BASE_COMMAND_SUPPLY_CAP,
    "drag selection admits the base budget of one-supply units",
  );
  budgetSelectionInput._commitClickSelection({ x: doubleClickRiflemen[0].x, y: doubleClickRiflemen[0].y }, false, true);
  assert(
    Array.from(budgetInputState.selection).length === BASE_COMMAND_SUPPLY_CAP / STATS[KIND.MACHINE_GUNNER].supply &&
      Array.from(budgetInputState.selection).every((id) => id >= 5200 && id < 5300),
    "double-click same-kind selection is filtered by command supply",
  );
  budgetSelectionInput._commitBoxSelection({ x0: 0, y0: 204, x1: 120, y1: 236 }, false);
  assert(
    Array.from(budgetInputState.selection).join(",") === "5400,5300,5301,5302,5303,5304",
    "drag selection pre-admits a late Command Car before budget-filling Tanks",
  );
  const alliedRightClickCommands = [];
  const rightClickInput = Object.create(Input.prototype);
  rightClickInput.state = teamSelectionState;
  rightClickInput.dom = selectionInput.dom;
  publishSelectionTestScene(rightClickInput);
  rightClickInput._selectedOwnUnitIds = Input.prototype._selectedOwnUnitIds;
  rightClickInput._selectedGathererIds = Input.prototype._selectedGathererIds;
  rightClickInput._selectedWorkerIds = Input.prototype._selectedWorkerIds;
  rightClickInput._selectedProducerBuildingIds = Input.prototype._selectedProducerBuildingIds;
  rightClickInput._issueCommand = (command) => alliedRightClickCommands.push(command);
  teamSelectionState.setSelection([ownWorker.id]);
  rightClickInput._onRightClick({ x: allyWorker.x, y: allyWorker.y });
  assert(
    alliedRightClickCommands.length === 1 &&
      alliedRightClickCommands[0].c === "move" &&
      alliedRightClickCommands[0].units.join(",") === String(ownWorker.id),
    "right-clicking an allied entity with own units selected sends move, not attack",
  );
  const minimapLike = Object.create(Minimap.prototype);
  minimapLike.state = teamSelectionState;
  assert(
    minimapLike._blipColor(allyWorker) === `#${COLORS.selectAlly.toString(16).padStart(6, "0")}`,
    "minimap blip color distinguishes allies from enemies",
  );
  // Placement is local-only
  const placementIntent = new ClientIntent();
  placementIntent.beginPlacement("barracks");
  assert(placementIntent.placement !== null, "placement started");
  placementIntent.updatePlacement(2, 3, true);
  assert(placementIntent.placement.tileX === 2, "updatePlacement sets tileX");
  assert(placementIntent.placement.tileY === 3, "updatePlacement sets tileY");
  assert(placementIntent.placement.valid === true, "updatePlacement sets valid");
  placementIntent.endPlacement();
  assert(placementIntent.placement === null, "endPlacement clears placement");

  const map = { width: 6, height: 6, tileSize: 32, terrain: new Array(36).fill(0) };
  const worker = { id: 7, owner: 1, kind: "worker", x: 80, y: 80 };
  const other = { id: 8, owner: 1, kind: "worker", x: 80, y: 80 };
  assert(
    footprintValidAgainstEntities([worker], new Set([7]), 1, 1, 2, 2, map) === true,
    "client_preview_allows_chosen_worker_body_inside_footprint",
  );
  assert(
    footprintValidAgainstEntities([other], new Set([7]), 1, 1, 2, 2, map) === false,
    "client_preview_rejects_other_unit_body_inside_footprint",
  );
  assert(
    movementBodyClass(KIND.WORKER) === "infantryLike" &&
      movementBodyClass(KIND.RIFLEMAN) === "infantryLike" &&
      movementBodyClass(KIND.MORTAR_TEAM) === "vehicleBody" &&
      movementBodyClass(KIND.ANTI_TANK_GUN) === "vehicleBody" &&
      movementBodyClass(KIND.ARTILLERY) === "vehicleBody" &&
      movementBodyClass(KIND.SCOUT_CAR) === "vehicleBody" &&
      movementBodyClass(KIND.TANK) === "vehicleBody" &&
      movementBodyClass(KIND.COMMAND_CAR) === "vehicleBody" &&
      movementBodyClass(KIND.SCOUT_PLANE) === "infantryLike",
    "client placement movement-body classes mirror server ground blockers",
  );
  assert(
    placementPolicyForBuilding(KIND.TANK_TRAP).unitOverlap === "infantryAllowed" &&
      placementPolicyForBuilding(KIND.DEPOT).unitOverlap === "none" &&
      placementPolicyForBuilding(KIND.PUMP_JACK).resourceOverlap === "oilCenterRequired",
    "special placement policies mirror server Tank Trap and Pump Jack rules",
  );
  assert(
    footprintValidAgainstEntities(
      [other],
      new Set(),
      1,
      1,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === true,
    "Tank Trap advisory preview allows infantry bodies inside the footprint",
  );
  const tank = { id: 9, owner: 1, kind: KIND.TANK, x: 116, y: 64 };
  const scoutPlane = { id: 90, owner: 1, kind: KIND.SCOUT_PLANE, x: 80, y: 80 };
  assert(!footprintValidAgainstEntities([tank], new Set(), 1, 1, 2, 2, map), "client preview rejects tank bodies");
  assert(footprintValidAgainstEntities([scoutPlane], new Set(), 1, 1, 2, 2, map), "client preview ignores Scout Plane render bodies");
  const trapTank = { id: 91, owner: 1, kind: KIND.TANK, x: 58, y: 48 };
  assert(
    footprintValidAgainstEntities(
      [trapTank],
      new Set(),
      1,
      1,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === false,
    "Tank Trap advisory preview still rejects vehicle-body overlap",
  );
  const pumpJackPolicy = placementPolicyForBuilding(KIND.PUMP_JACK);
  const liveOil = { id: 94, owner: 0, kind: KIND.OIL, x: 48, y: 48, remaining: 1000 };
  const depletedOil = { id: 95, owner: 0, kind: KIND.OIL, x: 48, y: 48, remaining: 0 };
  const steelPatch = { id: 96, owner: 0, kind: KIND.STEEL, x: 48, y: 48, remaining: 1000 };
  assert(
    footprintValidAgainstEntities([liveOil], new Set(), 1, 1, 1, 1, map, pumpJackPolicy) === true,
    "Pump Jack placement preview allows overlap with a live oil center",
  );
  assert(
    footprintPlacementBlocker([], new Set(), 1, 1, 1, 1, map, pumpJackPolicy) === "terrain",
    "Pump Jack placement preview requires an oil center inside the footprint",
  );
  assert(
    footprintPlacementBlocker([depletedOil], new Set(), 1, 1, 1, 1, map, pumpJackPolicy) === "structure",
    "Pump Jack placement preview rejects depleted oil nodes as ordinary resource blockers",
  );
  assert(
    footprintPlacementBlocker([steelPatch], new Set(), 1, 1, 1, 1, map, pumpJackPolicy) === "structure",
    "Pump Jack placement preview does not treat steel patches as valid oil sites",
  );
  assert(STATS[KIND.TANK].body.length === 50.4, "tank client body length mirrors server");
  assert(STATS[KIND.TANK].body.width === 28.8, "tank client body width mirrors server");
  assert(STATS[KIND.ANTI_TANK_GUN].body.length === 42.0, "anti-tank gun client body length mirrors server");
  assert(STATS[KIND.ANTI_TANK_GUN].body.width === 24.0, "anti-tank gun client body width mirrors server");
  assert(STATS[KIND.ARTILLERY].size === STATS[KIND.TANK].size, "Artillery selection size should match tank size");
  assert(
    STATS[KIND.ARTILLERY].body.length === STATS[KIND.TANK].body.length &&
      STATS[KIND.ARTILLERY].body.width === STATS[KIND.TANK].body.width,
    "Artillery client body should match tank footprint",
  );

  const input = Object.create(Input.prototype);
  input.state = {
    entitiesInterpolated: () => [worker, other],
  };
  input._selectionEntities = () => input.state.entitiesInterpolated();
  input._selectedWorkerIds = () => [7, 8];
  assert(
    input._footprintValid(1, 1, 2, 2, map) === false,
    "preview should not ignore every selected worker",
  );
  input.state.entitiesInterpolated = () => [worker];
  assert(
    input._footprintValid(1, 1, 2, 2, map) === true,
    "preview should ignore one selected worker body as an advisory build-placement allowance",
  );
  input.state.entitiesInterpolated = () => [other];
  assert(
    input._footprintValid(1, 1, 1, 1, map, KIND.TANK_TRAP) === true,
    "Tank Trap input preview allows non-builder infantry overlap",
  );
  input.state.entitiesInterpolated = () => [trapTank];
  assert(
    input._footprintValid(1, 1, 1, 1, map, KIND.TANK_TRAP) === false,
    "Tank Trap input preview rejects vehicle-body units",
  );
  const rockMap = { ...map, terrain: [...map.terrain] };
  rockMap.terrain[2 * rockMap.width + 2] = TERRAIN.ROCK;
  assert(
    footprintPlacementBlocker([], new Set(), 2, 2, 1, 1, rockMap, placementPolicyForBuilding(KIND.TANK_TRAP)) === "terrain",
    "Tank Trap placement blocker classifies impassable terrain",
  );
  assert(
    footprintPlacementBlocker(
      [{ id: 92, owner: 1, kind: KIND.BARRACKS, x: 80, y: 80 }],
      new Set(),
      2,
      2,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === "structure",
    "Tank Trap placement blocker classifies buildings separately from units",
  );
  assert(
    footprintPlacementBlocker(
      [trapTank],
      new Set(),
      1,
      1,
      1,
      1,
      map,
      placementPolicyForBuilding(KIND.TANK_TRAP),
    ) === "unit",
    "Tank Trap placement blocker classifies vehicle-body unit blockers",
  );
  delete input._selectedWorkerIds;

  const pairs = (tiles) => tiles.map((site) => [site.tileX, site.tileY]);
  const exactTankTrapSpacing = (tiles) => tiles.every((site, index) => {
    if (index === 0) return true;
    const prev = tiles[index - 1];
    const dx = Math.abs(site.tileX - prev.tileX);
    const dy = Math.abs(site.tileY - prev.tileY);
    return (dx === 1 && dy === 1) || (dx === 2 && dy === 0) || (dx === 0 && dy === 2);
  });
  assertDeepEqual(
    pairs(tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 5, tileY: 0 })),
    [[0, 0], [2, 0], [4, 0]],
    "Tank Trap horizontal line uses one-tile orthogonal gaps and omits off-cadence end",
  );
  assertDeepEqual(
    pairs(tankTrapLineTiles({ tileX: 1, tileY: 1 }, { tileX: 1, tileY: 5 })),
    [[1, 1], [1, 3], [1, 5]],
    "Tank Trap vertical line includes the end when it lands on cadence",
  );
  assertDeepEqual(
    pairs(tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 3, tileY: 3 })),
    [[0, 0], [1, 1], [2, 2], [3, 3]],
    "Tank Trap diagonal line keeps corner-touching sites",
  );
  const shallowLine = tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 5, tileY: 2 });
  const steepLine = tankTrapLineTiles({ tileX: 0, tileY: 0 }, { tileX: 2, tileY: 5 });
  assert(exactTankTrapSpacing(shallowLine), "Tank Trap shallow line uses only allowed vehicle-blocking spacing");
  assert(exactTankTrapSpacing(steepLine), "Tank Trap steep line uses only allowed vehicle-blocking spacing");
  assertDeepEqual(
    pairs(shallowLine),
    [[0, 0], [1, 1], [3, 1], [4, 2]],
    "Tank Trap shallow bridge sites avoid knight and two-by-two gaps",
  );
  const lineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 4, tileY: 0 },
    isValid: (tileX) => tileX !== 2,
  });
  assertDeepEqual(
    lineSites.map((site) => [site.tileX, site.tileY, site.valid]),
    [[0, 0, true], [2, 0, false], [4, 0, false]],
    "Tank Trap line preview marks sites invalid when skipping would create an oversized gap",
  );
  assertDeepEqual(
    pairs(validTankTrapLineSites(lineSites)),
    [[0, 0]],
    "Tank Trap dispatch preserves required spacing after invalid preview sites",
  );
  const terrainSkippedLineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 4, tileY: 0 },
    isValid: (tileX) => ({ valid: tileX !== 2, blockedBy: tileX === 2 ? "terrain" : null }),
  });
  assertDeepEqual(
    terrainSkippedLineSites.map((site) => [site.tileX, site.tileY, site.valid, site.skipped]),
    [[0, 0, true, false], [2, 0, false, true], [4, 0, true, false]],
    "Tank Trap line preview skips impassable terrain and resumes on the other side",
  );
  assertDeepEqual(
    pairs(validTankTrapLineSites(terrainSkippedLineSites)),
    [[0, 0], [4, 0]],
    "Tank Trap dispatch omits skipped terrain sites without stopping the line",
  );
  const structureSkippedLineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 6, tileY: 0 },
    isValid: (tileX) => ({ valid: tileX !== 2, blockedBy: tileX === 2 ? "structure" : null }),
  });
  assertDeepEqual(
    pairs(validTankTrapLineSites(structureSkippedLineSites)),
    [[0, 0], [4, 0], [6, 0]],
    "Tank Trap dispatch resumes normal spacing after skipping a building",
  );
  const unitBlockedLineSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 4, tileY: 0 },
    isValid: (tileX) => ({ valid: tileX !== 2, blockedBy: tileX === 2 ? "unit" : null }),
  });
  assertDeepEqual(
    unitBlockedLineSites.map((site) => [site.tileX, site.tileY, site.valid, site.skipped]),
    [[0, 0, true, false], [2, 0, false, false], [4, 0, false, false]],
    "Tank Trap line preview keeps unit-blocked gaps as line-stopping invalid sites",
  );
  const diagonalGapSites = buildTankTrapLineSites({
    start: { tileX: 0, tileY: 0 },
    end: { tileX: 2, tileY: 2 },
    isValid: (tileX, tileY) => !(tileX === 1 && tileY === 1),
  });
  assertDeepEqual(
    diagonalGapSites.map((site) => [site.tileX, site.tileY, site.valid]),
    [[0, 0, true], [1, 1, false], [2, 2, false]],
    "Tank Trap line preview forbids dispatching across a two-by-two diagonal gap",
  );
  const lineCommands = tankTrapBuildCommands([77, 88], [
    { tileX: 0, tileY: 0, valid: true },
    { tileX: 2, tileY: 0, valid: true },
    { tileX: 4, tileY: 0, valid: true },
    { tileX: 6, tileY: 0, valid: true },
  ]);
  assertDeepEqual(
    lineCommands.map((command) => [command.units, command.tileX, command.tileY, command.queued === true]),
    [
      [[77], 0, 0, false],
      [[88], 2, 0, false],
      [[77, 88], 4, 0, true],
      [[77, 88], 6, 0, true],
    ],
    "Tank Trap line dispatch assigns immediate single-worker builds then queued overflow builds",
  );
  const gapCommands = tankTrapBuildCommands([77, 88], [
    { tileX: 0, tileY: 0, valid: true },
    { tileX: 2, tileY: 0, valid: false },
    { tileX: 4, tileY: 0, valid: true },
  ]);
  assertDeepEqual(
    gapCommands.map((command) => [command.units, command.tileX, command.tileY, command.queued === true]),
    [[[77], 0, 0, false]],
    "Tank Trap line dispatch stops before valid-looking sites that would skip over a gap",
  );
  const lineDragInput = Object.create(Input.prototype);
  let dragRefreshes = 0;
  let dragConfirms = 0;
  lineDragInput.clientIntent = new ClientIntent();
  lineDragInput.clientIntent.beginPlacement(KIND.TANK_TRAP);
  lineDragInput.clientIntent.updatePlacement(3, 4, true, {
    lineSites: [{ tileX: 3, tileY: 4, valid: true }],
  });
  lineDragInput._refreshPlacement = () => {
    dragRefreshes += 1;
  };
  lineDragInput._confirmPlacement = () => {
    dragConfirms += 1;
  };
  lineDragInput._onLeftDown({ x: 10, y: 10 }, {});
  assert(
    lineDragInput._placementDrag?.tileX === 3 &&
      lineDragInput._placementDrag?.tileY === 4 &&
      lineDragInput._drag === undefined &&
      dragRefreshes === 1 &&
      dragConfirms === 0,
    "Tank Trap left-down starts placement drag instead of selection drag or immediate confirm",
  );

  const clickableTank = { id: 10, owner: 1, kind: KIND.TANK, x: 0, y: 0, facing: 0 };
  assert(
    pointHitsOrientedVehicle(clickableTank, 25.2, 0, 3) === true,
    "tank hit testing should reach the long hull axis",
  );
  assert(
    pointHitsOrientedVehicle(clickableTank, 0, 20, 3) === false,
    "tank hit testing should not use a stale circular side radius",
  );
  const clickableAntiTankGun = { id: 11, owner: 1, kind: KIND.ANTI_TANK_GUN, x: 0, y: 0, facing: 0 };
  assert(
    pointHitsOrientedVehicle(clickableAntiTankGun, 22, 0, 0) === true,
    "anti-tank gun hit testing should reach the wheeled body axis",
  );
  assert(
    pointHitsOrientedVehicle(clickableAntiTankGun, 0, 18, 0) === false,
    "anti-tank gun hit testing should not use the old circular radius",
  );

  const overlappingWorker = { id: 30, owner: 1, kind: KIND.WORKER, x: 100, y: 100 };
  const overlappingSteel = { id: 31, owner: 0, kind: KIND.STEEL, x: 104, y: 100, remaining: 1000 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [overlappingWorker, overlappingSteel],
    selectedEntities: () => [overlappingWorker],
    addCommandFeedback() {},
  };
  const rightClickCommands = [];
  input.commandIssuer = { issueCommand(command) { rightClickCommands.push(command); } };
  input._groundAtScreen = (x, y) => ({ x, y });
  publishSelectionTestScene(input);
  input._onRightClick({ x: 100, y: 100 });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "gather" &&
      rightClickCommands[0].node === overlappingSteel.id,
    "worker right-click should prioritize an overlapped resource patch over the worker body",
  );

  const overlappingOil = { id: 32, owner: 0, kind: KIND.OIL, x: 112, y: 112, remaining: 1000 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [overlappingWorker, overlappingOil],
    selectedEntities: () => [overlappingWorker],
    addCommandFeedback() {},
  };
  publishSelectionTestScene(input);
  rightClickCommands.length = 0;
  input._onRightClick({ x: 112, y: 112 }, { shiftKey: true });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "build" &&
      rightClickCommands[0].building === KIND.PUMP_JACK &&
      rightClickCommands[0].tileX === 3 &&
      rightClickCommands[0].tileY === 3 &&
      rightClickCommands[0].queued === true,
    "worker shift-right-click on oil should queue a Pump Jack build instead of direct gather",
  );

  const rallyCityCentre = {
    id: 33,
    owner: 1,
    kind: KIND.CITY_CENTRE,
    x: 64,
    y: 64,
    buildProgress: null,
  };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [rallyCityCentre, overlappingSteel, overlappingOil],
    selectedEntities: () => [rallyCityCentre],
    addCommandFeedback() {},
  };
  publishSelectionTestScene(input);
  rightClickCommands.length = 0;
  input._onRightClick({ x: overlappingSteel.x, y: overlappingSteel.y });
  assert(
    rightClickCommands.length === 0,
    "production-building right-click on steel should not issue a rally command",
  );
  rightClickCommands.length = 0;
  input._onRightClick({ x: overlappingOil.x, y: overlappingOil.y });
  assert(
    rightClickCommands.length === 0,
    "production-building right-click on oil should not issue a rally command",
  );

  const moveUnit = { id: 40, owner: 1, kind: KIND.RIFLEMAN, x: 120, y: 120 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [moveUnit],
    selectedEntities: () => [moveUnit],
    addCommandFeedback() {},
  };
  publishSelectionTestScene(input);
  rightClickCommands.length = 0;
  input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "move" &&
      rightClickCommands[0].queued === true,
    "Shift terrain right-click should send queued move",
  );

  const enemyUnit = { id: 41, owner: 2, kind: KIND.RIFLEMAN, x: 180, y: 180 };
  input.state.entitiesInterpolated = () => [moveUnit, enemyUnit];
  publishSelectionTestScene(input);
  rightClickCommands.length = 0;
  input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "attack" &&
      rightClickCommands[0].queued === true,
    "Shift right-click on enemies should send queued attack",
  );
  const deconstructWorker = { id: 42, owner: 1, kind: KIND.WORKER, x: 150, y: 150 };
  const enemyTankTrap = { id: 43, owner: 2, kind: KIND.TANK_TRAP, x: 180, y: 180 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    entitiesInterpolated: () => [deconstructWorker, enemyTankTrap],
    selectedEntities: () => [deconstructWorker],
    addCommandFeedback() {},
  };
  publishSelectionTestScene(input);
  rightClickCommands.length = 0;
  input._onRightClick({ x: 180, y: 180 }, { shiftKey: true });
  assert(
    rightClickCommands.length === 1 &&
      rightClickCommands[0].c === "deconstruct" &&
      rightClickCommands[0].units.join(",") === "42" &&
      rightClickCommands[0].target === enemyTankTrap.id &&
      rightClickCommands[0].queued === true,
    "Shift right-click on a Tank Trap with workers selected should send queued deconstruct",
  );
  input.dom = { clientWidth: 800, clientHeight: 600 };
  input.camera = { screenToWorld: (x, y) => ({ x, y }) };
  const deployedAntiTankGun = {
    id: 21,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
  };
  const otherDeployedAntiTankGun = {
    id: 22,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 120,
    y: 100,
    setupState: SETUP.DEPLOYED,
  };
  const packedAntiTankGun = {
    id: 23,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 110,
    y: 100,
    setupState: SETUP.PACKED,
  };
  input.state = {
    playerId: 1,
    entitiesInterpolated: () => [deployedAntiTankGun, otherDeployedAntiTankGun, packedAntiTankGun],
  };
  publishSelectionTestScene(input);
  assert(
    input
      ._closestOwnUnitKindInViewport(
        KIND.ANTI_TANK_GUN,
        deployedAntiTankGun.x,
        deployedAntiTankGun.y,
        deployedAntiTankGun,
      )
      .join(",") === "21,22",
    "selecting set-up anti-tank guns should not include packed anti-tank guns",
  );
  assert(
    input
      ._closestOwnUnitKindInViewport(KIND.ANTI_TANK_GUN, packedAntiTankGun.x, packedAntiTankGun.y, packedAntiTankGun)
      .join(",") === "23",
    "selecting packed anti-tank guns should not include set-up anti-tank guns",
  );
  assert(
    input._closestOwnUnitKindInViewport(KIND.ANTI_TANK_GUN, deployedAntiTankGun.x, deployedAntiTankGun.y).join(",") ===
      "21,23,22",
    "kind-only Anti-Tank Gun selection helper calls should keep legacy all Anti-Tank Gun behavior",
  );

  assert(input._controlGroupSlotFromKey({ code: "Digit1" }) === 0, "Digit1 maps to control group slot 0");
  assert(input._controlGroupSlotFromKey({ code: "Digit0" }) === 9, "Digit0 maps to control group slot 9");
  assert(input._controlGroupSlotFromKey({ code: "Numpad5" }) === 4, "Numpad5 maps to control group slot 4");
  assert(input._controlGroupSlotFromKey({ code: "KeyQ" }) === null, "non-number keys do not map to control groups");

  const hotkeyCalls = [];
  const hotkeyInput = Object.create(Input.prototype);
  hotkeyInput.state = {
    spectator: false,
    selection: new Set([1, 2]),
    controlGroups: Array.from({ length: 10 }, () => []),
    setControlGroup(slot, ids) {
      const values = Array.from(ids);
      this.controlGroups[slot] = values;
      hotkeyCalls.push({ type: "set", slot, ids: values });
      return values;
    },
    addToControlGroup(slot, ids) {
      hotkeyCalls.push({ type: "add", slot, ids: Array.from(ids) });
      return Array.from(ids);
    },
    setSelection(ids) {
      this.selection = new Set(ids);
      hotkeyCalls.push({ type: "select" });
    },
  };
  hotkeyInput._visibleSelectionIds = (ids) => Array.from(ids);
  hotkeyInput._lastControlGroupTap = null;
  hotkeyInput._jumpToControlGroupCluster = (slot) => hotkeyCalls.push({ type: "jump", slot });
  const keyEvent = (code, mods = {}) => ({
    code,
    altKey: !!mods.altKey,
    ctrlKey: !!mods.ctrlKey,
    metaKey: !!mods.metaKey,
    shiftKey: !!mods.shiftKey,
    repeat: !!mods.repeat,
    preventDefault() { this.prevented = true; },
    stopPropagation() { this.stopped = true; },
  });
  const saveEvent = keyEvent("Digit2", { altKey: true });
  assert(hotkeyInput._handleControlGroupHotkey(saveEvent) === true, "Alt+number saves a control group");
  assert(saveEvent.prevented && saveEvent.stopped, "handled control-group hotkeys prevent browser handling");
  const addEvent = keyEvent("Digit2", { shiftKey: true });
  assert(hotkeyInput._handleControlGroupHotkey(addEvent) === true, "Shift+number adds to a control group");
  hotkeyInput._handleControlGroupHotkey(keyEvent("Digit2"));
  hotkeyInput._handleControlGroupHotkey(keyEvent("Digit2"));
  assert(
    hotkeyCalls.map((c) => c.type).join(",") === "set,add,select,select,jump",
    "plain number recalls, and double-tap recalls then jumps",
  );

  const repeatHotkeyInput = Object.create(Input.prototype);
  repeatHotkeyInput.keys = {};
  repeatHotkeyInput.pointerLocked = false;
  repeatHotkeyInput._handleControlGroupHotkey = () => false;
  let repeatClicks = 0;
  let repeatable = true;
  globalThis.document = {
    getElementById(id) {
      assert(id === "command-card", "repeated command hotkeys should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "repeated command hotkeys should query hotkey buttons");
          return [{
            dataset: { hotkey: "W", repeatable: repeatable ? "true" : "false" },
            disabled: false,
            click() {
              repeatClicks += 1;
            },
          }];
        },
      };
    },
  };
  repeatHotkeyInput._handleKeyDown(keyEvent("KeyW", { repeat: true }));
  repeatable = false;
  repeatHotkeyInput._handleKeyDown(keyEvent("KeyW", { repeat: true }));
  assert(repeatClicks === 1, "only repeatable command-card buttons respond to native key repeat");

  const menuCancelInput = Object.create(Input.prototype);
  let menuClosed = 0;
  let selectionCleared = 0;
  menuCancelInput.state = {
    clearSelection() {
      selectionCleared += 1;
    },
  };
  menuCancelInput.clientIntent = new ClientIntent();
  menuCancelInput.clientIntent.closeCommandCardMenu = () => {
    menuClosed += 1;
    return true;
  };
  menuCancelInput._cancel();
  assert(menuClosed === 1, "Esc closes the worker build submenu first");
  assert(selectionCleared === 0, "Esc returning to worker commands does not clear selection");

  const ownBuilding = {
    id: 31,
    owner: 1,
    kind: KIND.BARRACKS,
    x: 200,
    y: 200,
  };
  const targetedInput = Object.create(Input.prototype);
  const sentCommands = [];
  const selectionClicks = [];
  const feedback = [];
  const targetedIntent = new ClientIntent();
  targetedIntent.addCommandFeedback = (kind, x, y) => {
    feedback.push({ kind, x, y });
  };
  targetedIntent.beginCommandTarget("attack");
  targetedInput.state = {
    playerId: 1,
  };
  targetedInput.clientIntent = targetedIntent;
  targetedInput.screenOverlay = { setMarquee() {}, clearMarquee() {} };
  targetedInput.commandIssuer = { issueCommand: (command) => sentCommands.push(command) };
  targetedInput._groundAtScreen = (x, y) => ({ x, y });
  targetedInput._entityAtScreen = () => ownBuilding;
  targetedInput._selectedOwnUnitIds = () => [7];
  targetedInput._commitClickSelection = (p) => selectionClicks.push(p);
  targetedInput._screenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  targetedInput._trackMouse = () => {};
  targetedInput._onLeftDown({ x: 200, y: 200 }, {});
  assert(targetedInput.clientIntent.commandTarget === null, "attack targeting clears after one click");
  assert(sentCommands.length === 1, "own click while attack targeting should issue one command");
  assert(sentCommands[0].c === "attack", "own click while attack targeting should issue attack");
  assert(sentCommands[0].units.join(",") === "7", "self-attack should use selected own units");
  assert(sentCommands[0].target === ownBuilding.id, "self-attack should target the clicked own entity");
  assert(feedback.length === 1 && feedback[0].kind === "attack", "self-attack click should show attack feedback");
  assert(targetedInput._drag == null, "attack targeting should not fall through to selection on the same click");
  targetedInput._handleMouseUp({
    button: 0,
    clientX: 200,
    clientY: 200,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(selectionClicks.length === 0, "attack targeting click should not also select on mouse-up");

  targetedInput.clientIntent.endCommandTarget();
  targetedInput._drag = null;
  targetedInput._entityAtScreen = () => null;
  targetedInput._onLeftDown({ x: 240, y: 240 }, {});
  targetedInput._handleMouseUp({
    button: 0,
    clientX: 240,
    clientY: 240,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(sentCommands.length === 1, "a second click without another A press should not issue attack-move");
  assert(selectionClicks.length === 1, "a second click without another A press should be normal selection");

  targetedInput.clientIntent.beginCommandTarget("move");
  targetedInput._onLeftDown({ x: 260, y: 260 }, { shiftKey: true });
  let lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "move", "move targeting should issue a move command");
  assert(lastSent.queued === true, "Shift move targeting should queue movement");

  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput._entityAtScreen = () => null;
  targetedInput._onLeftDown({ x: 280, y: 280 }, { shiftKey: true });
  lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "attackMove", "attack targeting terrain should attack-move");
  assert(lastSent.queued === true, "Shift attack-move targeting should queue attack-move");

  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput._entityAtScreen = () => ({ id: 99, owner: 2, kind: KIND.RIFLEMAN, x: 300, y: 300 });
  targetedInput._onLeftDown({ x: 300, y: 300 }, { shiftKey: true });
  lastSent = sentCommands[sentCommands.length - 1];
  assert(lastSent.c === "attack", "attack targeting an enemy should issue attack");
  assert(
    lastSent.queued === true,
    "Shift enemy attack targeting should queue attack",
  );

  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput.clientIntent.holdCommandTarget("attack", "KeyA", true);
  targetedInput._entityAtScreen = () => null;
  targetedInput._onLeftDown({ x: 320, y: 320 }, { shiftKey: true });
  assert(
    targetedInput.clientIntent.commandTarget === "attack",
    "Shift attack targeting should stay armed while A is held",
  );
  targetedInput._onLeftDown({ x: 340, y: 340 }, { shiftKey: true });
  assert(
    sentCommands.at(-2).c === "attackMove" &&
      sentCommands.at(-2).queued === true &&
      sentCommands.at(-1).c === "attackMove" &&
      sentCommands.at(-1).queued === true,
    "held A plus Shift should queue multiple attack-move orders",
  );
  targetedInput._onLeftDown({ x: 360, y: 360 }, { shiftKey: false });
  assert(
    targetedInput.clientIntent.commandTarget === "attack",
    "held A keeps attack targeting armed after an unqueued click",
  );

  targetedInput.clientIntent.endCommandTarget();
  targetedInput.clientIntent.beginCommandTarget("attack");
  targetedInput.clientIntent.holdCommandTarget("attack", "KeyA");
  targetedInput._handleKeyUp({ code: "KeyA", preventDefault() {} });
  assert(targetedInput.clientIntent.commandTarget === null, "A keyup exits sticky attack targeting");

  const originalDocument = globalThis.document;
  const hotkeyTargetedInput = Object.create(Input.prototype);
  const hotkeyIssues = [];
  const quickCastSelectionClicks = [];
  const quickCastBoxSelections = [];
  hotkeyTargetedInput.mouse = { x: 420, y: 260 };
  hotkeyTargetedInput.pointerLocked = false;
  hotkeyTargetedInput.cameraNavigation = null;
  hotkeyTargetedInput._panDrag = null;
  hotkeyTargetedInput._drag = null;
  hotkeyTargetedInput._dragging = false;
  hotkeyTargetedInput._placementDrag = null;
  hotkeyTargetedInput.screenOverlay = { setMarquee() {}, clearMarquee() {} };
  hotkeyTargetedInput._handleControlGroupHotkey = () => false;
  hotkeyTargetedInput._quickCastCommandTarget = (ev) => {
    hotkeyIssues.push({ shiftKey: !!ev.shiftKey, mouse: hotkeyTargetedInput.mouse });
    return Input.prototype._quickCastCommandTarget.call(hotkeyTargetedInput, ev);
  };
  hotkeyTargetedInput._issueTargetedCommand = (p, ev) => {
    hotkeyIssues.push({ issuedAt: p, queued: !!ev.shiftKey });
  };
  hotkeyTargetedInput._eventScreenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  hotkeyTargetedInput._screenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  hotkeyTargetedInput._trackMouse = () => {};
  hotkeyTargetedInput._commitClickSelection = (p) => quickCastSelectionClicks.push(p);
  hotkeyTargetedInput._commitBoxSelection = (drag) => quickCastBoxSelections.push(drag);
  hotkeyTargetedInput.state = {};
  hotkeyTargetedInput.clientIntent = new ClientIntent();
  globalThis.document = {
    getElementById(id) {
      assert(id === "command-card", "command hotkeys should query the command card");
      return {
        querySelectorAll(selector) {
          assert(selector === "button[data-hotkey]", "command hotkeys should query hotkey buttons");
          return [{
            dataset: { hotkey: "Y" },
            disabled: false,
            click() {
              hotkeyTargetedInput.clientIntent.beginCommandTarget("attack", { now: 100 + hotkeyIssues.length * 100 });
            },
          }];
        },
      };
    },
  };
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyA"));
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === null && hotkeyIssues.length === 0,
    "unbound legacy A key should not arm attack when Attack is rebound",
  );
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY"));
  hotkeyTargetedInput._handleKeyUp({ code: "KeyY", shiftKey: false, preventDefault() {} });
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === "attack",
    "plain targeted-order hotkey tap should stay armed after keyup",
  );
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY"));
  assert(
    hotkeyIssues.some((entry) => entry.issuedAt === hotkeyTargetedInput.mouse && entry.queued === false),
    "second same targeted-order hotkey should quick-cast at the cursor",
  );
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === null,
    "unqueued quick-cast should consume the armed targeted order",
  );
  hotkeyTargetedInput._onLeftDown({ x: 422, y: 261 }, {});
  hotkeyTargetedInput._handleMouseUp({
    button: 0,
    clientX: 422,
    clientY: 261,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(
    quickCastSelectionClicks.length === 0,
    "near click after unqueued quick-cast should not become selection",
  );
  assert(
    hotkeyTargetedInput._postQuickCastSelectionGuard === null,
    "post quick-cast selection guard should be one-shot",
  );

  armPostQuickCastSelectionGuard(hotkeyTargetedInput, { x: 420, y: 260 });
  hotkeyTargetedInput._onLeftDown({ x: 420, y: 260 }, {});
  hotkeyTargetedInput._handleMouseMove({ clientX: 428, clientY: 260 });
  hotkeyTargetedInput._handleMouseUp({
    button: 0,
    clientX: 428,
    clientY: 260,
    shiftKey: false,
    ctrlKey: false,
    metaKey: false,
  });
  assert(
    quickCastBoxSelections.length === 1,
    "drag after unqueued quick-cast should still perform box selection",
  );

  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY", { shiftKey: true }));
  hotkeyTargetedInput._handleKeyDown(keyEvent("KeyY", { shiftKey: true }));
  assert(
    hotkeyIssues.some((entry) => entry.issuedAt === hotkeyTargetedInput.mouse && entry.queued === true),
    "Shift double-tap targeted-order hotkey should quick-cast a queued order at the cursor",
  );
  assert(
    hotkeyTargetedInput.clientIntent.commandTarget === "attack",
    "Shift quick-cast should keep the targeted order armed until Shift is released",
  );
  hotkeyTargetedInput._handleKeyUp({ code: "KeyY", shiftKey: true, preventDefault() {} });
  hotkeyTargetedInput._handleKeyUp({ code: "ShiftLeft", preventDefault() {} });
  assert(hotkeyTargetedInput.clientIntent.commandTarget === null, "Shift release clears the queued hotkey target");
  globalThis.document = originalDocument;

  const placementKeyInput = Object.create(Input.prototype);
  let placementEnded = 0;
  let commandTargetShiftReleased = 0;
  let shiftKeyupPrevented = false;
  placementKeyInput.state = {};
  placementKeyInput.clientIntent = new ClientIntent();
  placementKeyInput.clientIntent.placement = { building: KIND.DEPOT, tileX: 2, tileY: 3, valid: true };
  placementKeyInput.clientIntent.releaseCommandTargetShift = () => {
    commandTargetShiftReleased += 1;
  };
  placementKeyInput.clientIntent.endPlacement = () => {
    placementEnded += 1;
    placementKeyInput.clientIntent.placement = null;
  };
  placementKeyInput._handleKeyUp({
    code: "ShiftRight",
    preventDefault() {
      shiftKeyupPrevented = true;
    },
  });
  assert(commandTargetShiftReleased === 1, "Shift release still clears command-target preservation");
  assert(placementEnded === 1 && placementKeyInput.clientIntent.placement === null, "Shift release clears build placement");
  assert(shiftKeyupPrevented === true, "Shift placement release prevents browser default");

  const placementBlurInput = Object.create(Input.prototype);
  let blurPlacementEnded = 0;
  placementBlurInput.pointerLocked = false;
  placementBlurInput.keys = { up: true, down: true, left: true, right: true };
  placementBlurInput.mouse = { x: 1, y: 2 };
  placementBlurInput._spacePan = true;
  placementBlurInput._panDrag = { x: 1, y: 2, button: 1 };
  placementBlurInput._drag = null;
  placementBlurInput.state = {};
  placementBlurInput.clientIntent = new ClientIntent();
  placementBlurInput.clientIntent.placement = { building: KIND.DEPOT, tileX: 2, tileY: 3, valid: true };
  placementBlurInput.clientIntent.endCommandTarget = () => {};
  placementBlurInput.clientIntent.endPlacement = () => {
    blurPlacementEnded += 1;
    placementBlurInput.clientIntent.placement = null;
  };
  placementBlurInput._handleBlur();
  assert(blurPlacementEnded === 1 && placementBlurInput.clientIntent.placement === null, "window blur clears build placement");

  const labBlurInput = Object.create(Input.prototype);
  let labBlurCancel = 0;
  labBlurInput.pointerLocked = false;
  labBlurInput.keys = { up: true, down: true, left: true, right: true };
  labBlurInput.mouse = { x: 1, y: 2 };
  labBlurInput._spacePan = true;
  labBlurInput._panDrag = { x: 1, y: 2, button: 1 };
  labBlurInput._drag = null;
  labBlurInput.state = {};
  labBlurInput.clientIntent = new ClientIntent();
  const labBlurTool = labBlurInput.clientIntent.beginLabTool({ kind: "spawnEntity", keepArmedOnWorldClick: true });
  labBlurInput.labToolController = {
    cancel() {
      labBlurCancel += 1;
      return labBlurInput.clientIntent.cancelLabTool("blur");
    },
  };
  labBlurInput._handleBlur();
  assert(labBlurInput.clientIntent.activeLabTool?.id === labBlurTool.id, "window blur leaves active lab spawn tools armed");
  assert(labBlurCancel === 0, "window blur does not route lab tool cancellation");

  const placementConfirmInput = Object.create(Input.prototype);
  const placementCommands = [];
  let confirmedPlacementEnded = 0;
  placementConfirmInput.commandIssuer = {
    command(command) {
      placementCommands.push(command);
    },
  };
  placementConfirmInput.state = {};
  placementConfirmInput.clientIntent = new ClientIntent();
  placementConfirmInput.clientIntent.placement = { building: KIND.DEPOT, tileX: 4, tileY: 5, valid: true };
  placementConfirmInput.clientIntent.endPlacement = () => {
    confirmedPlacementEnded += 1;
    placementConfirmInput.clientIntent.placement = null;
  };
  placementConfirmInput._selectedWorkerIds = () => [77];
  placementConfirmInput._confirmPlacement();
  assert(
    placementCommands.length === 1 &&
      placementCommands[0].c === "build" &&
      placementCommands[0].building === KIND.DEPOT &&
      placementCommands[0].tileX === 4 &&
      placementCommands[0].tileY === 5,
    "build placement confirm should send through a legacy one-argument command sender",
  );
  assert(confirmedPlacementEnded === 1, "build placement confirm exits placement after sending");

  const trapPlacementInput = Object.create(Input.prototype);
  const trapPlacementCommands = [];
  let trapPlacementEnded = 0;
  trapPlacementInput.commandIssuer = {
    command(command) {
      trapPlacementCommands.push(command);
    },
  };
  trapPlacementInput.state = {};
  trapPlacementInput.clientIntent = new ClientIntent();
  trapPlacementInput.clientIntent.placement = {
    building: KIND.TANK_TRAP,
    tileX: 0,
    tileY: 0,
    valid: true,
    lineSites: [
      { tileX: 0, tileY: 0, valid: true },
      { tileX: 2, tileY: 0, valid: false },
      { tileX: 4, tileY: 0, valid: true },
      { tileX: 6, tileY: 0, valid: true },
    ],
  };
  trapPlacementInput.clientIntent.endPlacement = () => {
    trapPlacementEnded += 1;
    trapPlacementInput.clientIntent.placement = null;
  };
  trapPlacementInput._selectedWorkerIds = () => [77, 88];
  trapPlacementInput._confirmPlacement({ shiftKey: true });
  assertDeepEqual(
    trapPlacementCommands.map((command) => [command.units, command.tileX, command.tileY, command.queued === true]),
    [
      [[77], 0, 0, false],
    ],
    "Tank Trap placement confirmation preserves spacing when preview sites contain a gap",
  );
  assert(trapPlacementEnded === 0, "Shift Tank Trap placement preserves placement mode without changing overflow queueing");

  const ekatEntity = { id: 88, owner: 1, kind: KIND.EKAT, x: 200, y: 220 };
  const ekatInput = Object.create(Input.prototype);
  ekatInput.mouse = { x: 360, y: 236 };
  ekatInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    abilityObjects: [
      {
        id: 901,
        owner: 1,
        ability: ABILITY.EKAT_LINE_SHOT,
        kind: ABILITY_OBJECT_KIND.MAGIC_ANCHOR,
        x: 260,
        y: 260,
        ownerState: { radius: 12 },
      },
      {
        id: 902,
        owner: 2,
        ability: ABILITY.EKAT_LINE_SHOT,
        kind: ABILITY_OBJECT_KIND.MAGIC_ANCHOR,
        x: 280,
        y: 280,
      },
    ],
    selectedEntities: () => [ekatEntity],
  };
  ekatInput.clientIntent = new ClientIntent();
  ekatInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.EKAT_LINE_SHOT });
  ekatInput._groundAtScreen = (x, y) => ({ x, y });
  ekatInput._refreshAbilityTargetPreview();
  assert(ekatInput.clientIntent.abilityTargetPreview?.pathOrigins.length === 2, "Ekat line preview includes caster plus owned anchor origin");
  assert(
    ekatInput.clientIntent.abilityTargetPreview.pathOrigins.some((origin) => origin.kind === ABILITY_OBJECT_KIND.MAGIC_ANCHOR),
    "Ekat line preview marks anchor origin kind",
  );
  const ekatPreviewGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: ekatPreviewGfx },
    { abilityTargetPreview: ekatInput.clientIntent.abilityTargetPreview },
  );
  assert(
    ekatPreviewGfx.calls.some(
      (call) => call[0] === "lineStyle" && call[2] === 0xc7d07a,
    ),
    "Ekat line preview draws Magic Anchor origins without crashing",
  );

  const returnInput = Object.create(Input.prototype);
  returnInput.mouse = { x: 420, y: 260 };
  returnInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    abilityObjects: [
      {
        id: 903,
        owner: 1,
        ability: ABILITY.EKAT_TELEPORT,
        kind: ABILITY_OBJECT_KIND.RETURN_MARKER,
        x: 180,
        y: 190,
        expiresIn: 70,
      },
    ],
    selectedEntities: () => [ekatEntity],
  };
  returnInput.clientIntent = new ClientIntent();
  returnInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.EKAT_TELEPORT });
  returnInput._groundAtScreen = (x, y) => ({ x, y });
  returnInput._refreshAbilityTargetPreview();
  assert(returnInput.clientIntent.abilityTargetPreview?.returnMarkers[0]?.id === 903, "Ekat dash preview exposes owned return marker preview");

  const abilityObjectGfx = new RecordingGraphics();
  _drawAbilityObjects.call(
    { _abilityObjectGfx: abilityObjectGfx, _map: { tileSize: 32 } },
    {
      abilityObjects: [
        { id: 904, kind: ABILITY_OBJECT_KIND.RETURN_MARKER, x: 128, y: 144 },
        { id: 905, kind: ABILITY_OBJECT_KIND.MAGIC_ANCHOR, x: 160, y: 192, ownerState: { hp: 80, radius: 16 } },
      ],
    },
  );
  assert(
    abilityObjectGfx.calls.some((call) => call[0] === "drawCircle" && call[1] === 128 && call[2] === 144) &&
      abilityObjectGfx.calls.some((call) => call[0] === "beginFill"),
    "ability object renderer draws return marker and anchor placeholders",
  );
}

{
  const tankTrapEntity = {
    id: 720,
    owner: 1,
    kind: KIND.TANK_TRAP,
    x: 64,
    y: 96,
    buildProgress: 0.5,
  };
  const fakePools = new Map();
  let iconCalls = 0;
  let tintCalls = 0;
  const fakeRenderer = {
    _map: { tileSize: 32 },
    _slot(pool, id) {
      const key = `${pool}:${id}`;
      if (!fakePools.has(key)) fakePools.set(key, new RecordingGraphics());
      return fakePools.get(key);
    },
    _tintFor() {
      tintCalls += 1;
      return 0x4878c8;
    },
    _icon() {
      iconCalls += 1;
    },
    _queueLabel() {},
  };
  _drawBuilding.call(fakeRenderer, tankTrapEntity, new Map([[1, 0x4878c8]]), {});
  const trapGraphics = fakePools.get("buildings:720");
  const trapPolygons = trapGraphics.calls.filter((call) => call[0] === "drawPolygon");
  const trapColors = trapGraphics.calls
    .filter((call) => call[0] === "beginFill" || call[0] === "lineStyle")
    .map((call) => (call[0] === "beginFill" ? call[1] : call[2]));
  assert(trapPolygons.length >= 12, "Tank Trap renderer draws filled steel hedgehog beams");
  assert(!trapColors.includes(0x4878c8), "Tank Trap renderer avoids owner/team coloring");
  assert(tintCalls === 0, "Tank Trap renderer does not request owner tint");
  assert(iconCalls === 0, "Tank Trap renderer uses geometry instead of the generic building stencil");
  const firstBeam = trapGraphics.calls.find((call) => call[0] === "drawPolygon")?.[1];
  const firstBeamXs = firstBeam.filter((_, index) => index % 2 === 0);
  const firstBeamYs = firstBeam.filter((_, index) => index % 2 === 1);
  const firstBeamSpan = Math.max(
    Math.max(...firstBeamXs) - Math.min(...firstBeamXs),
    Math.max(...firstBeamYs) - Math.min(...firstBeamYs),
  );
  assert(firstBeamSpan > 32, "Tank Trap renderer scales beams larger than the 1x1 footprint");

  const secondTrapEntity = { ...tankTrapEntity, id: 721 };
  _drawBuilding.call(fakeRenderer, secondTrapEntity, new Map([[1, 0x4878c8]]), {});
  const secondTrapGraphics = fakePools.get("buildings:721");
  const secondBeam = secondTrapGraphics.calls.find((call) => call[0] === "drawPolygon")?.[1];
  assert(
    firstBeam?.some((value, index) => Math.abs(value - secondBeam[index]) > 0.1),
    "Tank Trap renderer varies rotation between trap instances",
  );
}
