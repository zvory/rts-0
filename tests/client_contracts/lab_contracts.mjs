// tests/client_contracts/lab_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertDeepEqual,
} from "./assertions.mjs";
import {
  fakeStorage,
  findFakes,
  withFakeDocument,
} from "./fakes.mjs";
import { Net } from "../../client/src/net.js";
import {
  DEFAULT_FACTION_ID,
  KIND,
  LAB_SCENARIO,
  LAB_ROLE,
  UPGRADE,
  cmd,
} from "../../client/src/protocol.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import {
  LabClient,
  labVision,
  labVisionLabel,
} from "../../client/src/lab_client.js";
import {
  createDefaultControlPolicy,
  createLabControlPolicy,
} from "../../client/src/lab_control_policy.js";
import {
  LabPanel,
  labBuildingSpawnFactionOptions,
  labSpawnBuildingKindsForFaction,
  labSpawnFactionOptions,
  labSpawnUnitKindsForFaction,
} from "../../client/src/lab_panel.js";
import { LabPanelWindowChrome } from "../../client/src/lab_panel_window.js";

import { textWithin } from "./dom_text.mjs";

// Lab client and panel
// ---------------------------------------------------------------------------
{
  const sent = [];
  const net = new Net("ws://example.test/ws");
  net.ws = {
    readyState: WebSocket.OPEN,
    bufferedAmount: 0,
    send(json) {
      sent.push(JSON.parse(json));
    },
  };
  const labClient = new LabClient(net, { timeoutMs: 1000 });
  let observedState = null;
  let observedResult = null;
  labClient.subscribeState((state) => {
    observedState = state;
  });
  labClient.subscribeResult((result) => {
    observedResult = result;
  });
  labClient.setInitialState({
    room: "__lab__:sandbox:map=Default",
    operatorId: 1,
    role: LAB_ROLE.OPERATOR,
    vision: labVision.fullWorld(),
    dirty: false,
    operationCount: 0,
  });
  assert(observedState.role === "operator", "LabClient publishes initial lab state");
  const resultPromise = labClient.setVision(labVision.team(2));
  assert(
    sent.at(-1).op.vision.mode === "team" && sent.at(-1).requestId === 1,
    "LabClient allocates request ids for vision operations",
  );
  net._emit("labResult", { t: "labResult", requestId: 1, ok: true, op: "setVision" });
  const result = await resultPromise;
  assert(result.ok && observedResult.ok, "LabClient resolves matching labResult messages");
  void labClient.spawnEntity({ owner: 2, kind: KIND.RIFLEMAN, x: 128, y: 160, completed: true });
  assert(sent.at(-1).op.op === "spawnEntity" && sent.at(-1).op.kind === KIND.RIFLEMAN, "LabClient sends spawn operations");
  void labClient.setCompletedResearch(1, UPGRADE.TANK_UNLOCK, true);
  assert(sent.at(-1).op.op === "setCompletedResearch" && sent.at(-1).op.upgrade === UPGRADE.TANK_UNLOCK, "LabClient sends research operations");
  void labClient.exportScenario("saved setup");
  assert(sent.at(-1).op.op === "exportScenario" && sent.at(-1).op.name === "saved setup", "LabClient sends scenario export requests");
  void labClient.importScenario({
    schemaVersion: LAB_SCENARIO.SCHEMA_VERSION,
    kind: LAB_SCENARIO.KIND,
    entities: [{ id: 7, setUp: true, setupTarget: { x: 128, y: 160 } }],
  });
  assert(
    sent.at(-1).op.op === "importScenario" &&
      sent.at(-1).op.scenario.kind === LAB_SCENARIO.KIND &&
      sent.at(-1).op.scenario.entities[0].setupTarget.x === 128,
    "LabClient sends scenario import requests with setup fields",
  );
  assert(labVisionLabel(labVision.teams([1, 2])) === "Teams 1, 2", "labVisionLabel formats team unions");
  labClient.destroy();
}

