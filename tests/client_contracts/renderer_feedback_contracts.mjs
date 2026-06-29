// tests/client_contracts/renderer_feedback_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert, assertApprox } from "./assertions.mjs";
import { COLORS } from "../../client/src/config.js";
import {
  ABILITY,
  ABILITY_OBJECT_KIND,
  KIND,
  LAB_ROLE,
  ORDER_STAGE,
  SETUP,
  STATE,
} from "../../client/src/protocol.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { buildRendererFeedbackView } from "../../client/src/renderer/feedback_view_model.js";
import { _drawSelectionAndHp } from "../../client/src/renderer/entities.js";
import {
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawAntiTankGunSetupPreview,
  _drawCommandFeedback,
  _drawDebugPathOverlay,
  _drawMortarImpacts,
  _drawMortarShells,
  _drawOrderPlan,
  _drawPlacement,
  _drawRallyPoints,
  _drawResourceMiningPreview,
} from "../../client/src/renderer/feedback.js";
import { _drawSelectedUnitRanges } from "../../client/src/renderer/unit_ranges.js";

import { RecordingGraphics } from "./pixi_fakes.mjs";

function polygonCenter(points) {
  let x = 0;
  let y = 0;
  const count = points.length / 2;
  for (let i = 0; i < points.length; i += 2) {
    x += points[i];
    y += points[i + 1];
  }
  return { x: x / count, y: y / count };
}

{
  let selectedReads = 0;
  let commandFeedbackNow = 0;
  const selected = [{
    id: 7,
    owner: 1,
    kind: KIND.ANTI_TANK_GUN,
    x: 128,
    y: 128,
    facing: 0,
    setupState: SETUP.DEPLOYED,
  }];
  const mortarImpact = {
    x: 192,
    y: 208,
    radiusTiles: 3,
    seed: 91,
    createdAt: performance.now(),
  };
  const feedbackState = {
    playerId: 1,
    map: {
      tileSize: 32,
      resources: [{ id: 200, kind: KIND.STEEL, x: 80, y: 112, remaining: 900 }],
    },
    abilityObjects: [{
      id: 9,
      owner: 1,
      kind: ABILITY_OBJECT_KIND.RETURN_MARKER,
      ability: ABILITY.EKAT_TELEPORT,
      x: 220,
      y: 240,
    }],
    smokes: [{ id: 1, x: 64, y: 80, radiusTiles: 2 }],
    selectedEntities() {
      selectedReads += 1;
      return selected;
    },
    liveSmokeCanisters() { return []; },
    liveMortarLaunches() { return []; },
    liveMortarShells() { return []; },
    liveMortarTargets() { return []; },
    liveMortarImpacts() { return [mortarImpact]; },
    liveArtilleryTargets() { return []; },
    liveArtilleryLaunches() { return []; },
    liveArtilleryImpacts() { return []; },
    liveMuzzleFlashes() { return []; },
    isOwnOwner(owner) {
      return owner === 1;
    },
    isAllyOwner() {
      return false;
    },
  };
  const feedbackIntent = {
    placement: { building: KIND.CITY_CENTRE, tileX: 2, tileY: 3, valid: true },
    antiTankGunSetupPreview: {
      mouseX: 180,
      mouseY: 128,
      guns: [{ kind: KIND.ANTI_TANK_GUN, x: 128, y: 128 }],
    },
    abilityTargetPreview: {
      ability: ABILITY.SMOKE,
      mouseX: 180,
      mouseY: 128,
      carriers: [{ kind: KIND.SCOUT_CAR, x: 128, y: 128 }],
      rangePx: 96,
      radiusPx: 24,
      hoverInRange: true,
    },
    resourceMiningPreview: {
      resourceId: 200,
      resourceX: 80,
      resourceY: 112,
      ccId: 3,
      ccX: 220,
      ccY: 220,
      inRange: false,
    },
    liveCommandFeedback(now) {
      commandFeedbackNow = now;
      return [{ kind: "move", x: 96, y: 128, append: true, createdAt: now - 100 }];
    },
  };
  const feedbackView = buildRendererFeedbackView(feedbackState, {
    clientIntent: feedbackIntent,
    entities: selected,
    now: 1500,
  });

  assert(feedbackView.playerId === 1, "feedback view exposes player id");
  assert(feedbackView.placement?.building === KIND.CITY_CENTRE, "feedback view exposes placement shape");
  assert(feedbackView.commandFeedback.length === 1, "feedback view exposes live command feedback");
  assert(commandFeedbackNow === 1500, "feedback view samples live feedback at the requested frame time");
  assert(feedbackView.liveCommandFeedback(999) === feedbackView.commandFeedback, "feedback view returns stable command feedback for the frame");
  assert(feedbackView.selectedEntities() === selected, "feedback view exposes stable selected entities for the frame");
  assert(feedbackView.showUnitRangesEnabled, "feedback view exposes unit range preference as on by default");
  assert(!feedbackView.showSelectedFieldOfFireEnabled, "feedback view leaves selected field-of-fire inspection off outside lab");
  assert(selectedReads === 1, "feedback view snapshots selected entities once per frame");
  assert(feedbackView.entityById(7) === selected[0], "feedback view exposes renderer entity lookup");
  assert(feedbackView.abilityTargetPreview?.ability === ABILITY.SMOKE, "feedback view exposes ability target preview");
  assert(feedbackView.resourceMiningPreview?.resourceId === 200, "feedback view exposes resource mining preview");
  assert(feedbackView.abilityObjects.length === 1, "feedback view exposes ability objects");

  const placementGfx = new RecordingGraphics();
  const feedbackGfx = new RecordingGraphics();
  const abilityObjectGfx = new RecordingGraphics();
  const renderer = {
    _placementGfx: placementGfx,
    _feedbackGfx: feedbackGfx,
    _abilityObjectGfx: abilityObjectGfx,
    _lineProjectileTrails: new Map(),
    _map: { tileSize: 32 },
  };
  _drawPlacement.call(renderer, feedbackView, null);
  _drawCommandFeedback.call(renderer, feedbackView);
  _drawAntiTankGunSetupPreview.call(renderer, feedbackView);
  _drawAbilityTargetPreview.call(renderer, feedbackView);
  _drawAbilityObjects.call(renderer, feedbackView);
  _drawResourceMiningPreview.call(renderer, feedbackView);
  _drawMortarImpacts.call(renderer, feedbackView);

  assert(placementGfx.calls.some((call) => call[0] === "drawRoundedRect"), "renderer feedback reads placement through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "drawCircle"), "renderer feedback reads command/preview state through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "lineTo"), "renderer feedback reads resource mining preview through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "drawPolygon"), "renderer feedback draws live mortar impacts without missing helper references");
  assert(abilityObjectGfx.calls.some((call) => call[0] === "drawCircle"), "renderer feedback reads ability objects through the feedback view");
}

