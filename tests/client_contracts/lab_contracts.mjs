// tests/client_contracts/lab_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import fs from "node:fs";

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
  LAB_CHECKPOINT_SCENARIO,
  LAB_REPLAY,
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
  LabCatalogScreen,
  normalizeLabScenarioEntry,
} from "../../client/src/lab_catalog.js";
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
import {
  researchableUpgradesForFaction,
  STATS,
} from "../../client/src/config.js";

import { textWithin } from "./dom_text.mjs";

// Lab client and panel
// ---------------------------------------------------------------------------
{
  assert(
    LAB_REPLAY.SCHEMA === "rts.labReplay" &&
      LAB_REPLAY.KIND === "labReplay" &&
      LAB_REPLAY.SCHEMA_VERSION === 1 &&
      LAB_REPLAY.MAX_OPERATIONS === 50000,
    "lab replay artifact constants are available through the stable protocol mirror",
  );
}

{
  const styles = fs.readFileSync(new URL("../../client/styles.css", import.meta.url), "utf8");
  const labMobileStart = styles.indexOf("@media (max-width: 720px) {", styles.indexOf(".lab-result[data-state=\"ok\"]"));
  const labMobileEnd = styles.indexOf("/* --- Settings menu --- */", labMobileStart);
  const labMobileStyles = labMobileStart >= 0 && labMobileEnd > labMobileStart
    ? styles.slice(labMobileStart, labMobileEnd)
    : "";
  assert(
    /\.lab-options-window\s*\{[^}]*top:\s*232px\b/s.test(labMobileStyles),
    "lab mobile CSS moves the Options window below expanded room-time controls",
  );
  assert(
    /\.lab-tools-window\s*\{[^}]*top:\s*326px\b/s.test(labMobileStyles),
    "lab mobile CSS keeps the Tools window header on a separate tap row",
  );
  assert(
    /\.lab-panel-collapse\s*\{[^}]*min-height:\s*36px\b[^}]*touch-action:\s*manipulation\b/s.test(labMobileStyles),
    "lab mobile CSS gives its floating-panel collapse button a touch-friendly hit target",
  );
}

{
  const normalized = normalizeLabScenarioEntry({
    id: "lategame",
    title: "Lategame Arsenal",
    description: "Full tech setup",
    tags: ["two-player", "lategame"],
    map: "Chokes",
    playerCount: 2,
    scenario: { kind: LAB_CHECKPOINT_SCENARIO.KIND },
  });
  assert(normalized.id === "lategame", "lab catalog entry keeps stable setup id");
  assert(normalized.playerCount === 2, "lab catalog entry keeps bounded player count metadata");
  assert(!("scenario" in normalized), "lab catalog entry normalization keeps full setup JSON out of the listing model");
}

{
  const allKriegsiaResearch = [
    ...researchableUpgradesForFaction(DEFAULT_FACTION_ID, KIND.TRAINING_CENTRE),
    ...researchableUpgradesForFaction(DEFAULT_FACTION_ID, KIND.RESEARCH_COMPLEX),
  ];
  for (const filename of ["lategame.json", "render-preview.json"]) {
    const scenario = JSON.parse(
      fs.readFileSync(new URL(`../../server/assets/lab-scenarios/${filename}`, import.meta.url), "utf8"),
    );
    assert(scenario.kind === LAB_CHECKPOINT_SCENARIO.KIND, `${filename} uses checkpoint-backed lab setup shape`);
    if (filename === "render-preview.json") {
      assertDeepEqual(
        scenario.metadata?.lab?.initialCamera,
        { centerX: 2016, centerY: 2784 },
        "render-preview lab setup starts the camera on the formation south of the lake",
      );
    }
    const checkpoint = JSON.parse(scenario.checkpointPayload);
    assert(checkpoint.players?.length === 2, `${filename} remains a two-player bundled lab setup`);
    for (const player of checkpoint.players) {
      const completedResearch = [...(player.upgrades || [])].sort();
      const expectedResearch = allKriegsiaResearch.map((upgrade) =>
        upgrade.replace(/(^|_)([a-z])/g, (_match, _prefix, letter) => letter.toUpperCase()),
      );
      assertDeepEqual(
        completedResearch,
        [...expectedResearch].sort(),
        `${filename} grants all current Kriegsia research to player ${player.id}`,
      );
    }
  }
}