{
  const requests = [];
  const policy = createLabControlPolicy({
    labClient: { request: (op) => { requests.push(op); return Promise.resolve({ ok: true }); } },
    metadata: { role: LAB_ROLE.OPERATOR },
  });
  assert(policy.kind === "lab" && policy.canIssueAs(1), "lab control policy gates issue-as to operator");
  const state = {
    selectedEntities() {
      return [{ id: 11, owner: 2, kind: KIND.RIFLEMAN }];
    },
  };
  assert(policy.canControlOwner(2, state), "lab control policy controls a single selected owner");
  assert(!policy.canControlOwner(1, state), "lab control policy rejects non-selected owners");
  assert(policy.canUseCommandSurface(state), "lab operator can use the command surface");
  const issued = await policy.issueCommand(cmd.move([11], 20, 30), { state });
  assert(
    issued.sent && requests[0].playerId === 2 && requests[0].ignoreCommandLimits === true,
    "lab control policy routes gameplay commands through issue-as with command limits disabled by default",
  );
  const overBudgetUnits = Array.from({ length: 25 }, (_, index) => ({
    id: 100 + index,
    owner: 2,
    kind: KIND.RIFLEMAN,
    state: "idle",
  }));
  const overBudgetState = {
    selectedEntities() {
      return overBudgetUnits;
    },
    entityById(id) {
      return overBudgetUnits.find((entity) => entity.id === id) || null;
    },
  };
  policy.setIgnoreCommandLimits(false);
  const blocked = policy.issueCommand(cmd.stop(overBudgetUnits.map((entity) => entity.id)), { state: overBudgetState });
  assert(blocked.blocked === "commandBudget", "lab control policy can restore the command supply guard");
  policy.setIgnoreCommandLimits(true);
  const mixedState = {
    selectedEntities() {
      return [{ id: 11, owner: 1, kind: KIND.RIFLEMAN }, { id: 12, owner: 2, kind: KIND.RIFLEMAN }];
    },
  };
  assert(!policy.canIssueGameplayCommand(cmd.stop([11, 12]), mixedState).ok, "lab policy rejects mixed-owner gameplay commands");
  assert(
    !createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } }).canUseCommandSurface(state),
    "read-only lab viewers cannot use the command surface",
  );
  assert(!createDefaultControlPolicy().canUseCommandSurface({ spectator: true }), "default spectators cannot use the command surface");
  assert(!createDefaultControlPolicy().canIssueAs(1), "default control policy does not issue-as");
}

{
  assertDeepEqual(
    labSpawnFactionOptions().map((entry) => entry.id),
    ["kriegsia", "ekat"],
    "LabPanel spawn palette exposes product-playable faction catalogs",
  );
  assert(
    labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.RIFLEMAN),
    "LabPanel spawn palette includes Kriegsia catalog units",
  );
  assert(
    !labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.CITY_CENTRE),
    "LabPanel spawn palette excludes buildings from primary unit options",
  );
  assertDeepEqual(
    labSpawnUnitKindsForFaction("ekat"),
    [KIND.EKAT],
    "LabPanel spawn palette filters Ekat to Ekat units",
  );
  assertDeepEqual(
    labBuildingSpawnFactionOptions().map((entry) => entry.id),
    ["kriegsia", "ekat"],
    "LabPanel building spawn palette exposes product-playable faction catalogs",
  );
  assert(
    labSpawnBuildingKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.CITY_CENTRE),
    "LabPanel building spawn palette includes Kriegsia catalog buildings",
  );
  assert(
    !labSpawnBuildingKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.RIFLEMAN),
    "LabPanel building spawn palette excludes units from building options",
  );
  assertDeepEqual(
    labSpawnBuildingKindsForFaction("ekat"),
    [KIND.ZAMOK],
    "LabPanel building spawn palette filters Ekat to Ekat buildings",
  );
}