{
  const now = performance.now();
  const selected = [
    {
      id: 71,
      owner: 2,
      kind: KIND.MORTAR_TEAM,
      x: 96,
      y: 96,
      hp: 70,
      maxHp: 70,
      state: STATE.MOVE,
      orderPlan: [{ kind: ORDER_STAGE.MOVE, x: 140, y: 128 }],
      debugPath: {
        waypoints: [{ x: 110, y: 100 }, { x: 128, y: 116 }],
        goal: { x: 140, y: 128 },
      },
    },
    {
      id: 72,
      owner: 2,
      kind: KIND.ARTILLERY,
      x: 160,
      y: 96,
      hp: 200,
      maxHp: 200,
      setupState: SETUP.DEPLOYED,
      setupFacing: 0,
    },
    {
      id: 73,
      owner: 2,
      kind: KIND.BARRACKS,
      x: 128,
      y: 160,
      hp: 500,
      maxHp: 500,
      rallyPlan: [{ kind: "move", x: 190, y: 180 }],
    },
    {
      id: 74,
      owner: 2,
      kind: KIND.TANK,
      x: 224,
      y: 96,
      hp: 300,
      maxHp: 300,
      weaponRangeTiles: 7,
    },
  ];
  const feedbackState = {
    playerId: 1,
    spectator: true,
    players: [
      { id: 1, teamId: 1 },
      { id: 2, teamId: 2 },
    ],
    controlPolicy: createLabControlPolicy({ metadata: { role: LAB_ROLE.OPERATOR } }),
    showUnitRangesEnabled: true,
    debugPathOverlaysEnabled: true,
    showAllDebugPathOverlays: false,
    selectedEntities() {
      return selected;
    },
    liveSmokeCanisters() { return []; },
    liveMortarLaunches() { return []; },
    liveMortarShells() { return []; },
    liveMortarTargets() { return []; },
    liveMortarImpacts() { return []; },
    liveArtilleryTargets() { return []; },
    liveArtilleryLaunches() { return []; },
    liveArtilleryImpacts() { return []; },
    liveMuzzleFlashes() { return []; },
    isOwnOwner(owner) {
      return owner === 1;
    },
    isAllyOwner() {
      return false;
    },
  };
  const feedbackView = buildRendererFeedbackView(feedbackState, {
    selectedEntities: selected,
    clientIntent: {
      liveCommandFeedback() {
        return [
          { kind: "move", x: 210, y: 220, append: false, createdAt: now - 10, ownerId: 2 },
          { kind: "move", x: 240, y: 220, append: false, createdAt: now - 10, ownerId: 1 },
        ];
      },
    },
    now,
  });
  assert(feedbackView.issueAsOwnerId === 2, "lab renderer feedback resolves the selected issue-as owner");
  assert(feedbackView.feedbackOwnerId === 2, "lab renderer feedback resolves the current feedback owner");
  assert(feedbackView.isFeedbackOwner(2), "lab renderer feedback treats selected P2 as feedback owner");
  assert(!feedbackView.isFeedbackOwner(1), "lab renderer feedback does not treat raw playerId as feedback owner");
  assert(feedbackView.showSelectedFieldOfFireEnabled, "lab renderer feedback enables selected support-weapon field-of-fire inspection");

  const commandGfx = new RecordingGraphics();
  _drawCommandFeedback.call({ _feedbackGfx: commandGfx }, feedbackView);
  assert(
    commandGfx.calls.filter((call) => call[0] === "drawCircle").length === 1,
    "lab command feedback filters markers to the controlled owner",
  );

  const rangeGfx = new RecordingGraphics();
  _drawSelectedUnitRanges.call({ _feedbackGfx: rangeGfx, _map: { tileSize: 32 } }, feedbackView);
  assert(rangeGfx.calls.some((call) => call[0] === "lineTo"), "lab P2 selected units draw range rings");
  assert(rangeGfx.calls.some((call) => call[0] === "arc"), "lab P2 deployed support weapons draw field-of-fire ranges");
  assert(
    rangeGfx.calls.some((call) => call[0] === "lineStyle" && call[1] === 1 && call[3] === 0.68),
    "selected unit range rings draw at doubled opacity",
  );
  assert(
    rangeGfx.calls.some((call) => call[0] === "lineStyle" && call[1] === 1.5 && call[3] === 0.36),
    "selected support-weapon field-of-fire outlines draw at doubled opacity",
  );
  assert(
    rangeGfx.calls.some((call) => call[0] === "beginFill" && call[2] === 0.07),
    "selected support-weapon field-of-fire fill draws at doubled opacity",
  );
  assert(
    rangeGfx.calls.some((call) => call[0] === "lineTo" && call[1] > 446 && Math.abs(call[2] - 96) < 8),
    "unit range overlay can read per-entity dynamic range fields",
  );

  const minRangeGfx = new RecordingGraphics();
  _drawSelectedUnitRanges.call(
    { _feedbackGfx: minRangeGfx, _map: { tileSize: 32 } },
    {
      playerId: 2,
      showUnitRangesEnabled: true,
      selectedEntities: () => [{
        id: 75,
        owner: 2,
        kind: KIND.TANK,
        x: 224,
        y: 96,
        weaponRangePx: 240,
        weaponMinRangePx: 96,
      }],
    },
  );
  assert(
    minRangeGfx.calls.some((call) => call[0] === "lineStyle" && call[1] === 1 && call[3] === 0.56),
    "selected unit minimum-range rings draw at doubled opacity",
  );

  const workerRangeGfx = new RecordingGraphics();
  _drawSelectedUnitRanges.call(
    { _feedbackGfx: workerRangeGfx, _map: { tileSize: 32 } },
    {
      playerId: 2,
      showUnitRangesEnabled: true,
      selectedEntities: () => [
        { id: 76, owner: 2, kind: KIND.WORKER, x: 256, y: 96 },
        {
          id: 77,
          owner: 2,
          kind: KIND.WORKER,
          x: 288,
          y: 96,
          weaponRangePx: 192,
          weaponMinRangePx: 64,
          weaponArcRad: Math.PI / 3,
          weaponFacing: 0,
        },
      ],
    },
  );
  assert(workerRangeGfx.calls.length === 0, "selected workers never draw unit range indicators");

  const disabledRangeGfx = new RecordingGraphics();
  _drawSelectedUnitRanges.call(
    { _feedbackGfx: disabledRangeGfx, _map: { tileSize: 32 } },
    { ...feedbackView, showUnitRangesEnabled: false },
  );
  assert(
    disabledRangeGfx.calls.some((call) => call[0] === "arc"),
    "lab selected deployed support weapons keep field-of-fire cones when unit ranges are disabled",
  );
  assert(
    !disabledRangeGfx.calls.some((call) => call[0] === "lineTo" && call[1] > 446 && Math.abs(call[2] - 96) < 8),
    "lab selected non-support unit ranges stay hidden when unit ranges are disabled",
  );

  const setupGfx = new RecordingGraphics();
  _drawAntiTankGunSetupPreview.call(
    { _feedbackGfx: setupGfx, _map: { tileSize: 32 } },
    {
      ...feedbackView,
      antiTankGunSetupPreview: {
        mouseX: 200,
        mouseY: 96,
        guns: [{ kind: KIND.ANTI_TANK_GUN, x: 160, y: 96 }],
      },
    },
  );
  assert(setupGfx.calls.some((call) => call[0] === "arc"), "lab P2 support weapons still draw setup-preview wedges");

  const orderGfx = new RecordingGraphics();
  _drawOrderPlan.call({ _feedbackGfx: orderGfx }, feedbackView);
  assert(orderGfx.calls.some((call) => call[0] === "lineTo"), "lab P2 selected units draw accepted order plans");

  const debugGfx = new RecordingGraphics();
  _drawDebugPathOverlay.call({ _feedbackGfx: debugGfx }, feedbackView);
  assert(debugGfx.calls.some((call) => call[0] === "drawCircle"), "lab P2 selected units draw debug path overlays");

  const rallyGfx = new RecordingGraphics();
  _drawRallyPoints.call({ _feedbackGfx: rallyGfx }, feedbackView);
  assert(rallyGfx.calls.some((call) => call[0] === "drawPolygon"), "lab P2 selected producers draw rally lines");

  const ringGfx = new RecordingGraphics();
  _drawSelectionAndHp.call(
    {
      _slot() {
        return ringGfx;
      },
      _ringRadius() {
        return { rx: 12, ry: 8, cy: 0 };
      },
      _hpBar() {},
    },
    selected[0],
    new Set([selected[0].id]),
    feedbackState,
  );
  assert(
    ringGfx.calls.some((call) => call[0] === "lineStyle" && call[2] === COLORS.selectOwn),
    "lab P2 selected entities use own selection-ring color",
  );
}

