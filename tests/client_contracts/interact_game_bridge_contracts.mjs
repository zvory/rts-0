import assert from "node:assert/strict";

import {
  InteractGameBridge,
  interactGameLaunchEnabled,
  interactScenarioLaunchEnabled,
} from "../../client/src/interact_game_bridge.js";
import { interactLaunchUrl } from "../../scripts/interact/game_launch_url.ts";

const scenarioLaunchUrl = interactLaunchUrl({
  mode: "scenario",
  baseUrl: "http://localhost/",
  room: "interact-scenario-test",
  map: "Chokes",
  opponent: "ai_2_1",
  renderer: "pixi",
  seed: "",
  scenario: "",
  devScenario: { id: "direct_reverse_order", unit: "tank", count: 1, blocker: "", case: "" },
});

assert.equal(
  interactGameLaunchEnabled(new URL("http://localhost/?rtsLaunch=match&rtsRoom=interact-game-test&rtsRole=player&interact=game")),
  true,
  "the game bridge accepts only its isolated player launch URL",
);
assert.equal(
  interactGameLaunchEnabled(new URL("http://localhost/?rtsLaunch=match&rtsRoom=public-room&rtsRole=player&interact=game")),
  false,
  "the game bridge cannot attach to an arbitrary public match",
);
assert.equal(
  interactGameLaunchEnabled(new URL("http://localhost/?rtsLaunch=match&rtsRoom=interact-game-test&rtsRole=spectator&interact=game")),
  true,
  "the game bridge accepts its isolated AI-vs-AI spectator launch",
);
assert.equal(
  new URL(scenarioLaunchUrl).searchParams.get("interact"),
  "dev-scenario",
  "the Interact driver emits the public dev-scenario launch gate",
);
assert.equal(
  interactScenarioLaunchEnabled(new URL(scenarioLaunchUrl)),
  true,
  "the observation bridge accepts the driver's bounded dev-scenario launch",
);
assert.equal(
  interactScenarioLaunchEnabled(new URL("http://localhost/?watchScenario=1&id=direct_reverse_order&unit=tank&count=1&interact=game")),
  false,
  "dev scenarios require their separate dev-scenario namespace gate",
);
assert.equal(
  interactScenarioLaunchEnabled(new URL("http://localhost/?watchScenario=1&id=direct_reverse_order&unit=tank&count=1&interact=scenario")),
  false,
  "the observation bridge rejects the retired scenario launch gate",
);
assert.equal(
  interactScenarioLaunchEnabled(new URL("http://localhost/?watchScenario=1&id=bad/scenario&unit=tank&count=1&interact=dev-scenario")),
  false,
  "the scenario bridge rejects unsafe launch tokens",
);

const previousDocument = globalThis.document;
const previousRequestAnimationFrame = globalThis.requestAnimationFrame;
const elements = new Map([
  ["game-screen", node({ hidden: false })],
  ["viewport", node({ hidden: false, rect: { x: 0, y: 0, width: 1000, height: 700 } })],
  ["hud", node({ hidden: false })],
  ["res-steel", node({ text: "100" })],
  ["res-oil", node({ text: "75" })],
  ["res-supply", node({ text: "2 / 10" })],
  ["game-timer", node({ text: "00:03" })],
  ["idle-workers-count", node({ text: "1" })],
  ["selected-panel", node({ text: "Rifleman" })],
  ["command-card", node({ buttons: [node({ text: "Move" })] })],
  ["give-up-confirm", node({ hidden: true })],
  ["game-over", node({ hidden: true })],
  ["game-over-text", node({ text: "Defeat" })],
  ["game-over-scores", node({ text: "Interact 0 — AI 1" })],
]);
globalThis.document = {
  fonts: { status: "loaded" },
  getElementById: (id) => elements.get(id) || null,
};
globalThis.requestAnimationFrame = (callback) => callback();