await withFakeDocument(async () => {
  const buildLabClient = (role) => {
    const net = new Net("ws://example.test/ws");
    const labClient = new LabClient(net);
    labClient.setInitialState({
      room: "__lab__:sandbox:map=Default",
      operatorId: 1,
      role,
      vision: labVision.fullWorld(),
      dirty: true,
      operationCount: 7,
    });
    return labClient;
  };
  const buildMatch = (panelRef) => ({
    clientIntent: new ClientIntent(),
    state: {
      map: { width: 64, height: 64 },
      playerResources: [],
      selectedEntities() {
        return [];
      },
    },
    armLabTool(tool) {
      const armed = this.clientIntent.beginLabTool({ id: `tool-${tool.kind}-${panelRef.root.children.length}`, ...tool });
      panelRef.panel?.applyLabToolChange({ type: "armed", tool: armed });
      return armed;
    },
    cancelLabTool(reason) {
      const cancelled = this.clientIntent.cancelLabTool(reason);
      if (cancelled) panelRef.panel?.applyLabToolChange({ type: "cancelled", reason, tool: cancelled });
      return cancelled;
    },
  });
  const startPayload = {
    map: { name: "Default" },
    players: [
      { id: 1, teamId: 1, color: "#2255aa" },
      { id: 2, teamId: 2, color: "#bb4422" },
    ],
  };

  const rootA = document.createElement("div");
  const rootB = document.createElement("div");
  const refA = { root: rootA, panel: null };
  const refB = { root: rootB, panel: null };
  const operatorA = buildLabClient(LAB_ROLE.OPERATOR);
  const operatorB = buildLabClient(LAB_ROLE.OPERATOR);
  const matchA = buildMatch(refA);
  const matchB = buildMatch(refB);
  refA.panel = new LabPanel({ root: rootA, labClient: operatorA, startPayload, match: matchA });
  refB.panel = new LabPanel({ root: rootB, labClient: operatorB, startPayload, match: matchB });

  assert(textWithin(rootB).includes("Operator"), "later lab joiner operator role renders as Operator");
  assert(refB.panel.fields.has("lab-player"), "later lab joiner operator receives target player controls");
  refA.panel.armSpawnPaletteTool(KIND.RIFLEMAN);
  assert(textWithin(rootA).includes("Armed: Spawn Rifleman"), "one lab tab can arm a setup tool locally");
  assert(textWithin(rootB).includes("No setup tool armed"), "another lab tab keeps its own setup tool state");

  const readOnlyRoot = document.createElement("div");
  const readOnlyClient = buildLabClient(LAB_ROLE.READ_ONLY);
  const readOnlyRef = { root: readOnlyRoot, panel: null };
  readOnlyRef.panel = new LabPanel({
    root: readOnlyRoot,
    labClient: readOnlyClient,
    startPayload,
    match: buildMatch(readOnlyRef),
  });
  assert(textWithin(readOnlyRoot).includes("Read-only"), "read-only lab role renders read-only status");
  assert(!readOnlyRef.panel.fields.has("lab-player"), "read-only lab role does not expose target player controls");

  refA.panel.destroy();
  refB.panel.destroy();
  readOnlyRef.panel.destroy();
  operatorA.destroy();
  operatorB.destroy();
  readOnlyClient.destroy();
});