await withFakeDocument(async () => {
  const root = document.createElement("section");
  const starts = [];
  let requestedUrl = "";
  const screen = new LabCatalogScreen({
    root,
    initialRoom: "sandbox",
    fetchImpl: async (url, options) => {
      requestedUrl = url;
      assert(options.cache === "no-store", "LabCatalogScreen requests the catalog without cache");
      return {
        ok: true,
        async json() {
          return [
            {
              id: "lategame",
              title: "Lategame Arsenal",
              description: "Full tech setup",
              tags: ["two-player", "lategame"],
              map: "Chokes",
              playerCount: 2,
              filename: "lategame.json",
            },
          ];
        },
      };
    },
    onStart: (launch) => starts.push(launch),
  });
  screen.mount();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();

  assert(requestedUrl === "/api/lab-scenarios", "LabCatalogScreen loads the server catalog endpoint");
  assert(textWithin(root).includes("Blank Lab"), "LabCatalogScreen renders a blank lab start row");
  assert(textWithin(root).includes("Lategame Arsenal"), "LabCatalogScreen renders bundled setup metadata");
  screen.setConnected(true);
  const scenarioButton = findFakes(
    root,
    (el) => el.tagName === "BUTTON" && el.textContent === "Start setup",
  )[0];
  scenarioButton.listeners.click();
  assert(
    starts[0]?.room === "sandbox" &&
      starts[0]?.map === "Chokes" &&
      starts[0]?.scenario === "lategame",
    "LabCatalogScreen reports the selected room, map, and catalog setup id",
  );
});

await withFakeDocument(() => {
  const root = document.createElement("section");
  const starts = [];
  const screen = new LabCatalogScreen({
    root,
    initialRoom: "sandbox",
    onStart: (launch) => starts.push(launch),
  });
  screen.setConnected(true);
  const blankButton = findFakes(
    root,
    (el) => el.tagName === "BUTTON" && el.textContent === "Start blank",
  )[0];
  blankButton.listeners.click();
  assert(
    starts[0]?.room === "sandbox" &&
      starts[0]?.map === "1v1" &&
      starts[0]?.scenario === "blank",
    "LabCatalogScreen starts blank labs on the current default 1v1 map",
  );
});

