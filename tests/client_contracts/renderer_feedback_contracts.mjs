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
  TERRAIN,
  WEAPON_KIND,
} from "../../client/src/protocol.js";
import { createLabControlPolicy } from "../../client/src/lab_control_policy.js";
import { buildRendererFeedbackView } from "../../client/src/renderer/feedback_view_model.js";
import { createLiveRigDefinitions } from "../../client/src/renderer/rigs/live_routing.js";
import { _drawSelectionAndHp } from "../../client/src/renderer/entities.js";
import {
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawAntiTankGunSetupPreview,
  _drawAttackTargetPreview,
  _drawCommandFeedback,
  _drawDebugPathOverlay,
  _drawMortarImpacts,
  _drawMortarShells,
  _drawMuzzleFlashes,
  _drawOrderPlan,
  _drawPlacement,
  _drawRallyPoints,
  _drawResourceMiningPreview,
} from "../../client/src/renderer/feedback.js";
import { drawLabMapDraftOverlay, drawLabToolPreview } from "../../client/src/renderer/lab_tool_preview.js";
import { _drawMissToasts } from "../../client/src/renderer/miss_toasts.js";
import {
  _drawPanzerfaustImpacts,
  _drawPanzerfaustShots,
} from "../../client/src/renderer/panzerfaust_feedback.js";
import { _drawSelectedUnitRanges } from "../../client/src/renderer/unit_ranges.js";
import { muzzleFeedbackStyle } from "../../client/src/renderer/weapon_feedback_style.js";

import { RecordingGraphics, installFakePixi } from "./pixi_fakes.mjs";

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

function nearPoint(call, point, epsilon = 0.001) {
  return Math.abs(call[1] - point.x) <= epsilon && Math.abs(call[2] - point.y) <= epsilon;
}

{
  const restorePixi = installFakePixi();
  const priorNow = performance.now;
  const fixedNow = 5000;
  performance.now = () => fixedNow;
  try {
    const target = { id: 81, owner: 2, kind: KIND.RIFLEMAN, x: 160, y: 120 };
    const context = {
      _missToastPool: new Map(),
      layers: { feedback: new PIXI.Container() },
      _ringRadius() {
        return { rx: 12, ry: 8, cy: 3 };
      },
      _recordRenderDiagnostic() {},
    };
    _drawMissToasts.call(context, {
      entityById(id) {
        return id === target.id ? target : null;
      },
      liveMissToasts(now) {
        assertApprox(now, fixedNow, 0.001, "miss toast renderer samples current frame time");
        return [{ id: 1, to: target.id, createdAt: fixedNow }];
      },
    });
    const label = context.layers.feedback.children[0];
    assert(label?.text === "Miss!", "miss toast renders the Miss! label");
    assertApprox(label.style.fontSize, 6.75, 0.001, "miss toast text is 1.5x larger than the prior tiny size");
    assertApprox(label.style.strokeThickness, 1.5, 0.001, "miss toast stroke scales with the text");
    assertApprox(label.x, target.x + 14, 0.001, "miss toast sits close to the receiving unit's right edge");
    assertApprox(label.y, target.y - 8, 0.001, "miss toast sits close to the receiving unit's top edge");
    assert(label.x > target.x, "miss toast appears to the right of the receiving unit");
    assert(label.y < target.y, "miss toast appears above the receiving unit");

    _drawMissToasts.call(context, {
      entityById() {
        return null;
      },
      liveMissToasts() {
        return [];
      },
    });
    assert(context._missToastPool.size === 0, "expired miss toast labels are destroyed");
    assert(
      context.layers.feedback.children.length === 0,
      "expired miss toast labels are detached from the feedback layer",
    );
    assert(label.destroyed === true, "expired miss toast label display objects are destroyed");
  } finally {
    performance.now = priorNow;
    restorePixi();
  }
}