await withFakeDocument(async () => {
  const sent = [];
  const net = new Net("ws://example.test/ws");
  net.ws = {
    readyState: WebSocket.OPEN,
    bufferedAmount: 0,
    send(json) {
      sent.push(JSON.parse(json));
    },
  };
  const root = document.createElement("div");
  const labClient = new LabClient(net);
  let armedTool = null;
  let armedCallbacks = null;
  let cancelledToolReason = null;
  let selectedEntities = [];
  let ignoreCommandLimits = true;
  let panel = null;
  const match = {
    clientIntent: new ClientIntent(),
    camera: { x: 320, y: 352 },
    state: {
      map: { width: 64, height: 64 },
      playerResources: [{ steel: 500, oil: 200 }],
      controlPolicy: {
        ignoreCommandLimitsEnabled() {
          return ignoreCommandLimits;
        },
        setIgnoreCommandLimits(enabled) {
          ignoreCommandLimits = !!enabled;
        },
      },
      selectedEntities() {
        return selectedEntities;
      },
    },
    armLabTool(tool, callbacks) {
      armedTool = this.clientIntent.beginLabTool({ id: "lab-tool-test", ...tool });
      const toolAtArm = armedTool;
      armedCallbacks = {
        onWorldClick: (event) => {
          const result = callbacks.onWorldClick?.(event);
          if (!toolAtArm.keepArmedOnWorldClick) this.cancelLabTool("worldClick");
          return result;
        },
        onBoxSelection: (event) => {
          const result = callbacks.onBoxSelection?.(event);
          if (!toolAtArm.keepArmedOnBoxSelection) this.cancelLabTool("boxSelect");
          return result;
        },
      };
      panel?.applyLabToolChange({ type: "armed", tool: armedTool });
      return armedTool;
    },
    cancelLabTool(reason) {
      cancelledToolReason = reason;
      const cancelled = this.clientIntent.cancelLabTool(reason);
      if (cancelled) panel?.applyLabToolChange({ type: "cancelled", reason, tool: cancelled });
      return cancelled;
    },
  };
  labClient.setInitialState({
    room: "__lab__:sandbox:map=Default",
    operatorId: 1,
    role: LAB_ROLE.OPERATOR,
    vision: labVision.fullWorld(),
    dirty: false,
    operationCount: 0,
  });
  panel = new LabPanel({
    root,
    labClient,
    launch: { publicRoom: "sandbox", map: "Default" },
    startPayload: {
      map: { name: "Default" },
      players: [
        { id: 1, teamId: 1, color: "#2255aa" },
        { id: 2, teamId: 2, color: "#bb4422" },
      ],
    },
    match,
  });
  const buttonByText = (label) => findFakes(root, (el) => el.tagName === "BUTTON" && el.textContent === label)[0];
  const playerButtonById = (id) => findFakes(root, (el) => (
    el.tagName === "BUTTON" && el.dataset?.playerId === String(id)
  ))[0];
  const playerButtons = () => findFakes(root, (el) => (
    el.tagName === "BUTTON" && String(el.className).includes("lab-player-btn")
  ));
  const spawnPanel = (kind) => findFakes(root, (el) => (
    el.tagName === "SECTION" && el.dataset?.spawnPanel === kind
  ))[0];
  const sectionByClass = (className) => findFakes(root, (el) => (
    el.tagName === "SECTION" && String(el.className).split(/\s+/).includes(className)
  ))[0];
  const panelByClass = (className) => findFakes(root, (el) => (
    el.tagName === "ASIDE" && String(el.className).split(/\s+/).includes(className)
  ))[0];
  const resolveLastLabResult = (options = {}) => {
    const envelope = sent.at(-1);
    net._emit("labResult", {
      t: "labResult",
      requestId: envelope.requestId,
      ok: options.ok !== false,
      op: envelope.op.op,
      error: options.error || "",
      outcome: options.outcome || null,
    });
  };

  const optionsPanel = panelByClass("lab-options-window");
  const toolsPanel = panelByClass("lab-tools-window");
  assert(root.children.length === 2, "LabPanel mounts separate options and tools windows inside the app-owned root");
  assert(optionsPanel && toolsPanel && !toolsPanel.hidden, "LabPanel shows both floating windows for operators");
  assert(
    findFakes(root, (el) => String(el.className).split(/\s+/).includes("lab-panel-reset")).length === 0,
    "LabPanel omits reset buttons from its movable panels",
  );
  assert(
    [optionsPanel, toolsPanel].every((child) => child.children.some((grandchild) => grandchild.className === "lab-panel-resize-handle")),
    "LabPanel exposes visible resize handles for both windows",
  );
  assert(
    findFakes(root, (el) => el.tagName === "BUTTON" && el.dataset?.labPanelCollapse === "true").length >= 2,
    "LabPanel exposes collapse arrow affordances for both windows",
  );
  const headerKickers = findFakes(root, (el) => el.className === "lab-panel-kicker").map((el) => el.textContent);
  assert(
    headerKickers.includes("Options") &&
      headerKickers.includes("Tools") &&
      !headerKickers.some((text) => /^Lab /.test(text)),
    "LabPanel window headers use compact Options and Tools labels",
  );
  assert(
    findFakes(optionsPanel, (el) => el.tagName === "H2").length === 0 &&
      findFakes(toolsPanel, (el) => el.tagName === "H2").length === 0,
    "LabPanel window headers omit the room/map title",
  );
  assert(textWithin(root).includes("Operator"), "LabPanel renders role state");
  assert(buttonByText("Cancel tool").disabled, "LabPanel disables tool cancellation when no setup tool is armed");
  assert(panel.fields.has("lab-player"), "LabPanel tracks one shared target player for lab setup tools");
  assert(
    playerButtons().length === 2 &&
      playerButtonById(1)?.dataset.color === "#2255aa" &&
      playerButtonById(2)?.dataset.color === "#bb4422",
    "LabPanel renders one team-colored target button per player",
  );
  assert(
    playerButtonById(1)?.dataset.selected === "true" &&
      playerButtonById(2)?.dataset.selected === "false",
    "LabPanel marks the selected target player button",
  );
  assert(!textWithin(root).includes("Advanced Spawn"), "LabPanel omits the advanced spawn form");
  assert(
    textWithin(root).includes("Unit Spawn") && textWithin(root).includes("Building Spawn"),
    "LabPanel renders separate unit and building spawn sections",
  );
  assert(
    textWithin(root).includes("Options") && textWithin(root).includes("Unlimited commands"),
    "LabPanel renders command limit controls in an Options panel",
  );
  assert(
    textWithin(sectionByClass("lab-options")).includes("Vision") &&
      textWithin(sectionByClass("lab-options")).includes("Unlimited commands") &&
      textWithin(sectionByClass("lab-options")).includes("Scenario") &&
      !textWithin(sectionByClass("lab-options")).includes("Unit Spawn") &&
      !textWithin(sectionByClass("lab-options")).includes("Player State"),
    "LabPanel groups global controls in the Options section",
  );
  assert(
    textWithin(sectionByClass("lab-tools")).includes("Unit Spawn") &&
      textWithin(sectionByClass("lab-tools")).includes("Building Spawn") &&
      textWithin(sectionByClass("lab-tools")).includes("Player State") &&
      textWithin(sectionByClass("lab-tools")).includes("Remove tool") &&
      !textWithin(sectionByClass("lab-tools")).includes("Unlimited commands") &&
      !textWithin(sectionByClass("lab-tools")).includes("Scenario"),
    "LabPanel groups placement tools in the Tools section",
  );
  assert(!textWithin(root).includes("Map Tools"), "LabPanel removes the old Map Tools section label");
  assert(!buttonByText("Move to point") && !buttonByText("Set owner"), "LabPanel removes the old selected move and owner tools");
  assert(!textWithin(root).includes("Unlimited selection"), "LabPanel removes the old unlimited selection option");
  const commandLimitToggle = panel.fields.get("ignore-command-limits");
  commandLimitToggle.checked = false;
  commandLimitToggle.listeners.change();
  assert(!ignoreCommandLimits, "LabPanel command option toggles command limit policy");
  assert(textWithin(root).includes("Command limit restored."), "LabPanel summarizes command limit restoration");
  assert(
    spawnPanel("units")?.dataset.targetPlayerId === "1" &&
      spawnPanel("units")?.dataset.targetColor === "#2255aa" &&
      spawnPanel("buildings")?.dataset.targetPlayerId === "1" &&
      spawnPanel("buildings")?.dataset.targetColor === "#2255aa",
    "LabPanel tints spawn panels with the selected target player's color",
  );
  assert(
    !panel.fields.has("spawn-owner") &&
      !panel.fields.has("advanced-spawn-owner") &&
      !panel.fields.has("resource-player") &&
      !panel.fields.has("research-player") &&
      !panel.fields.has("set-owner"),
    "LabPanel does not render per-tool player selectors for spawn or player-state controls",
  );
  assert(
    !textWithin(root).includes("Advanced Spawn") &&
      !panel.fields.has("advanced-spawn-kind") &&
      !panel.fields.has("advanced-spawn-completed") &&
      !panel.fields.has("spawn-completed") &&
      !panel.fields.has("research-completed"),
    "LabPanel does not expose advanced spawn or completion toggles",
  );
  const teamButton = buttonByText("Team 2");
  teamButton.listeners.click();
  assert(sent.at(-1).op.vision.teamId === 2, "LabPanel vision controls send lab vision requests");
  playerButtonById(2).listeners.click();
  assert(panel.fields.get("lab-player").value === "2", "LabPanel target player buttons update shared target state");
  assert(
    spawnPanel("units")?.dataset.targetPlayerId === "2" &&
      spawnPanel("units")?.dataset.targetColor === "#bb4422" &&
      spawnPanel("buildings")?.dataset.targetPlayerId === "2" &&
      spawnPanel("buildings")?.dataset.targetColor === "#bb4422",
    "LabPanel retints spawn panels when the target player changes",
  );
  panel.armSpawnPaletteTool(KIND.RIFLEMAN);
  assert(armedTool?.kind === "spawnEntity", "LabPanel unit palette arms the spawn lab tool through Match");
  assert(armedTool?.keepArmedOnWorldClick === true, "LabPanel unit palette keeps the spawn tool armed across world clicks");
  assert(textWithin(root).includes("Armed: Spawn Rifleman"), "LabPanel shows the armed spawn tool state");
  assert(!buttonByText("Cancel tool").disabled, "LabPanel enables tool cancellation while a setup tool is armed");
  assert(
    armedTool.payload.owner === 2 &&
      armedTool.payload.kind === KIND.RIFLEMAN &&
      armedTool.payload.factionId === DEFAULT_FACTION_ID &&
      armedTool.payload.completed === true,
    "LabPanel unit palette captures owner, faction, and kind with completed spawn payloads",
  );
  armedCallbacks.onWorldClick({ tool: { ...armedTool }, x: 128.5, y: 160.25 });
  assert(match.clientIntent.activeLabTool?.id === armedTool.id, "LabPanel spawn tool stays armed after sending a spawn request");
  assert(
    sent.at(-1).op.op === "spawnEntity" &&
      sent.at(-1).op.owner === 2 &&
      sent.at(-1).op.kind === KIND.RIFLEMAN &&
      sent.at(-1).op.x === 128.5 &&
      sent.at(-1).op.y === 160.25 &&
      sent.at(-1).op.completed === true,
    "LabPanel spawn tool sends clicked world coordinates through LabClient with completed spawns",
  );
  net._emit("labResult", {
    t: "labResult",
    requestId: sent.at(-1).requestId,
    ok: false,
    op: "spawnEntity",
    error: "occupied placement",
  });
  assert(textWithin(root).includes("occupied placement"), "LabPanel surfaces rejected spawn results through the status path");
  assert(match.clientIntent.activeLabTool?.id === armedTool.id, "LabPanel spawn tool stays armed after rejected spawn results");
  assert(!buttonByText("Cancel tool").disabled, "LabPanel keeps cancellation available after rejected spawn results");
  panel.armSpawnPaletteTool(KIND.RIFLEMAN);
  match.cancelLabTool("escape");
  assert(textWithin(root).includes("Spawn Rifleman cancelled."), "LabPanel surfaces keyboard cancellation through the status path");
  panel.fields.get("spawn-faction").value = "ekat";
  panel.fields.get("spawn-faction").listeners.change();
  assert(panel.spawnPalette.kind === KIND.EKAT, "LabPanel faction selection updates the unit palette deterministically");
  panel.armBuildingSpawnPaletteTool(KIND.CITY_CENTRE);
  assert(armedTool?.kind === "spawnEntity", "LabPanel building palette arms the spawn lab tool through Match");
  assert(
    armedTool.payload.owner === 2 &&
      armedTool.payload.kind === KIND.CITY_CENTRE &&
      armedTool.payload.factionId === DEFAULT_FACTION_ID &&
      armedTool.payload.completed === true,
    "LabPanel building palette captures owner, faction, and kind with completed spawn payloads",
  );
  armedCallbacks.onWorldClick({ tool: { ...armedTool }, x: 240, y: 288 });
  assert(
    sent.at(-1).op.op === "spawnEntity" &&
      sent.at(-1).op.owner === 2 &&
      sent.at(-1).op.kind === KIND.CITY_CENTRE &&
      sent.at(-1).op.x === 240 &&
      sent.at(-1).op.y === 288 &&
      sent.at(-1).op.completed === true,
    "LabPanel building spawn tool sends clicked world coordinates through LabClient",
  );
  panel.fields.get("building-spawn-faction").value = "ekat";
  panel.fields.get("building-spawn-faction").listeners.change();
  assert(panel.buildingSpawnPalette.kind === KIND.ZAMOK, "LabPanel faction selection updates the building palette deterministically");
  buttonByText("Remove tool").listeners.click();
  assert(armedTool?.kind === "removeSelectableUnits", "LabPanel arms a remove setup tool for map clicks and drags");
  assert(
    armedTool?.keepArmedOnWorldClick === true &&
      armedTool?.consumeBoxSelection === true &&
      armedTool?.keepArmedOnBoxSelection === true,
    "LabPanel remove tool stays armed and consumes box selections",
  );
  assert(textWithin(root).includes("Armed: Remove entities"), "LabPanel shows the armed remove tool state");
  const removeClickPromise = armedCallbacks.onWorldClick({ tool: { ...armedTool }, entityIds: [41], x: 300, y: 320 });
  assert(
    sent.at(-1).op.op === "deleteEntity" &&
      sent.at(-1).op.entityId === 41,
    "LabPanel remove click deletes the hit selectable entity",
  );
  resolveLastLabResult({ outcome: { entityId: 41 } });
  await removeClickPromise;
  assert(textWithin(root).includes("Deleted 1 entity."), "LabPanel summarizes remove click deletes");
  assert(match.clientIntent.activeLabTool?.id === armedTool.id, "LabPanel remove tool stays armed after a click delete");
  const removeBoxPromise = armedCallbacks.onBoxSelection({ tool: { ...armedTool }, entityIds: [42, 43] });
  assert(sent.at(-1).op.op === "deleteEntity" && sent.at(-1).op.entityId === 42, "LabPanel remove drag deletes the first boxed entity");
  resolveLastLabResult({ outcome: { entityId: 42 } });
  await Promise.resolve();
  assert(sent.at(-1).op.op === "deleteEntity" && sent.at(-1).op.entityId === 43, "LabPanel remove drag deletes all boxed entities");
  resolveLastLabResult({ outcome: { entityId: 43 } });
  await removeBoxPromise;
  assert(textWithin(root).includes("Deleted 2 entities."), "LabPanel summarizes remove drag deletes");
  assert(match.clientIntent.activeLabTool?.id === armedTool.id, "LabPanel remove tool stays armed after a drag delete");
  assert(!buttonByText("Delete"), "LabPanel does not expose a duplicate selected-delete button");
  selectedEntities = [
    { id: 31, owner: 1, kind: KIND.RIFLEMAN },
    { id: 32, owner: 2, kind: KIND.RIFLEMAN },
  ];
  panel.render();
  assert(
    !buttonByText("Move to point") && !buttonByText("Set owner"),
    "LabPanel keeps selected move and owner controls removed even when entities are selected",
  );
  playerButtonById(1).listeners.click();
  panel.fields.get("resource-steel").value = "900";
  panel.fields.get("resource-oil").value = "300";
  buttonByText("Set resources").listeners.click();
  assert(
    sent.at(-1).op.op === "setPlayerResources" &&
      sent.at(-1).op.playerId === 1 &&
      sent.at(-1).op.steel === 900 &&
      sent.at(-1).op.oil === 300,
    "LabPanel resource fields normalize player state edits through the shared target player",
  );
  resolveLastLabResult({ outcome: { playerId: 1, steel: 900, oil: 300 } });
  assert(
    panel.fields.get("lab-player").value === "1" &&
      playerButtonById(1)?.dataset.selected === "true" &&
      panel.fields.get("resource-steel").value === "900" &&
      panel.fields.get("resource-oil").value === "300",
    "LabPanel preserves resource form values after set-resources results re-render the panel",
  );
  const giveAllPromise = buttonByText("Give All").listeners.click();
  assert(
    sent.at(-1).op.op === "setPlayerResources" &&
      sent.at(-1).op.playerId === 1 &&
      sent.at(-1).op.steel === 99999 &&
      sent.at(-1).op.oil === 99999,
    "LabPanel Give All starts by giving player one maximum lab resources",
  );
  resolveLastLabResult({ outcome: { playerId: 1, steel: 99999, oil: 99999 } });
  await Promise.resolve();
  assert(
    sent.at(-1).op.op === "setPlayerResources" &&
      sent.at(-1).op.playerId === 2 &&
      sent.at(-1).op.steel === 99999 &&
      sent.at(-1).op.oil === 99999,
    "LabPanel Give All sends maximum lab resources to every player",
  );
  resolveLastLabResult({ outcome: { playerId: 2, steel: 99999, oil: 99999 } });
  await giveAllPromise;
  assert(
    textWithin(root).includes("Gave 2 players 99999 steel and 99999 oil."),
    "LabPanel Give All summarizes the all-player resource grant",
  );
  playerButtonById(2).listeners.click();
  panel.fields.get("research-upgrade").value = UPGRADE.TANK_UNLOCK;
  buttonByText("Set research").listeners.click();
  assert(
    sent.at(-1).op.op === "setCompletedResearch" &&
      sent.at(-1).op.playerId === 2 &&
      sent.at(-1).op.upgrade === UPGRADE.TANK_UNLOCK &&
      sent.at(-1).op.completed === true,
    "LabPanel research edits use the shared target player and complete upgrades",
  );
  resolveLastLabResult({ outcome: { playerId: 2, upgrade: UPGRADE.TANK_UNLOCK, completed: true } });
  assert(
    panel.fields.get("lab-player").value === "2" &&
      playerButtonById(2)?.dataset.selected === "true" &&
      panel.fields.get("resource-steel").value === "900" &&
      panel.fields.get("resource-oil").value === "300" &&
      panel.fields.get("research-upgrade").value === UPGRADE.TANK_UNLOCK,
    "LabPanel preserves resource and research form values after set-research results re-render the panel",
  );
  panel.fields.get("scenario-name").value = "saved setup";
  void labClient.exportScenario(panel.value("scenario-name"));
  assert(sent.at(-1).op.op === "exportScenario" && sent.at(-1).op.name === "saved setup", "LabPanel scenario name feeds export requests");
  panel.fields.get("scenario-json").value = JSON.stringify({
    schemaVersion: 1,
    kind: "labScenario",
    name: "saved setup",
    metadata: { exportedTick: 0, lab: { vision: labVision.fullWorld() } },
  });
  void panel.importScenario();
  assert(sent.at(-1).op.op === "importScenario" && sent.at(-1).op.scenario.name === "saved setup", "LabPanel imports pasted scenario JSON");
  panel.destroy();
  labClient.destroy();
  assert(cancelledToolReason === "panelDestroy", "LabPanel cancels an active lab tool on teardown");
  assert(root.children.every((child) => child.removed === true), "LabPanel destroy removes both DOM roots");
});