await withFakeDocument(() => {
  const root = document.createElement("section");
  let backCount = 0;
  const screen = new LabCatalogScreen({
    root,
    initialRoom: "sandbox",
    onBack: () => { backCount += 1; },
  });
  screen.setConnected(true);
  const backButton = findFakes(
    root,
    (el) => el.tagName === "BUTTON" && el.textContent === "Back",
  )[0];
  assert(!!backButton, "LabCatalogScreen renders the optional desktop Back button");
  backButton.listeners.click();
  assert(backCount === 1, "LabCatalogScreen sends Back to its app-owned navigation callback");
  assert(textWithin(root).includes("Returning to main screen"),
    "LabCatalogScreen reports the desktop transition while navigation begins");
});

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
    room: "__lab__:sandbox:map=Chokes",
    operatorId: 1,
    role: LAB_ROLE.OPERATOR,
    vision: labVision.all(),
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
  assert(sent.at(-1).op.op === "exportScenario" && sent.at(-1).op.name === "saved setup", "LabClient sends setup export requests through the compatibility op");
  void labClient.importScenario({
    schemaVersion: LAB_CHECKPOINT_SCENARIO.SCHEMA_VERSION,
    kind: LAB_CHECKPOINT_SCENARIO.KIND,
    name: "saved setup",
  });
  assert(
    sent.at(-1).op.op === "importScenario" &&
      sent.at(-1).op.scenario.kind === LAB_CHECKPOINT_SCENARIO.KIND &&
      sent.at(-1).op.scenario.name === "saved setup",
    "LabClient sends checkpoint setup import requests through the compatibility op",
  );
  const beforeLegacyImportCount = sent.length;
  const legacyImport = await labClient.importScenario({
    schemaVersion: 1,
    kind: "labScenario",
    name: "old setup",
  });
  assert(
    sent.length === beforeLegacyImportCount &&
      !legacyImport.ok &&
      legacyImport.op === "importScenario" &&
      legacyImport.error.includes("Legacy labScenario JSON is no longer supported"),
    "LabClient rejects legacy setup imports locally with an explicit compatibility error",
  );
  void labClient.validateScenario({
    slug: "saved-setup",
    name: "Saved setup",
    title: "Saved setup",
    description: "Ready to review.",
    tags: ["test"],
  });
  assert(
    sent.at(-1).op.op === "validateScenario" &&
      sent.at(-1).op.metadata.slug === "saved-setup" &&
      sent.at(-1).op.metadata.tags[0] === "test",
    "LabClient sends setup authoring validation requests with metadata",
  );
  labClient.resetScenario();
  assert(sent.at(-1).t === "seekRoomTimeTo" && sent.at(-1).tick === 0, "LabClient resets setups by seeking lab room time to tick zero");
  assert(labVisionLabel(labVision.all()) === "Full", "labVisionLabel formats all-team vision");
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
    playerId: 1,
    resources: { steel: 75, oil: 0, supplyUsed: 1, supplyCap: 10 },
    upgrades: [UPGRADE.TANK_UNLOCK],
    playerResources: [
      { id: 1, steel: 999, oil: 999, supplyUsed: 1, supplyCap: 99 },
      {
        id: 2,
        steel: 125,
        oil: 50,
        supplyUsed: 7,
        supplyCap: 12,
        upgrades: [UPGRADE.ANTI_TANK_GUN_UNLOCK],
      },
    ],
    players: [
      { id: 1, teamId: 1, factionId: DEFAULT_FACTION_ID },
      { id: 2, teamId: 2, factionId: "ekat" },
      { id: 3, teamId: 2, factionId: DEFAULT_FACTION_ID },
    ],
    selectedEntities() {
      return [{ id: 11, owner: 2, kind: KIND.RIFLEMAN }];
    },
  };
  assert(policy.canControlOwner(2, state), "lab control policy controls a single selected owner");
  assert(!policy.canControlOwner(1, state), "lab control policy rejects non-selected owners");
  assert(policy.feedbackOwner(state) === 2, "lab control policy exposes the selected feedback owner");
  assert(policy.feedbackOwnerForSelection(state.selectedEntities()) === 2, "lab control policy resolves feedback owner from a selection read model");
  assert(policy.isFeedbackOwner(2, state), "lab control policy identifies the feedback owner");
  assert(!policy.isFeedbackOwner(1, state), "lab control policy does not treat the raw local player id as feedback owner");
  assert(policy.commandOwner(state) === 2, "lab control policy exposes the selected command owner");
  assert(policy.commandResources(state).steel === 125, "lab command resources resolve from the selected owner row");
  assert(policy.commandFactionId(state) === "ekat", "lab command faction resolves from the selected owner");
  assertDeepEqual(
    policy.commandUpgrades(state),
    [UPGRADE.ANTI_TANK_GUN_UNLOCK],
    "lab command upgrades resolve from per-owner upgrade rows when available",
  );
  assert(policy.isCommandOwner(2, state), "lab command owner matching is exact-owner based");
  assert(policy.isCommandEnemyOwner(1, state), "lab command enemies are classified relative to the selected owner");
  assert(policy.isCommandAllyOwner(3, state), "lab command allies are classified relative to the selected owner");
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
  assert(
    createLabControlPolicy({ metadata: { role: LAB_ROLE.READ_ONLY } }).feedbackOwner(state) === null,
    "read-only lab viewers do not get a feedback owner",
  );
  assert(!createDefaultControlPolicy().canUseCommandSurface({ spectator: true }), "default spectators cannot use the command surface");
  assert(createDefaultControlPolicy().isFeedbackOwner(1, { playerId: 1 }), "default control policy keeps local-player feedback ownership");
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
  assert(
    !labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.SCOUT_PLANE),
    "LabPanel spawn palette excludes the ability-only Kriegsia Scout Plane",
  );
  assert(
    STATS[KIND.BARRACKS].trains.length === 3 &&
      labSpawnUnitKindsForFaction(DEFAULT_FACTION_ID).includes(KIND.PANZERFAUST),
    "LabPanel exposes the standalone Panzerfaust unit from the Barracks catalog",
  );
  assertDeepEqual(
    labSpawnUnitKindsForFaction("ekat"),
    [KIND.EKAT, KIND.GOLEM],
    "LabPanel spawn palette filters Ekat to Ekat catalog units",
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
      room: "__lab__:sandbox:map=Chokes",
      operatorId: 1,
      role,
      vision: labVision.all(),
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
    map: { name: "Chokes" },
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
      playerId: 1,
      playerResources: [
        { id: 1, steel: 500, oil: 200 },
        { id: 2, steel: 700, oil: 100 },
      ],
      upgrades: [UPGRADE.ANTI_TANK_GUN_UNLOCK],
      playerUpgrades: [
        { id: 2, upgrades: [] },
      ],
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
    updateLabToolPayload(payload) {
      armedTool = this.clientIntent.updateLabToolPayload(payload);
      if (armedTool) panel?.applyLabToolChange({ type: "updated", tool: armedTool });
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
    room: "__lab__:sandbox:map=Chokes",
    operatorId: 1,
    role: LAB_ROLE.OPERATOR,
    vision: labVision.all(),
    godModePlayers: [1],
    dirty: false,
    operationCount: 0,
  });
  panel = new LabPanel({
    root,
    labClient,
    launch: { publicRoom: "sandbox", map: "Chokes" },
    startPayload: {
      map: { name: "Chokes" },
      players: [
        { id: 1, teamId: 1, color: "#2255aa" },
        { id: 2, teamId: 2, color: "#bb4422" },
      ],
    },
    match,
    controlPolicy: createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } }),
    commandLimitSettings: {
      ignoreCommandLimitsEnabled() {
        return ignoreCommandLimits;
      },
      setIgnoreCommandLimits(enabled) {
        ignoreCommandLimits = !!enabled;
      },
    },
  });
  const buttonByText = (label) => findFakes(root, (el) => el.tagName === "BUTTON" && el.textContent === label)[0];
  const buttonByUpgrade = (upgrade) => findFakes(root, (el) => (
    el.tagName === "BUTTON" && el.dataset?.upgrade === upgrade
  ))[0];
  const playerButtonById = (id) => findFakes(root, (el) => (
    el.tagName === "BUTTON" && el.dataset?.playerId === String(id)
  ))[0];
  const playerButtons = () => findFakes(root, (el) => (
    el.tagName === "BUTTON" && String(el.className).includes("lab-player-btn")
  ));
  const playerButtonGroup = () => findFakes(root, (el) => (
    String(el.className).split(/\s+/).includes("lab-player-buttons")
  ))[0];
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
  assertDeepEqual(
    panel.resourcesForTargetPlayer(),
    { steel: 500, oil: 200 },
    "LabPanel resolves target resources from the explicitly tagged player row",
  );
  match.state.playerResources = [{ id: 2, steel: 900, oil: 800 }];
  assertDeepEqual(
    panel.resourcesForTargetPlayer(),
    { steel: 0, oil: 0 },
    "LabPanel does not borrow another visible player's resource row for the target player",
  );
  match.state.playerResources = [
    { id: 1, steel: 500, oil: 200 },
    { id: 2, steel: 700, oil: 100 },
  ];
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
    !textWithin(sectionByClass("lab-options")).includes("Vision") &&
      textWithin(sectionByClass("lab-options")).includes("Unlimited commands") &&
      textWithin(sectionByClass("lab-options")).includes("Checkpoint Setup") &&
      textWithin(sectionByClass("lab-options")).includes("Lab Replay") &&
      !textWithin(sectionByClass("lab-options")).includes("Unit Spawn") &&
      !textWithin(sectionByClass("lab-options")).includes("Player State"),
    "LabPanel leaves observer vision to the shared room controls and groups Lab-only controls in Options",
  );
  assert(
    textWithin(sectionByClass("lab-tools")).includes("Unit Spawn") &&
      textWithin(sectionByClass("lab-tools")).includes("Building Spawn") &&
      textWithin(sectionByClass("lab-tools")).includes("Player State") &&
      textWithin(sectionByClass("lab-tools")).includes("Remove entities") &&
      !textWithin(sectionByClass("lab-tools")).includes("Unlimited commands") &&
      !textWithin(sectionByClass("lab-tools")).includes("Checkpoint Setup") &&
      !textWithin(sectionByClass("lab-tools")).includes("Lab Replay"),
    "LabPanel groups placement tools in the Tools section",
  );
  assert(
    buttonByText("Export setup JSON") &&
      buttonByText("Import setup JSON") &&
      buttonByText("Reset setup"),
    "LabPanel labels setup checkpoint JSON controls separately from replay controls",
  );
  assert(
    buttonByText("Save lab replay")?.disabled &&
      buttonByText("Open lab replay")?.disabled &&
      buttonByText("Save lab replay")?.dataset.labReplayAction === "save" &&
      buttonByText("Open lab replay")?.dataset.labReplayAction === "open",
    "LabPanel renders distinct lab replay save/open affordances outside the setup JSON wire controls",
  );
  assert(
    findFakes(sectionByClass("lab-tools"), (el) => el.tagName === "H3" && el.textContent === "Tools").length === 0,
    "LabPanel does not repeat the Tools title inside the Tools window",
  );
  assert(
    !textWithin(sectionByClass("lab-tools")).includes("Target Player") &&
      !findFakes(sectionByClass("lab-tools"), (el) => String(el.className).split(/\s+/).includes("lab-player-label")).length &&
      sectionByClass("lab-tools").children.includes(playerButtonGroup()),
    "LabPanel renders target player buttons directly without the old labels or fieldset",
  );
  assert(
    textWithin(sectionByClass("lab-tools").children.at(-1)).includes("Player State"),
    "LabPanel places Player State at the bottom of the Tools section",
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
      !panel.fields.has("research-completed") &&
      !panel.fields.has("research-upgrade") &&
      !buttonByText("Set research"),
    "LabPanel does not expose advanced spawn or completion toggles",
  );
  assert(
    buttonByText("AT Guns")?.dataset.researched === "true" &&
      buttonByText("AT Guns")?.dataset.available === "true" &&
      buttonByText("AT Guns")?.["aria-pressed"] === "true",
    "LabPanel renders completed research as a depressed available button",
  );
  assert(
    buttonByUpgrade(UPGRADE.ARTILLERY_UNLOCK)?.textContent === "Artillery" &&
      buttonByUpgrade(UPGRADE.ARTILLERY_UNLOCK)?.dataset.researched === "false" &&
      buttonByUpgrade(UPGRADE.ARTILLERY_UNLOCK)?.dataset.available === "false" &&
      buttonByUpgrade(UPGRADE.ARTILLERY_UNLOCK)?.["aria-pressed"] === "false",
    "LabPanel renders incomplete Artillery as an up unavailable button",
  );
  assert(
    buttonByText("Tank Production")?.dataset.researched === "false" &&
      buttonByText("Tank Production")?.dataset.available === "false" &&
      buttonByText("Tank Production")?.["aria-pressed"] === "false",
    "LabPanel renders incomplete research as an up unavailable button",
  );
  assert(!buttonByText("Apply teams"), "LabPanel omits the arbitrary team-union controls");
  assert(
    !buttonByText("Full") && !buttonByText("Team 2"),
    "LabPanel does not duplicate the shared observer-view selector",
  );
  playerButtonById(2).listeners.click();
  assert(panel.fields.get("lab-player").value === "2", "LabPanel target player buttons update shared target state");
  assert(
    spawnPanel("units")?.dataset.targetPlayerId === "2" &&
      spawnPanel("units")?.dataset.targetColor === "#bb4422" &&
      spawnPanel("buildings")?.dataset.targetPlayerId === "2" &&
      spawnPanel("buildings")?.dataset.targetColor === "#bb4422",
    "LabPanel retints spawn panels when the target player changes",
  );
  assert(
    panel.fields.get("resource-steel").value === "700" &&
      panel.fields.get("resource-oil").value === "100",
    "LabPanel refreshes resource fields from the newly selected player",
  );
  assert(
    buttonByText("AT Guns")?.dataset.researched === "false" &&
      buttonByText("AT Guns")?.["aria-pressed"] === "false",
    "LabPanel refreshes completed research for the newly selected player",
  );
  assert(
    panel.fields.get("player-god-mode").checked === false,
    "LabPanel refreshes god mode state for the newly selected player",
  );
  playerButtonById(1).listeners.click();
  assert(
    panel.fields.get("resource-steel").value === "500" &&
      panel.fields.get("resource-oil").value === "200" &&
      buttonByText("AT Guns")?.dataset.researched === "true" &&
      panel.fields.get("player-god-mode").checked === true,
    "LabPanel restores every player-specific control when switching back",
  );
  panel.armSpawnPaletteTool(KIND.RIFLEMAN);
  assert(
    armedTool?.kind === "spawnEntity" && armedTool.payload.owner === 1,
    "LabPanel initially arms the spawn tool for the selected target player",
  );
  const spawnToolBeforeRetarget = armedTool;
  const spawnCallbacksBeforeRetarget = armedCallbacks;
  match.clientIntent.updateLabToolPreview({ toolId: armedTool.id, x: 96, y: 128 });
  playerButtonById(2).listeners.click();
  assert(
    armedTool === spawnToolBeforeRetarget &&
      armedCallbacks === spawnCallbacksBeforeRetarget &&
      armedTool?.kind === "spawnEntity" &&
      armedTool.payload.owner === 2 &&
      match.clientIntent.activeLabTool?.payload?.owner === 2 &&
      match.clientIntent.labToolPreview?.payload?.owner === 2 &&
      match.clientIntent.labToolPreview?.x === 96 &&
      match.clientIntent.labToolPreview?.y === 128,
    "LabPanel retargets the existing spawn tool and cursor preview without interrupting it",
  );
  assert(armedTool?.kind === "spawnEntity", "LabPanel unit palette arms the spawn lab tool through Match");
  assert(armedTool?.keepArmedOnWorldClick === true, "LabPanel unit palette keeps the spawn tool armed across world clicks");
  assert(armedTool?.paintOnDrag === true, "LabPanel unit palette enables persistent drag painting");
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
  assert(armedTool?.paintOnDrag === true, "LabPanel building palette enables persistent drag painting");
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
  buttonByText("Remove entities").listeners.click();
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
  const setResearchPromise = buttonByText("Tank Production").listeners.click();
  assert(
    sent.at(-1).op.op === "setCompletedResearch" &&
      sent.at(-1).op.playerId === 2 &&
      sent.at(-1).op.upgrade === UPGRADE.TANK_UNLOCK &&
      sent.at(-1).op.completed === true,
    "LabPanel research buttons use the shared target player and complete upgrades",
  );
  resolveLastLabResult({ outcome: { playerId: 2, upgrade: UPGRADE.TANK_UNLOCK, completed: true } });
  await setResearchPromise;
  assert(
    panel.fields.get("lab-player").value === "2" &&
      playerButtonById(2)?.dataset.selected === "true" &&
      panel.fields.get("resource-steel").value === "700" &&
      panel.fields.get("resource-oil").value === "100" &&
      buttonByText("Tank Production")?.dataset.researched === "true" &&
      buttonByText("Tank Production")?.["aria-pressed"] === "true",
    "LabPanel keeps the selected player's resources and depresses completed research after result re-rendering",
  );
  const clearResearchPromise = buttonByText("Tank Production").listeners.click();
  assert(
    sent.at(-1).op.op === "setCompletedResearch" &&
      sent.at(-1).op.playerId === 2 &&
      sent.at(-1).op.upgrade === UPGRADE.TANK_UNLOCK &&
      sent.at(-1).op.completed === false,
    "LabPanel researched buttons toggle completed research off",
  );
  resolveLastLabResult({ outcome: { playerId: 2, upgrade: UPGRADE.TANK_UNLOCK, completed: false } });
  await clearResearchPromise;
  assert(
    buttonByText("Tank Production")?.dataset.researched === "false" &&
      buttonByText("Tank Production")?.["aria-pressed"] === "false",
    "LabPanel raises research buttons again after clearing completed research",
  );
  panel.fields.get("scenario-title").value = "Saved Setup";
  panel.fields.get("scenario-title").listeners.input();
  assert(panel.fields.get("scenario-slug").value === "saved-setup", "LabPanel generates an authoring slug from the setup title");
  panel.fields.get("scenario-slug").value = "manual_slug";
  panel.fields.get("scenario-slug").listeners.input();
  panel.fields.get("scenario-title").value = "Changed Setup";
  panel.fields.get("scenario-title").listeners.input();
  assert(panel.fields.get("scenario-slug").value === "manual_slug", "LabPanel preserves manually edited authoring slugs");
  panel.fields.get("scenario-name").value = "saved setup";
  panel.fields.get("scenario-title").value = "Saved Setup";
  panel.fields.get("scenario-slug").value = "saved-setup";
  panel.fields.get("scenario-description").value = "A catalog-ready saved setup.";
  panel.fields.get("scenario-tags").value = "two-player, test";
  const validatePromise = buttonByText("Validate setup").listeners.click();
  assert(
    sent.at(-1).op.op === "validateScenario" &&
      sent.at(-1).op.metadata.slug === "saved-setup" &&
      sent.at(-1).op.metadata.description === "A catalog-ready saved setup." &&
      sent.at(-1).op.metadata.tags.length === 2,
    "LabPanel validates setup authoring metadata through LabClient",
  );
  resolveLastLabResult({
    outcome: {
      summary: "Setup ready.",
      preview: {
        slug: "saved-setup",
        scenarioPath: "server/assets/lab-scenarios/saved-setup.json",
        manifestEntry: { id: "saved-setup", title: "Saved Setup" },
        scenarioJson: "{\n  \"kind\": \"labCheckpointScenario\"\n}\n",
      },
    },
  });
  await validatePromise;
  assert(
    panel.fields.get("scenario-json").value.includes("\"kind\": \"labCheckpointScenario\"") &&
      textWithin(root).includes("server/assets/lab-scenarios/saved-setup.json"),
    "LabPanel shows the dry-run setup JSON and target file preview",
  );
  panel.fields.get("scenario-slug").value = "bad slug";
  const beforeInvalidValidate = sent.length;
  await buttonByText("Validate setup").listeners.click();
  assert(
    sent.length === beforeInvalidValidate &&
      textWithin(root).includes("Slug must be"),
    "LabPanel blocks invalid authoring metadata before sending validation",
  );
  panel.fields.get("scenario-name").value = "saved setup";
  void labClient.exportScenario(panel.value("scenario-name"));
  assert(sent.at(-1).op.op === "exportScenario" && sent.at(-1).op.name === "saved setup", "LabPanel setup name feeds export requests");
  panel.fields.get("scenario-json").value = JSON.stringify({
    schemaVersion: LAB_CHECKPOINT_SCENARIO.SCHEMA_VERSION,
    kind: LAB_CHECKPOINT_SCENARIO.KIND,
    name: "saved setup",
    metadata: { exportedTick: 0, lab: { vision: labVision.all() } },
  });
  void panel.importScenario();
  assert(sent.at(-1).op.op === "importScenario" && sent.at(-1).op.scenario.name === "saved setup", "LabPanel imports pasted checkpoint setup JSON");
  buttonByText("Reset setup").listeners.click();
  assert(sent.at(-1).t === "seekRoomTimeTo" && sent.at(-1).tick === 0, "LabPanel reset setup seeks the lab timeline to the setup start");
  assert(textWithin(root).includes("Setup reset requested."), "LabPanel surfaces reset setup requests locally");
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
    panelLabel: "map editor",
  });
  const header = chrome.renderHeader({ kicker: "Lab", title: "sandbox" });
  const resizeHandle = chrome.renderResizeHandle();
  el.append(header, resizeHandle);

  const dragHandle = header.children[0];
  const actions = header.children[1];
  const collapseButton = actions.children[0];
  assert(actions.children.length === 1, "LabPanelWindowChrome omits the reset button from panel headers");
  assert(
    dragHandle["aria-label"] === "Move map editor panel" &&
      resizeHandle["aria-label"] === "Resize map editor panel",
    "LabPanelWindowChrome applies the panel label to move and resize controls",
  );
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
      collapseButton.textContent === "Expand" &&
      JSON.parse(storage.values.get("test.lab.panel.window")).collapsed === true,
    "LabPanelWindowChrome persists collapsed panel state",
  );
  collapseButton.listeners.pointerdown({
    button: 0,
    isPrimary: true,
    pointerId: 8,
    pointerType: "touch",
  });
  collapseButton.listeners.pointerup({
    pointerId: 8,
    pointerType: "touch",
    preventDefault() {},
    stopPropagation() {},
  });
  assert(
    el.dataset.collapsed === "false" &&
      collapseButton.textContent === "Collapse" &&
      JSON.parse(storage.values.get("test.lab.panel.window")).collapsed === false,
    "LabPanelWindowChrome touch release toggles collapse without waiting for a synthesized click",
  );
  collapseButton.listeners.click({
    pointerType: "touch",
    detail: 1,
    preventDefault() {},
    stopPropagation() {},
  });
  assert(el.dataset.collapsed === "false", "LabPanelWindowChrome ignores the click synthesized after touch collapse");
  collapseButton.listeners.pointerdown({
    button: 0,
    isPrimary: true,
    pointerId: 10,
    pointerType: "touch",
  });
  collapseButton.listeners.pointerleave({
    pointerId: 10,
    pointerType: "touch",
  });
  collapseButton.listeners.pointerup({
    pointerId: 10,
    pointerType: "touch",
    preventDefault() {},
    stopPropagation() {},
  });
  assert(el.dataset.collapsed === "false", "LabPanelWindowChrome cancels touch collapse when the pointer leaves");

  dragHandle.listeners.keydown({
    key: "Home",
    preventDefault() {},
  });
  assert(el.dataset.windowed === "false", "LabPanelWindowChrome reset returns to the stylesheet layout");
  assert(el.dataset.collapsed === "false", "LabPanelWindowChrome reset expands the panel");
  assert(collapseButton.textContent === "Collapse", "LabPanelWindowChrome labels the expanded panel action");
  assert(!storage.values.has("test.lab.panel.window"), "LabPanelWindowChrome reset clears stored geometry");
  chrome.destroy();
  assert(!windowListeners.has("resize"), "LabPanelWindowChrome removes global listeners on destroy");
});