try {
  let snapshotSequence = 1;
  const entities = [
    { id: 10, kind: "rifleman", owner: 1, x: 100, y: 100, hp: 100, maxHp: 100, state: "idle", orderPlan: [] },
    { id: 20, kind: "rifleman", owner: 2, x: 300, y: 300, hp: 100, maxHp: 100, state: "idle", orderPlan: [] },
    { id: 30, kind: "rifleman", owner: 1, x: 120, y: 120, hp: 0, maxHp: 100, state: "dead", visionOnly: true, orderPlan: [] },
    { id: 40, kind: "rifleman", owner: 2, x: 320, y: 320, hp: 100, maxHp: 100, state: "firing", shotReveal: true, orderPlan: [] },
  ];
  let issuedCommand = null;
  let overview = null;
  let autoSpectatorEnabled = true;
  const presentationCalls = [];
  const match = {
    giveUpSent: false,
    matchRunId: "run-test",
    state: {
      get currRecvTime() { return snapshotSequence; },
      tick: 3,
      selection: new Set(),
      spectator: false,
      playerId: 1,
      localPlayer: { id: 1, teamId: 1, factionId: "kriegsia", name: "Interact", color: "#fff", isAi: false },
      players: [
        { id: 1, teamId: 1, factionId: "kriegsia", name: "Interact", color: "#fff", isAi: false },
        { id: 2, teamId: 2, factionId: "kriegsia", name: "AI", color: "#000", isAi: true },
      ],
      resources: { steel: 100, oil: 75, supplyUsed: 2, supplyCap: 10 },
      map: { name: "Chokes", width: 64, height: 64, tileSize: 32 },
      entitiesInterpolated: () => entities,
      entityById: (id) => entities.find((entity) => entity.id === id) || null,
      setSelection(ids) { this.selection = new Set(ids); },
      selectedEntities() { return entities.filter((entity) => this.selection.has(entity.id)); },
      worldInBounds: (x, y) => x >= 0 && y >= 0 && x < 2048 && y < 2048,
    },
    commandIssuer: {
      issueCommand(command) {
        issuedCommand = command;
        snapshotSequence += 1;
        match.state.tick += 1;
        return { sent: true, clientSeq: 7 };
      },
    },
    requestGiveUp() {
      this.giveUpSent = true;
      elements.get("game-over").hidden = false;
    },
    handleResize: () => presentationCalls.push("match-resize"),
    capabilities: { roomTime: { setSpeed: true } },
    roomTimeControls: { roomTimeState: { currentTick: 3, speed: 1, paused: false, ended: false } },
    net: {
      setRoomTimeSpeed(speed) {
        match.roomTimeControls.roomTimeState = { ...match.roomTimeControls.roomTimeState, speed, paused: false };
        return true;
      },
    },
    setAutoSpectatorEnabled(enabled) { autoSpectatorEnabled = enabled; },
    autoSpectator: {
      get enabled() { return autoSpectatorEnabled; },
    },
    camera: {
      snapshot: () => ({ version: 1, focus: { x: 0, y: 0 }, framingScale: 1, boundsPolicy: "mapOverscroll" }),
      projectionSnapshot: () => ({ viewport: { widthCssPx: 1000, heightCssPx: 700 } }),
      viewportGroundBounds: () => ({ minX: 0, minY: 0, maxX: 2048, maxY: 2048 }),
      containsProjected: () => true,
      fitWorldPoints: (points, options) => { overview = { points, options }; },
    },
  };
  const windowLike = {};
  const bridge = new InteractGameBridge({
    app: {
      match,
      matchLaunch: { room: "interact-game-test" },
      matchLaunchFailed: false,
      net: { ws: { readyState: 1 } },
      setCleanPresentation: (enabled) => presentationCalls.push(`app-presentation:${enabled}`),
    },
    windowLike,
    enabled: true,
    sleep: async () => {},
  });

  assert.equal(bridge.status().ready, true, "an authoritative isolated match makes the game bridge ready");
  assert.equal(bridge.status().role, "player", "status identifies the controlled-player launch");
  assert.equal(bridge.status().autoSpectatorEnabled, true, "status projects the active fight-following director state");
  const inspection = bridge.inspect();
  assert.deepEqual(inspection.entities.map(({ id }) => id), [10], "inspection defaults to locally owned entities");
  assert.equal(inspection.entities[0].controllable, true, "inspection labels movable local units");
  assert.deepEqual(inspection.ui.resources, { steel: "100", oil: "75", supply: "2 / 10" }, "inspection projects bounded HUD text");
  await bridge.presentation({ mode: "clean" });
  assert.deepEqual(presentationCalls, ["app-presentation:true"], "presentation delegates its one resize to the app-owned seam");
  assert.deepEqual(
    bridge.inspect({ ownership: "visible" }).entities.map(({ id }) => id),
    [10, 20],
    "inspection excludes shot-reveal and lingering vision-only render records",
  );
  const selected = await bridge.select({ entityIds: [10] });
  assert.deepEqual(selected.selection, [10], "game bridge selects a visible entity through browser-local state");
  assert.deepEqual(bridge.status().selection, [10], "game status carries selection into capture provenance");
  assert.deepEqual(bridge.captureReadiness().selection, [10], "game capture readiness records the rendered selection");
  assert.deepEqual(bridge.inspect().selection, [10], "game inspection reports the current selection ids");
  await assert.rejects(
    () => bridge.select({ entityIds: [30] }),
    (error) => error?.code === "unknownEntity",
    "game selection rejects non-selectable lingering vision records",
  );

  const moved = await bridge.move({ units: [10], x: 500, y: 500 });
  assert.equal(moved.accepted, true, "move reports normal client admission");
  assert.deepEqual(issuedCommand, { c: "move", units: [10], x: 500, y: 500 }, "move builds exactly one normal move command");
  await assert.rejects(
    () => bridge.time({ action: "speed", speed: 8 }),
    (error) => error?.code === "spectatorRequired",
    "ordinary game players cannot control room time",
  );
  await assert.rejects(() => bridge.move({ units: [20], x: 500, y: 500 }), (error) => error?.code === "notControllable");
  await assert.rejects(
    () => bridge.move({ units: [30], x: 500, y: 500 }),
    (error) => error?.code === "notControllable",
    "lingering vision records cannot be moved as owned units",
  );

  const gaveUp = await bridge.giveUp();
  assert.equal(gaveUp.phase, "concluded", "give-up waits for the score screen");
  assert.equal(gaveUp.ui.scoreScreenVisible, true, "give-up returns the concluded UI state");
  assert.equal(bridge.captureReadiness().phase, "concluded", "capture readiness identifies the stable stopped score-screen frame");
  elements.get("game-over").hidden = true;
  match.giveUpSent = false;
  match.state.spectator = true;
  assert.equal(bridge.status().role, "spectator", "status identifies the AI-vs-AI spectator launch");
  assert.equal(bridge.status().ready, true, "an active spectator remains fully inspectable");
  assert.deepEqual((await bridge.select({ entityIds: [20] })).selection, [20], "AI-vs-AI spectators can select visible AI units");
  await assert.rejects(() => bridge.move({ units: [10], x: 600, y: 600 }), (error) => error?.code === "playerSeatRequired");
  const time = await bridge.time({ action: "speed", speed: 8 });
  assert.equal(time.roomTime.speed, 8, "AI-only spectator time control confirms the authoritative speed");
  match.state.spectator = false;
  const scenarioBridge = new InteractGameBridge({
    app: bridge.app,
    windowLike: {},
    enabled: true,
    mode: "scenario",
    sleep: async () => {},
  });
  const scenarioTime = await scenarioBridge.time({ action: "speed", speed: 4 });
  assert.equal(scenarioTime.roomTime.speed, 4, "active-player dev scenarios retain room-time control for time-lapse capture");
  assert.deepEqual((await scenarioBridge.select({ entityIds: [20] })).selection, [20], "dev-scenario observers share browser-local selection support");
  const rejectedScenarioMove = await scenarioBridge.call("move", { units: [10], x: 600, y: 600 });
  assert.equal(rejectedScenarioMove.ok, false, "active-player dev scenarios expose no gameplay mutation bridge");
  assert.equal(rejectedScenarioMove.error.code, "methodUnavailable", "scenario gameplay mutations fail at the page-bridge boundary");
  scenarioBridge.destroy();
  match.state.spectator = true;
  bridge.camera({ action: "overview", padding: 20 });
  assert.equal(autoSpectatorEnabled, false, "whole-map framing disables automatic fight-following camera movement");
  assert.deepEqual(overview, {
    points: [{ x: 0, y: 0 }, { x: 2048, y: 2048 }],
    options: { paddingCssPx: 20 },
  }, "whole-map framing fits authoritative map bounds with CSS padding");
  bridge.destroy();
} finally {
  if (previousDocument === undefined) delete globalThis.document;
  else globalThis.document = previousDocument;
  if (previousRequestAnimationFrame === undefined) delete globalThis.requestAnimationFrame;
  else globalThis.requestAnimationFrame = previousRequestAnimationFrame;
}

function node({ hidden = false, text = "", rect = null, buttons = [] } = {}) {
  return {
    hidden,
    textContent: text,
    getBoundingClientRect: () => rect,
    querySelectorAll: () => buttons,
  };
}

console.log("✅ interact_game_bridge_contracts.mjs: isolated launch gate, UI inspection, move, and give-up passed");
