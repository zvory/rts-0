// tests/client_contracts/observer_analysis_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import {
  fakeStorage,
  findFakes,
  withFakeOverlayDocument,
} from "./fakes.mjs";
import {
  KIND,
  UPGRADE,
} from "../../client/src/protocol.js";
import {
  AiDiagnosticsPanel,
  createAiDiagnosticsPanelPreferences,
  normalizeMapAnalysisSummary,
  shouldMountAiDiagnosticsPanel,
} from "../../client/src/ai_diagnostics_panel.js";
import {
  OBSERVER_ANALYSIS_TABS,
  ObserverAnalysisOverlay,
  calculateViewportArmyValue,
  createObserverAnalysisOverlayPreferences,
  shouldMountObserverAnalysisOverlay,
} from "../../client/src/observer_analysis_overlay.js";
import { createRoomCapabilities } from "../../client/src/room_capabilities.js";

import { textWithin } from "./dom_text.mjs";

// Observer analysis overlay
// ---------------------------------------------------------------------------
{
  const players = [
    { id: 1, name: "Red", color: "#cc1111" },
    { id: 2, name: "Blue", color: "#1133bb" },
  ];
  const calculatorRows = calculateViewportArmyValue({
    players,
    cameraBounds: { x: 0, y: 0, width: 100, height: 100 },
    stats: {
      [KIND.RIFLEMAN]: { size: 9, cost: { steel: 50, oil: 0 } },
      [KIND.TANK]: { size: 18, cost: { steel: 300, oil: 150 } },
      [KIND.BARRACKS]: { size: 24, cost: { steel: 150, oil: 0 } },
    },
    entities: [
      { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 },
      { id: 2, owner: 1, kind: KIND.TANK, x: 150, y: 20 },
      { id: 3, owner: 2, kind: KIND.TANK, x: 99, y: 50 },
      { id: 4, owner: 2, kind: KIND.BARRACKS, x: 20, y: 20 },
      { id: 5, owner: 1, kind: KIND.RIFLEMAN, x: 40, y: 40, shotReveal: true },
      { id: 6, owner: 1, kind: KIND.RIFLEMAN, x: 60, y: 40, visionOnly: true },
      { id: 7, owner: 1, kind: KIND.STEEL, x: 25, y: 25 },
      { id: 8, owner: 2, kind: KIND.MACHINE_GUNNER, x: 30, y: 30 },
    ],
  });
  const redValue = calculatorRows.find((row) => row.owner === 1);
  const blueValue = calculatorRows.find((row) => row.owner === 2);
  assert(redValue.steel === 100 && redValue.oil === 0, "army value counts visible units and visionOnly units");
  assert(blueValue.steel === 300 && blueValue.oil === 150, "army value groups costs by owner");
  assert(calculatorRows.length === 2, "army value keeps known player rows only for known owners");

  const emptyRows = calculateViewportArmyValue({
    players,
    cameraBounds: { x: 500, y: 500, width: 100, height: 100 },
    entities: [{ id: 9, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 }],
  });
  assert(
    emptyRows.every((row) => row.steel === 0 && row.oil === 0),
    "army value reports zero for players with no visible on-screen units",
  );

  const storage = fakeStorage();
  const prefs = createObserverAnalysisOverlayPreferences(storage);
  prefs.selectedTab = "units-lost";
  prefs.visible = false;
  prefs.collapsed = true;

  const restored = createObserverAnalysisOverlayPreferences(storage);
  assert(restored.selectedTab === "units-lost", "observer analysis selected tab persists");
  assert(restored.visible === false, "observer analysis visible state persists");
  assert(restored.collapsed === true, "observer analysis collapsed state persists");

  restored.selectedTab = "not-a-tab";
  assert(
    restored.selectedTab === OBSERVER_ANALYSIS_TABS[0].id,
    "observer analysis rejects unknown tab ids",
  );

  const legacyStorage = fakeStorage();
  legacyStorage.setItem("rts.replayAnalysisOverlay", JSON.stringify({
    selectedTab: "production",
    visible: false,
    collapsed: true,
  }));
  const migrated = createObserverAnalysisOverlayPreferences(legacyStorage);
  assert(migrated.selectedTab === "production", "observer analysis reads legacy replay preference key");
  migrated.visible = true;
  assert(
    legacyStorage.getItem("rts.observerAnalysisOverlay") !== null,
    "observer analysis writes the observer preference key after reading legacy preferences",
  );

  assert(
    shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { replay: {}, spectator: true, diagnostics: { observerAnalysis: true } },
      }),
    }),
    "observer analysis mounts when the start payload advertises it for replay viewers",
  );
  assert(
    shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { spectator: true, diagnostics: { observerAnalysis: true } },
      }),
    }),
    "observer analysis mounts when the start payload advertises it for live spectators",
  );
  assert(
    !shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { spectator: false, diagnostics: { observerAnalysis: false } },
      }),
    }),
    "observer analysis stays hidden without diagnostic metadata",
  );
  assert(
    !shouldMountObserverAnalysisOverlay({
      capabilities: createRoomCapabilities({
        startPayload: { replay: {}, spectator: true },
      }),
    }),
    "observer analysis does not mount from replay identity alone",
  );

  const aiStorage = fakeStorage();
  const aiPrefs = createAiDiagnosticsPanelPreferences(aiStorage);
  aiPrefs.visible = false;
  aiPrefs.collapsed = true;
  aiPrefs.setMapLayerVisible("chokes", false);
  const restoredAiPrefs = createAiDiagnosticsPanelPreferences(aiStorage);
  assert(restoredAiPrefs.visible === false, "AI diagnostics visible state persists");
  assert(restoredAiPrefs.collapsed === true, "AI diagnostics collapsed state persists");
  assert(
    restoredAiPrefs.mapLayerVisibility(["chokes"]).chokes === false,
    "AI diagnostics map layer visibility persists",
  );
  const staleAiStorage = fakeStorage();
  staleAiStorage.setItem("rts.aiDiagnosticsPanel", JSON.stringify({
    mapLayers: { regions: false, voronoi: true, chokes: false, futureRoutes: false },
  }));
  const staleAiPrefs = createAiDiagnosticsPanelPreferences(staleAiStorage);
  const staleMapLayers = staleAiPrefs.mapLayerVisibility(["chokes", "futureRoutes"]);
  assert(
    staleMapLayers.chokes === false
      && staleMapLayers.futureRoutes === false
      && !Object.hasOwn(staleMapLayers, "regions")
      && !Object.hasOwn(staleMapLayers, "voronoi"),
    "AI diagnostics drops retired map-analysis layers without losing current layer preferences",
  );
  staleAiPrefs.setMapLayerVisible("regions", true);
  const retiredLayerIds = staleAiPrefs.mapLayerVisibility(["regions", "voronoi"]);
  assert(
    !Object.hasOwn(retiredLayerIds, "regions") && !Object.hasOwn(retiredLayerIds, "voronoi"),
    "AI diagnostics rejects retired map-analysis layer ids from current payloads and direct toggles",
  );
  const mapSummary = normalizeMapAnalysisSummary({
    mapWidth: 126,
    mapHeight: 126,
    tileSize: 32,
    layers: [
      { id: "chokes", label: "Chokes", primitives: [{ kind: "tileRect" }, { kind: "marker" }] },
      { id: "bases", label: "Bases", primitives: [{ kind: "marker" }] },
      { id: "resources", label: "Resources", primitives: [{ kind: "marker" }] },
    ],
  });
  assert(
    mapSummary.primitives === 4
      && mapSummary.layers.some((layer) => layer.id === "labels" && layer.primitives === 4),
    "AI diagnostics summarizes map-analysis layers and synthetic labels toggle",
  );
  assert(
    shouldMountAiDiagnosticsPanel({
      capabilities: createRoomCapabilities({
        startPayload: { spectator: true, diagnostics: { observerAnalysis: true } },
      }),
    }),
    "AI diagnostics panel mounts from observer-analysis diagnostic capability",
  );
  assert(
    !shouldMountAiDiagnosticsPanel({
      capabilities: createRoomCapabilities({
        startPayload: { spectator: true, diagnostics: { observerAnalysis: false } },
      }),
    }),
    "AI diagnostics panel stays hidden without observer-analysis diagnostic capability",
  );

  withFakeOverlayDocument(({ FakeElement }) => {
    const root = new FakeElement("section");
    restored.selectedTab = "army-value";
    restored.visible = true;
    restored.collapsed = false;
    const overlay = new ObserverAnalysisOverlay({
      root,
      preferences: restored,
      getPlayers: () => players,
      getCameraBounds: () => ({ x: 0, y: 0, width: 100, height: 100 }),
      getEntities: () => [{ id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 }],
    });
    assert(root.children.length === 1, "observer analysis overlay mounts generated DOM");
    const overlayRoot = root.children[0];
    assert(root.querySelector(".replay-army-value-row"), "observer analysis renders army value rows");
    assert(
      findFakes(root, (el) => el.classList.contains("replay-army-value-steel"))
        .some((cell) => cell.querySelector(".steel"))
        && findFakes(root, (el) => el.classList.contains("replay-army-value-oil"))
          .some((cell) => cell.querySelector(".oil")),
      "observer analysis army value uses shared steel and oil icons",
    );
    const analysisBody = root.querySelector("#replay-analysis-body");
    const stableArmyValueRenders = analysisBody.replaceChildrenCount;
    overlay.update({
      authoritativeEntities: [{ id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 }],
    });
    assert(
      analysisBody.replaceChildrenCount === stableArmyValueRenders,
      "observer analysis skips unchanged army-value body DOM replacement",
    );
    overlay.update({
      authoritativeEntities: [
        { id: 1, owner: 1, kind: KIND.RIFLEMAN, x: 20, y: 20 },
        { id: 2, owner: 2, kind: KIND.TANK, x: 20, y: 20 },
      ],
    });
    assert(
      analysisBody.replaceChildrenCount === stableArmyValueRenders + 1,
      "observer analysis replaces army-value body when visible values change",
    );

    const unitsTab = root.querySelector(".replay-analysis-tab");
    assert(unitsTab, "observer analysis renders tab buttons");
    overlayRoot.listeners.click?.({ target: unitsTab, preventDefault() {}, stopPropagation() {} });
    assert(
      restored.selectedTab === unitsTab.dataset.tabId,
      "observer analysis tab clicks update shared preferences",
    );

    const hide = root.querySelector(".replay-analysis-hide");
    overlayRoot.listeners.click?.({ target: hide, preventDefault() {}, stopPropagation() {} });
    assert(restored.visible === false, "observer analysis hide action updates shared preferences");

    const show = root.querySelector(".replay-analysis-show");
    overlayRoot.listeners.click?.({ target: show, preventDefault() {}, stopPropagation() {} });
    assert(restored.visible === true, "observer analysis show action updates shared preferences");
    assert(restored.collapsed === false, "observer analysis show expands the panel");

    restored.selectedTab = "production";
    overlay.render();
    assert(
      textWithin(root).includes("Waiting for observer analysis"),
      "production tab shows a loading state before analysis arrives",
    );
    overlay.applyObserverAnalysis({ tick: 1, players: [{ id: 1, units: [], production: [] }, { id: 2, units: [], production: [] }] });
    assert(
      textWithin(root).includes("No active production"),
      "production tab handles empty production cleanly",
    );

    overlay.applyObserverAnalysis({
      tick: 12,
      players: [
        {
          id: 1,
          units: [],
          production: [
            {
              buildingId: 11,
              buildingKind: KIND.BARRACKS,
              itemKind: KIND.MACHINE_GUNNER,
              itemType: "unit",
              progress: 0.42,
              queueDepth: 2,
            },
          ],
        },
        {
          id: 2,
          units: [],
          production: [
            {
              buildingId: 21,
              buildingKind: KIND.RESEARCH_COMPLEX,
              itemKind: UPGRADE.TANK_UNLOCK,
              itemType: "upgrade",
              progress: 0.75,
              queueDepth: 1,
            },
          ],
        },
      ],
    });
    const productionText = textWithin(root);
    assert(productionText.includes("Red"), "production tab groups rows by first player");
    assert(productionText.includes("Blue"), "production tab groups rows by second player");
    assert(
      productionText.includes("Machine Gunner at Barracks") && productionText.includes("42") && productionText.includes("Q 2"),
      "production tab renders active unit production with progress and queue depth",
    );
    assert(
      productionText.includes("Tank Production at R&D Complex") && productionText.includes("75"),
      "production tab renders active research with mirrored upgrade labels",
    );

    restored.selectedTab = "units";
    overlay.render();
    overlay.applyObserverAnalysis({
      tick: 20,
      players: [
        {
          id: 1,
          units: [
            { kind: KIND.RIFLEMAN, count: 3, steelValue: 150, oilValue: 0 },
            { kind: KIND.TANK, count: 1, steelValue: 300, oilValue: 150 },
          ],
          production: [],
        },
        {
          id: 2,
          units: [{ kind: KIND.WORKER, count: 2, steelValue: 100, oilValue: 0 }],
          production: [],
        },
      ],
    });
    const unitText = textWithin(root);
    assert(unitText.includes("Total") && unitText.includes("4") && unitText.includes("450") && unitText.includes("150"),
      "units tab includes totals for the current player group");
    assert(unitText.includes("Rifleman") && unitText.includes("Tank"), "units tab renders per-kind unit rows");
    assert(unitText.includes("Blue") && unitText.includes("Engineer"), "units tab renders multiple players");
    assert(
      findFakes(root, (el) => el.classList.contains("replay-units-steel"))
        .some((cell) => cell.querySelector(".steel"))
        && findFakes(root, (el) => el.classList.contains("replay-units-oil"))
          .some((cell) => cell.querySelector(".oil")),
      "units tab uses shared steel and oil icons for resource values",
    );

    overlay.applyObserverAnalysis({
      tick: 5,
      players: [{ id: 1, units: [{ kind: KIND.WORKER, count: 1, steelValue: 50, oilValue: 0 }], production: [] }],
    });
    const replacedUnitText = textWithin(root);
    assert(replacedUnitText.includes("Engineer"), "units tab renders replacement analysis after seek");
    assert(!replacedUnitText.includes("Tank"), "units tab drops stale rows after seek replacement");

    restored.selectedTab = "units-lost";
    overlay.render();
    assert(
      textWithin(root).includes("No units lost"),
      "units lost tab handles analysis with no loss rows cleanly",
    );
    overlay.applyObserverAnalysis({
      tick: 30,
      players: [
        {
          id: 1,
          units: [],
          production: [],
          unitsLost: [
            { kind: KIND.RIFLEMAN, count: 2, steelValue: 100, oilValue: 0 },
            { kind: KIND.TANK, count: 1, steelValue: 300, oilValue: 150 },
          ],
          resourcesLost: { steel: 400, oil: 150 },
        },
        {
          id: 2,
          units: [],
          production: [],
          unitsLost: [{ kind: KIND.WORKER, count: 3, steelValue: 150, oilValue: 0 }],
          resourcesLost: { steel: 150, oil: 0 },
        },
      ],
    });
    const unitsLostText = textWithin(root);
    assert(
      unitsLostText.includes("Total lost") && unitsLostText.includes("3") && unitsLostText.includes("400") && unitsLostText.includes("150"),
      "units lost tab includes per-player totals with steel and oil value lost",
    );
    assert(
      unitsLostText.includes("Rifleman") && unitsLostText.includes("Tank") && unitsLostText.includes("Engineer"),
      "units lost tab renders per-kind loss rows for multiple players",
    );

    restored.selectedTab = "resources";
    overlay.render();
    overlay.applyObserverAnalysis({
      tick: 36,
      players: [
        {
          id: 1,
          units: [],
          production: [],
          resourcesLost: { steel: 400, oil: 150 },
          resources: {
            lifetime: { steel: 120, oil: 30 },
            last5s: { steel: 40, oil: 10 },
            lastMinute: { steel: 100, oil: 25 },
          },
        },
        {
          id: 2,
          units: [],
          production: [],
          resourcesLost: { steel: 150, oil: 0 },
          resources: {
            lifetime: { steel: 55, oil: 5 },
            last5s: { steel: 0, oil: 0 },
            lastMinute: { steel: 20, oil: 5 },
          },
        },
      ],
    });
    const resourcesText = textWithin(root);
    assert(
      resourcesText.includes("Mined resources")
        && resourcesText.includes("Lifetime")
        && resourcesText.includes("Last 5s")
        && resourcesText.includes("Last 1m")
        && resourcesText.includes("Total")
        && resourcesText.includes("175")
        && resourcesText.includes("35")
        && resourcesText.includes("120")
        && resourcesText.includes("30"),
      "resources tab renders lifetime, short-window, and minute mined-resource totals",
    );
    assert(
      findFakes(root, (el) => el.classList.contains("replay-resources-steel"))
        .some((cell) => cell.querySelector(".steel"))
        && findFakes(root, (el) => el.classList.contains("replay-resources-oil"))
          .some((cell) => cell.querySelector(".oil")),
      "resources tab uses shared steel and oil icons",
    );

    restored.selectedTab = "resources-lost";
    overlay.render();
    const resourcesLostText = textWithin(root);
    assert(
      resourcesLostText.includes("Dead unit value")
        && resourcesLostText.includes("Spent steel and oil value of units that died")
        && resourcesLostText.includes("Total")
        && resourcesLostText.includes("550")
        && resourcesLostText.includes("150"),
      "resources lost tab labels the narrow observer analysis definition and totals killed unit value",
    );
    assert(
      resourcesLostText.includes("Red") && resourcesLostText.includes("Blue"),
      "resources lost tab renders per-player killed unit value",
    );

    const tabButtons = root.querySelectorAll(".replay-analysis-tab");
    const firstTab = tabButtons[0];
    overlayRoot.listeners.keydown?.({
      target: firstTab,
      key: "End",
      preventDefault() {},
      stopPropagation() {},
    });
    assert(restored.selectedTab === "resources-lost", "observer analysis keyboard End selects the last tab");
    assert(tabButtons[tabButtons.length - 1].focused === true, "observer analysis keyboard navigation focuses the selected tab");

    overlay.destroy();
    assert(root.children.length === 0, "observer analysis overlay removes generated DOM on destroy");
  });

  withFakeOverlayDocument(({ FakeElement }) => {
    const root = new FakeElement("section");
    restoredAiPrefs.visible = true;
    restoredAiPrefs.collapsed = false;
    let latestMapLayers = null;
    const panel = new AiDiagnosticsPanel({
      root,
      preferences: restoredAiPrefs,
      getPlayers: () => players,
      onMapLayerVisibilityChange: (layers) => {
        latestMapLayers = layers;
      },
    });
    assert(root.children.length === 2, "AI diagnostics panel mounts generated DOM plus a show affordance");
    assert(root.querySelector(".lab-panel-drag-handle"), "AI diagnostics panel uses the shared movable lab window titlebar");
    assert(root.querySelector(".lab-panel-resize-handle"), "AI diagnostics panel uses the shared resizable lab window handle");
    assert(latestMapLayers?.chokes === false, "AI diagnostics publishes persisted map layer visibility on mount");
    assert(
      textWithin(root).includes("Waiting for observer analysis"),
      "AI diagnostics panel shows a factual waiting state before analysis arrives",
    );

    panel.applyObserverAnalysis({
      tick: 40,
      players: [{ id: 1, units: [], production: [] }],
    });
    assert(
      textWithin(root).includes("No AI diagnostics"),
      "AI diagnostics panel handles observer analysis without AI trace rows cleanly",
    );

    const mapAnalysisPayload = {
      mapWidth: 126,
      mapHeight: 126,
      tileSize: 32,
      layers: [
        {
          id: "chokes",
          label: "Chokes",
          defaultVisible: true,
          primitives: [{ kind: "tileRect", id: "choke:0", tileX: 5, tileY: 6, tileW: 1, tileH: 4 }],
        },
        {
          id: "bases",
          label: "Bases",
          defaultVisible: true,
          primitives: [{ kind: "marker", id: "base:1", x: 64, y: 96, radius: 14 }],
        },
        {
          id: "resources",
          label: "Resources",
          defaultVisible: true,
          primitives: [{ kind: "marker", id: "resource:0", x: 96, y: 128, radius: 10 }],
        },
      ],
    };
    panel.applyObserverAnalysis({
      tick: 45,
      players: [
        {
          id: 1,
          units: [],
          production: [],
          aiDiagnostics: {
            profileId: "ai_2_1",
            traceTick: 36,
            lines: [
              "profile=ai_2_1 tick=36",
              "goal=Production status=Selected blockers=- intents=train:Rifleman",
            ],
          },
        },
        {
          id: 2,
          units: [],
          production: [],
          aiDiagnostics: {
            profileId: "ai_2_pressure",
            traceTick: 72,
            lines: [
              "goal=Harass status=Moving intents=attackMove",
            ],
          },
        },
      ],
      mapAnalysis: mapAnalysisPayload,
    });
    const aiDiagnosticsText = textWithin(root);
    assert(
      aiDiagnosticsText.includes("AI Diagnostics")
        && aiDiagnosticsText.includes("AI players")
        && aiDiagnosticsText.includes("Trace lines")
        && aiDiagnosticsText.includes("Map layers")
        && aiDiagnosticsText.includes("Map analysis")
        && aiDiagnosticsText.includes("Chokes")
        && aiDiagnosticsText.includes("2")
        && aiDiagnosticsText.includes("ai_2_1")
        && aiDiagnosticsText.includes("tick 36")
        && aiDiagnosticsText.includes("goal")
        && aiDiagnosticsText.includes("Production")
        && aiDiagnosticsText.includes("intents")
        && aiDiagnosticsText.includes("train:Rifleman"),
      "AI diagnostics panel renders status, profile, trace tick, and parsed decision fields",
    );
    const chokeToggle = findFakes(root, (el) =>
      el.classList.contains("ai-diagnostics-map-toggle")
      && el.dataset.aiMapLayer === "chokes",
    )[0];
    assert(chokeToggle?.getAttribute("aria-checked") === "false", "AI diagnostics renders persisted map toggle state");
    root.children[0].listeners.click?.({ target: chokeToggle, preventDefault() {}, stopPropagation() {} });
    assert(
      restoredAiPrefs.mapLayerVisibility(["chokes"]).chokes === true
        && latestMapLayers?.chokes === true,
      "AI diagnostics map layer toggles update preferences and renderer visibility",
    );
    const aiTabs = root.querySelectorAll(".ai-diagnostics-tab");
    assert(aiTabs.length === 2, "AI diagnostics panel renders one tab for each AI diagnostics row");
    assert(
      !textWithin(root).includes("Harass"),
      "AI diagnostics panel shows one selected AI trace at a time",
    );
    const secondTab = aiTabs[1];
    root.children[0].listeners.click?.({ target: secondTab, preventDefault() {}, stopPropagation() {} });
    assert(restoredAiPrefs.selectedPlayerId === 2, "AI diagnostics tab clicks persist the selected AI");
    assert(
      textWithin(root).includes("Harass") && !textWithin(root).includes("Production"),
      "AI diagnostics panel switches trace content when selecting another AI tab",
    );
    const firstTab = root.querySelectorAll(".ai-diagnostics-tab")[0];
    root.children[0].listeners.click?.({ target: firstTab, preventDefault() {}, stopPropagation() {} });
    const aiDiagnosticsBody = root.querySelector(".ai-diagnostics-body");
    const stableTraceRenderCount = aiDiagnosticsBody.replaceChildrenCount;
    panel.applyObserverAnalysis({
      tick: 46,
      players: [
        {
          id: 1,
          units: [],
          production: [],
          aiDiagnostics: {
            profileId: "ai_2_1",
            traceTick: 36,
            lines: [
              "profile=ai_2_1 tick=36",
              "goal=Production status=Selected blockers=- intents=train:Rifleman",
            ],
          },
        },
        {
          id: 2,
          units: [],
          production: [],
          aiDiagnostics: {
            profileId: "ai_2_pressure",
            traceTick: 72,
            lines: [
              "goal=Harass status=Moving intents=attackMove",
            ],
          },
        },
      ],
      mapAnalysis: mapAnalysisPayload,
    });
    assert(
      aiDiagnosticsBody.replaceChildrenCount === stableTraceRenderCount,
      "AI diagnostics panel skips DOM replacement for observer tick-only updates",
    );
    aiDiagnosticsBody.scrollTop = 11;
    aiDiagnosticsBody.scrollLeft = 3;
    const scrolledTabs = root.querySelector(".ai-diagnostics-tabs");
    const scrolledPlayer = root.querySelector(".ai-diagnostics-player");
    scrolledTabs.scrollLeft = 19;
    scrolledPlayer.scrollTop = 81;
    scrolledPlayer.scrollLeft = 5;
    const traceUpdateRenderCount = aiDiagnosticsBody.replaceChildrenCount;
    panel.applyObserverAnalysis({
      tick: 47,
      players: [
        {
          id: 1,
          units: [],
          production: [],
          aiDiagnostics: {
            profileId: "ai_2_1",
            traceTick: 37,
            lines: [
              "profile=ai_2_1 tick=37",
              "goal=Production status=Selected blockers=- intents=train:Rifleman",
              "queue=Rifleman count=2",
            ],
          },
        },
        {
          id: 2,
          units: [],
          production: [],
          aiDiagnostics: {
            profileId: "ai_2_pressure",
            traceTick: 72,
            lines: [
              "goal=Harass status=Moving intents=attackMove",
            ],
          },
        },
      ],
      mapAnalysis: mapAnalysisPayload,
    });
    assert(
      aiDiagnosticsBody.replaceChildrenCount === traceUpdateRenderCount + 1,
      "AI diagnostics panel replaces DOM when the selected trace changes",
    );
    assert(
      aiDiagnosticsBody.scrollTop === 11
        && aiDiagnosticsBody.scrollLeft === 3
        && root.querySelector(".ai-diagnostics-tabs").scrollLeft === 19
        && root.querySelector(".ai-diagnostics-player").scrollTop === 81
        && root.querySelector(".ai-diagnostics-player").scrollLeft === 5,
      "AI diagnostics panel preserves scroll offsets across selected trace updates",
    );

    const panelRoot = root.children[0];
    const hide = root.querySelector(".ai-diagnostics-hide");
    panelRoot.listeners.click?.({ target: hide, preventDefault() {}, stopPropagation() {} });
    assert(restoredAiPrefs.visible === false, "AI diagnostics hide action updates shared preferences");

    const show = root.querySelector(".ai-diagnostics-show");
    show.listeners.click?.({ target: show, preventDefault() {}, stopPropagation() {} });
    assert(restoredAiPrefs.visible === true, "AI diagnostics show action updates shared preferences");
    assert(restoredAiPrefs.collapsed === false, "AI diagnostics show expands the panel");

    panel.destroy();
    assert(root.children.length === 0, "AI diagnostics panel removes generated DOM on destroy");
  });
}