{
  const priorNow = performance.now;
  const fixedNow = 2000;
  const shell = {
    fromX: 100,
    fromY: 50,
    toX: 300,
    toY: 250,
    durationMs: 2000,
    createdAt: 1500,
  };
  const feedbackGfx = new RecordingGraphics();

  performance.now = () => fixedNow;
  try {
    _drawMortarShells.call({ _feedbackGfx: feedbackGfx }, {
      liveMortarShells(now) {
        assertApprox(now, fixedNow, 0.001, "mortar shell renderer samples current frame time");
        return [shell];
      },
    });
  } finally {
    performance.now = priorNow;
  }

  const expectedX = 150;
  const expectedY = 100;
  const shadow = feedbackGfx.calls.find((call) => call[0] === "drawEllipse");
  const body = feedbackGfx.calls.find((call) => call[0] === "drawPolygon");
  assert(shadow, "mortar shell renderer draws the shell shadow");
  assert(body, "mortar shell renderer draws the shell body");
  const bodyCenter = polygonCenter(body[1]);
  assertApprox(shadow[1], expectedX, 0.001, "mortar shell shadow advances linearly on x");
  assertApprox(shadow[2], expectedY, 0.001, "mortar shell shadow advances linearly on y");
  assertApprox(bodyCenter.x, expectedX, 0.001, "mortar shell body center advances linearly on x");
  assertApprox(bodyCenter.y, expectedY, 0.001, "mortar shell body center advances linearly on y");
}

{
  const placementGfx = new RecordingGraphics();
  _drawPlacement.call({
    _placementGfx: placementGfx,
    _map: { tileSize: 32 },
  }, {
    placement: {
      building: KIND.TANK_TRAP,
      tileX: 0,
      tileY: 0,
      valid: true,
      lineSites: [
        { tileX: 0, tileY: 0, valid: true },
        { tileX: 2, tileY: 0, valid: false },
        { tileX: 4, tileY: 0, valid: true },
      ],
    },
  }, null);
  const rects = placementGfx.calls.filter((call) => call[0] === "drawRoundedRect");
  const fills = placementGfx.calls.filter((call) => call[0] === "beginFill");
  assert(rects.length === 3, "Tank Trap line placement preview draws each candidate site");
  assert(
    fills.some((call) => call[1] === COLORS.placeOk) && fills.some((call) => call[1] === COLORS.placeBad),
    "Tank Trap line placement preview distinguishes valid and invalid sites",
  );
}