await withFakeDocument(async () => {
  const el = document.createElement("aside");
  const storage = fakeStorage({
    "test.lab.panel.mobile": JSON.stringify({
      schemaVersion: 1,
      collapsed: true,
      left: 620,
      top: 58,
      width: 320,
      height: 432,
    }),
  });
  const windowObj = {
    innerWidth: 390,
    innerHeight: 844,
    localStorage: storage,
    addEventListener() {},
    removeEventListener() {},
  };
  const chrome = new LabPanelWindowChrome(el, {
    windowObj,
    storage,
    storageKey: "test.lab.panel.mobile",
  });
  const header = chrome.renderHeader({ kicker: "Options", collapseLabel: "options panel" });
  const collapseButton = header.children[1].children[0];

  assert(el.dataset.windowed === "false", "LabPanelWindowChrome ignores saved desktop geometry on mobile widths");
  assert(!el.style.left && !el.style.top, "LabPanelWindowChrome leaves mobile panels on stylesheet positions");
  assert(el.dataset.collapsed === "true" && collapseButton.textContent === "Expand",
    "LabPanelWindowChrome preserves saved collapsed state without restoring overlapping geometry");
  collapseButton.listeners.pointerdown({
    button: 0,
    isPrimary: true,
    pointerId: 9,
    pointerType: "touch",
  });
  collapseButton.listeners.pointerup({
    pointerId: 9,
    pointerType: "touch",
    preventDefault() {},
    stopPropagation() {},
  });
  assert(
    JSON.parse(storage.values.get("test.lab.panel.mobile")).left === 620,
    "LabPanelWindowChrome mobile touch collapse toggles preserve saved desktop geometry",
  );
  windowObj.innerWidth = 1000;
  chrome.constrainToViewport();
  assert(
    el.dataset.windowed === "true" &&
      el.style.left === "620px" &&
      el.style.top === "58px",
    "LabPanelWindowChrome restores saved desktop geometry after leaving mobile layout",
  );
  chrome.destroy();
});

// ---------------------------------------------------------------------------