{
  const rifle = muzzleFeedbackStyle(KIND.RIFLEMAN, WEAPON_KIND.RIFLEMAN_RIFLE);
  assertApprox(rifle.tracerWidth, 0.45, 0.0001, "rifleman tracer width is 30% of the old infantry line");
  assertApprox(rifle.tailWidth, 0.3, 0.0001, "rifleman tracer tail width is 30% of the old infantry tail");

  const mg = muzzleFeedbackStyle(KIND.MACHINE_GUNNER, WEAPON_KIND.MACHINE_GUNNER_MG);
  assertApprox(mg.tracerWidth, 0.75, 0.0001, "machine gunner tracer width is 50% of the old infantry line");
  assertApprox(mg.tailWidth, 0.5, 0.0001, "machine gunner tracer tail width is 50% of the old infantry tail");

  const scoutCar = muzzleFeedbackStyle(KIND.SCOUT_CAR, WEAPON_KIND.SCOUT_CAR_MG);
  assertApprox(scoutCar.tracerWidth, 0.75, 0.0001, "scout car MG tracer width matches machine gun tracers");
  assertApprox(scoutCar.tailWidth, 0.5, 0.0001, "scout car MG tracer tail width matches machine gun tracers");

  const coax = muzzleFeedbackStyle(KIND.MACHINE_GUNNER, WEAPON_KIND.TANK_COAX);
  assertApprox(coax.tracerWidth, 0.9, 0.0001, "tank coax tracer width is 50% of the old coax line");
  assertApprox(coax.tracerCoreWidth, 0.375, 0.0001, "tank coax tracer core width is 50% of the old coax core");
  assertApprox(coax.tailWidth, 0.45, 0.0001, "tank coax tracer tail width is 50% of the old coax tail");

  const antiTankGun = muzzleFeedbackStyle(KIND.ANTI_TANK_GUN, WEAPON_KIND.ANTI_TANK_GUN);
  assertApprox(antiTankGun.tracerWidth, 2.5, 0.0001, "anti-tank gun tracer width is unchanged");
  assertApprox(antiTankGun.tailWidth, 1.4, 0.0001, "anti-tank gun tracer tail width is unchanged");
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
    livePanzerfaustShots() {
      return [{ fromX: 128, fromY: 128, toX: 176, toY: 128, durationMs: 500, seed: 123, createdAt: performance.now() - 120 }];
    },
    livePanzerfaustImpacts() {
      return [{ x: 176, y: 128, seed: 124, createdAt: performance.now() - 100 }];
    },
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
    attackTargetPreview: {
      targetId: 88,
      kind: KIND.RIFLEMAN,
      x: 144,
      y: 160,
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
  assert(feedbackView.attackTargetPreview?.targetId === 88, "feedback view exposes attack target hover preview");
  assert(feedbackView.resourceMiningPreview?.resourceId === 200, "feedback view exposes resource mining preview");
  assert(feedbackView.abilityObjects.length === 1, "feedback view exposes ability objects");
  assert(feedbackView.panzerfaustShots.length === 1, "feedback view exposes Panzerfaust launch/travel effects");
  assert(feedbackView.panzerfaustImpacts.length === 1, "feedback view exposes Panzerfaust impact effects");

  const placementGfx = new RecordingGraphics();
  const feedbackGfx = new RecordingGraphics();
  const abilityObjectGfx = new RecordingGraphics();
  const renderer = {
    _placementGfx: placementGfx,
    _feedbackGfx: feedbackGfx,
    _abilityObjectGfx: abilityObjectGfx,
    _lineProjectileTrails: new Map(),
    _map: { tileSize: 32 },
    _ringRadius: () => ({ rx: 18, ry: 12, cy: 5 }),
  };
  _drawPlacement.call(renderer, feedbackView, null);
  _drawCommandFeedback.call(renderer, feedbackView);
  _drawAttackTargetPreview.call(renderer, feedbackView);
  _drawAntiTankGunSetupPreview.call(renderer, feedbackView);
  _drawAbilityTargetPreview.call(renderer, feedbackView);
  _drawAbilityObjects.call(renderer, feedbackView);
  _drawResourceMiningPreview.call(renderer, feedbackView);
  _drawMortarImpacts.call(renderer, feedbackView);
  _drawPanzerfaustShots.call(renderer, feedbackView);
  _drawPanzerfaustImpacts.call(renderer, feedbackView);

  assert(placementGfx.calls.some((call) => call[0] === "drawRoundedRect"), "renderer feedback reads placement through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "drawCircle"), "renderer feedback reads command/preview state through the feedback view");
  assert(
    feedbackGfx.calls.some((call) => call[0] === "drawEllipse" && call[1] === 144 && call[2] === 165 && call[3] === 18),
    "renderer feedback draws attack target hover rings through the feedback view",
  );
  assert(feedbackGfx.calls.some((call) => call[0] === "lineTo"), "renderer feedback reads resource mining preview through the feedback view");
  assert(feedbackGfx.calls.some((call) => call[0] === "drawPolygon"), "renderer feedback draws live mortar impacts without missing helper references");
  assert(feedbackGfx.calls.some((call) => call[0] === "drawRect" || call[0] === "drawPolygon"), "renderer feedback draws Panzerfaust shot and impact cues");
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
  const selected = [
    {
      id: 81,
      owner: 1,
      kind: KIND.BARRACKS,
      x: 100,
      y: 100,
      hp: 500,
      maxHp: 500,
      rallyPlan: [{ kind: "move", x: 140, y: 110 }],
    },
    {
      id: 82,
      owner: 2,
      kind: KIND.BARRACKS,
      x: 260,
      y: 100,
      hp: 500,
      maxHp: 500,
      rallyPlan: [{ kind: "move", x: 300, y: 110 }],
    },
    {
      id: 83,
      owner: 1,
      kind: KIND.ANTI_TANK_GUN,
      x: 100,
      y: 180,
      hp: 120,
      maxHp: 120,
      setupState: SETUP.DEPLOYED,
      setupFacing: 0,
    },
    {
      id: 84,
      owner: 2,
      kind: KIND.ANTI_TANK_GUN,
      x: 260,
      y: 180,
      hp: 120,
      maxHp: 120,
      setupState: SETUP.DEPLOYED,
      setupFacing: Math.PI,
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
    showUnitRangesEnabled: false,
    selectedEntities() {
      return selected;
    },
  };
  const feedbackView = buildRendererFeedbackView(feedbackState, { selectedEntities: selected });
  assert(feedbackView.issueAsOwnerId === null, "mixed-owner lab selection has no issue-as owner");
  assert(feedbackView.feedbackOwnerId === null, "mixed-owner lab selection has no single feedback owner");
  assert(
    JSON.stringify(feedbackView.feedbackOwnerIds) === JSON.stringify([1, 2]),
    "mixed-owner lab feedback keeps selected owners inspectable",
  );
  assert(feedbackView.isFeedbackOwner(1), "mixed-owner lab feedback treats selected P1 as inspectable");
  assert(feedbackView.isFeedbackOwner(2), "mixed-owner lab feedback treats selected P2 as inspectable");
  assert(feedbackView.showSelectedFieldOfFireEnabled, "mixed-owner lab support weapons keep inspection cones enabled");

  const rallyGfx = new RecordingGraphics();
  _drawRallyPoints.call({ _feedbackGfx: rallyGfx }, feedbackView);
  assert(
    rallyGfx.calls.filter((call) => call[0] === "drawPolygon").length === 2,
    "mixed-owner lab selections draw rally flags for every selected owner",
  );

  const rangeGfx = new RecordingGraphics();
  _drawSelectedUnitRanges.call({ _feedbackGfx: rangeGfx, _map: { tileSize: 32 } }, feedbackView);
  assert(
    rangeGfx.calls.filter((call) => call[0] === "arc").length >= 2,
    "mixed-owner lab selections draw field-of-fire wedges for every selected owner",
  );

  for (const entity of selected.slice(0, 2)) {
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
      entity,
      new Set([entity.id]),
      feedbackView,
    );
    assert(
      ringGfx.calls.some((call) => call[0] === "lineStyle" && call[2] === COLORS.selectOwn),
      "mixed-owner lab selected entities use controllable selection-ring color",
    );
  }
}

{
  const priorNow = performance.now;
  const fixedNow = 8200;
  const feedbackGfx = new RecordingGraphics();
  const tank = {
    id: 91,
    owner: 1,
    kind: KIND.TANK,
    x: 100,
    y: 100,
    weaponFacing: 0,
    facing: 0,
  };
  const target = {
    id: 92,
    owner: 2,
    kind: KIND.WORKER,
    x: 184,
    y: 100,
  };
  const mainMuzzle = { x: tank.x + 24.875, y: tank.y };
  const coaxMuzzle = { x: tank.x + 12.775, y: tank.y - 5.55 };

  performance.now = () => fixedNow - 100;
  try {
    _drawMuzzleFlashes.call({
      visualNow: () => fixedNow,
      _feedbackGfx: feedbackGfx,
      _map: { tileSize: 32 },
      _liveRigDefinitionsByKind: createLiveRigDefinitions(),
    }, {
      entityById(id) {
        return id === tank.id ? tank : id === target.id ? target : null;
      },
      weaponRecoil(id, kind, now) {
        assertApprox(now, fixedNow, 0.001, "muzzle origin samples recoil at the current frame time");
        return id === tank.id && kind === KIND.TANK ? 0.5 : 0;
      },
      liveMuzzleFlashes(now) {
        assertApprox(now, fixedNow, 0.001, "muzzle renderer samples current frame time");
        return [{
          from: tank.id,
          to: target.id,
          targetPos: { x: target.x, y: target.y },
          weaponKind: WEAPON_KIND.TANK_CANNON,
          createdAt: fixedNow,
        }, {
          from: tank.id,
          to: target.id,
          targetPos: { x: target.x, y: target.y },
          weaponKind: WEAPON_KIND.TANK_COAX,
          createdAt: fixedNow,
        }];
      },
    });
  } finally {
    performance.now = priorNow;
  }

  const circles = feedbackGfx.calls.filter((call) => call[0] === "drawCircle");
  assert(circles.length >= 2, "same-tick tank coax still draws visible muzzle flashes");
  const cannonCircles = circles.filter((call) => nearPoint(call, mainMuzzle));
  const coaxCircles = circles.filter((call) => nearPoint(call, coaxMuzzle));
  const cannonTraceStarts = feedbackGfx.calls.filter((call) => call[0] === "moveTo" && nearPoint(call, mainMuzzle));
  assert(cannonTraceStarts.length >= 1, "tank cannon tracer uses the animated main muzzle anchor");
  assert(cannonCircles.length === 0, "tank cannon circular muzzle flash is suppressed for the rig-authored flare");
  assert(coaxCircles.length >= 2, "tank coax muzzle flash uses the animated coax muzzle anchor");
  assert(
    coaxCircles.every((call) => call[3] <= 7),
    "tank coax muzzle flash uses machine-gun scale rather than Tank cannon scale",
  );
  assert(
    feedbackGfx.calls.some((call) => call[0] === "moveTo" && nearPoint(call, mainMuzzle)),
    "tank cannon tracer starts at the main muzzle anchor",
  );
  assert(
    feedbackGfx.calls.some((call) => call[0] === "moveTo" && nearPoint(call, coaxMuzzle)),
    "tank coax tracer starts at the coax muzzle anchor",
  );
  assert(
    feedbackGfx.calls.some((call) => (
      call[0] === "lineStyle" &&
      Math.abs(call[1] - 0.9) <= 0.0001 &&
      call[2] === 0xfff0a6
    )),
    "tank coax tracer uses a thinner bright MG tracer line",
  );
  assert(
    feedbackGfx.calls.some((call) => (
      call[0] === "lineStyle" &&
      Math.abs(call[1] - 0.375) <= 0.0001 &&
      call[2] === 0xffffff
    )),
    "tank coax tracer includes a thinner hot core line for readability",
  );
}

{
  const priorNow = performance.now;
  const fixedNow = 8400;
  const feedbackGfx = new RecordingGraphics();
  const tankReveal = {
    id: 93,
    owner: 2,
    kind: KIND.TANK,
    x: 200,
    y: 200,
    weaponFacing: Math.PI / 2,
    facing: 0,
    shotReveal: true,
  };
  const target = {
    id: 94,
    owner: 1,
    kind: KIND.WORKER,
    x: 200,
    y: 280,
  };
  const expectedCoaxMuzzle = { x: 205.55, y: 216.6 };

  performance.now = () => fixedNow;
  try {
    _drawMuzzleFlashes.call({
      _feedbackGfx: feedbackGfx,
      _map: { tileSize: 32 },
      _liveRigDefinitionsByKind: createLiveRigDefinitions(),
    }, {
      entityById(id) {
        return id === tankReveal.id ? tankReveal : id === target.id ? target : null;
      },
      liveMuzzleFlashes() {
        return [{
          from: tankReveal.id,
          to: target.id,
          targetPos: { x: target.x, y: target.y },
          weaponKind: WEAPON_KIND.TANK_COAX,
          createdAt: fixedNow,
        }];
      },
    });
  } finally {
    performance.now = priorNow;
  }

  assert(
    feedbackGfx.calls.some((call) => call[0] === "moveTo" && nearPoint(call, expectedCoaxMuzzle)),
    "shot-reveal tank coax tracer uses the transformed coax muzzle when rig data is available",
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

{
  const unitPreview = new RecordingGraphics();
  drawLabToolPreview(unitPreview, {
    kind: "spawnEntity",
    payload: { kind: KIND.RIFLEMAN, owner: 2 },
    x: 96,
    y: 128,
  }, 32);
  assert(
    unitPreview.calls.some((call) => call[0] === "drawCircle" && call[1] === 96 && call[2] === 128),
    "armed Lab unit tools draw a unit ghost directly beneath the cursor",
  );

  const buildingPreview = new RecordingGraphics();
  drawLabToolPreview(buildingPreview, {
    kind: "spawnEntity",
    payload: { kind: KIND.CITY_CENTRE, owner: 1 },
    x: 160,
    y: 160,
  }, 32);
  assert(
    buildingPreview.calls.some((call) => call[0] === "drawRoundedRect"),
    "armed Lab building tools draw their snapped footprint ghost",
  );

  const terrainPreview = new RecordingGraphics();
  drawLabToolPreview(terrainPreview, {
    kind: "editMapTerrain",
    payload: { terrain: TERRAIN.WATER },
    x: 79,
    y: 97,
  }, 32);
  assert(
    terrainPreview.calls.some((call) => call[0] === "drawRect" && call[1] === 64 && call[2] === 96),
    "armed Lab terrain tools draw the selected tile ghost beneath the cursor",
  );

  const removePreview = new RecordingGraphics();
  drawLabToolPreview(removePreview, {
    kind: "removeSelectableUnits",
    x: 224,
    y: 192,
  }, 32);
  assert(
    removePreview.calls.filter((call) => call[0] === "lineTo").length === 2,
    "armed Lab remove tools draw a clear X beneath the cursor",
  );

  const draftOverlay = new RecordingGraphics();
  drawLabMapDraftOverlay(draftOverlay, {
    players: [{
      playerIndex: 0,
      start: { x: 8, y: 9 },
      naturals: [{ x: 12, y: 13 }],
    }],
  }, 32);
  assert(
    draftOverlay.calls.some((call) => call[0] === "drawCircle" && call[1] === 272 && call[2] === 304),
    "the Lab map draft draws a persistent Player 1 start marker at its authored tile",
  );
  assert(
    draftOverlay.calls.some((call) => call[0] === "drawCircle" && call[1] === 400 && call[2] === 432),
    "the Lab map draft draws each authored natural base on the map",
  );

  const feedbackView = buildRendererFeedbackView(
    { map: { width: 8, height: 8, tileSize: 32 } },
    {
      clientIntent: {
        labToolPreview: { toolId: "lab-tool-1", kind: "removeSelectableUnits", x: 224, y: 192 },
        labMapDraftOverlay: { players: [{ playerIndex: 0, start: { x: 8, y: 9 }, naturals: [] }] },
      },
    },
  );
  assert(
    feedbackView.labToolPreview?.kind === "removeSelectableUnits",
    "renderer feedback view carries the active Lab tool preview across the intent boundary",
  );
  assert(
    feedbackView.labMapDraftOverlay?.players?.[0]?.start?.x === 8,
    "renderer feedback view carries persistent authored map markers across the intent boundary",
  );
}