await withFakeDocument(async () => {
  const root = document.createElement("div");
  const el = document.createElement("aside");
  root.appendChild(el);
  const storage = fakeStorage();
  const windowListeners = new Map();
  const windowObj = {
    innerWidth: 1000,
    innerHeight: 800,
    localStorage: storage,
    addEventListener(type, handler) {
      windowListeners.set(type, handler);
    },
    removeEventListener(type, handler) {
      if (windowListeners.get(type) === handler) windowListeners.delete(type);
    },
  };
  const chrome = new LabPanelWindowChrome(el, {
    windowObj,
    storage,
    storageKey: "test.lab.panel.window",
  });
  const header = chrome.renderHeader({ kicker: "Lab", title: "sandbox" });
  const resizeHandle = chrome.renderResizeHandle();
  el.append(header, resizeHandle);

  const dragHandle = header.children[0];
  const actions = header.children[1];
  const collapseButton = actions.children[0];
  assert(actions.children.length === 1, "LabPanelWindowChrome omits the reset button from panel headers");
  dragHandle.listeners.pointerdown({
    button: 0,
    pointerId: 7,
    clientX: 900,
    clientY: 90,
    preventDefault() {},
    stopPropagation() {},
  });
  windowListeners.get("pointermove")({
    pointerId: 7,
    clientX: 840,
    clientY: 126,
    preventDefault() {},
  });
  assert(el.style.left === "608px" && el.style.top === "94px", "LabPanelWindowChrome drags the panel by pointer delta");
  windowListeners.get("pointerup")({ pointerId: 7 });
  assert(storage.values.has("test.lab.panel.window"), "LabPanelWindowChrome persists moved panel geometry");

  resizeHandle.listeners.keydown({
    key: "ArrowRight",
    shiftKey: true,
    preventDefault() {},
  });
  assert(el.style.width === "392px", "LabPanelWindowChrome keyboard resize increases width by the large step");
  dragHandle.listeners.keydown({
    key: "ArrowLeft",
    preventDefault() {},
  });
  assert(el.style.left === "572px", "LabPanelWindowChrome keyboard move nudges the clamped panel");

  collapseButton.listeners.click();
  assert(
    el.dataset.collapsed === "true" &&
      collapseButton.textContent === "▸" &&
      JSON.parse(storage.values.get("test.lab.panel.window")).collapsed === true,
    "LabPanelWindowChrome persists collapsed panel state",
  );

  dragHandle.listeners.keydown({
    key: "Home",
    preventDefault() {},
  });
  assert(el.dataset.windowed === "false", "LabPanelWindowChrome reset returns to the stylesheet layout");
  assert(el.dataset.collapsed === "false", "LabPanelWindowChrome reset expands the panel");
  assert(!storage.values.has("test.lab.panel.window"), "LabPanelWindowChrome reset clears stored geometry");
  chrome.destroy();
  assert(!windowListeners.has("resize"), "LabPanelWindowChrome removes global listeners on destroy");
});

// ---------------------------------------------------------------------------
