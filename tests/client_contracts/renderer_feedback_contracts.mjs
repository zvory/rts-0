// tests/client_contracts/renderer_feedback_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import { assert } from "./assertions.mjs";
import { COLORS } from "../../client/src/config.js";
import {
  ABILITY,
  ABILITY_OBJECT_KIND,
  KIND,
  SETUP,
} from "../../client/src/protocol.js";
import { buildRendererFeedbackView } from "../../client/src/renderer/feedback_view_model.js";
import {
  _drawAbilityObjects,
  _drawAbilityTargetPreview,
  _drawAntiTankGunSetupPreview,
  _drawCommandFeedback,
  _drawMortarImpacts,
  _drawPlacement,
  _drawResourceMiningPreview,
} from "../../client/src/renderer/feedback.js";

import { RecordingGraphics } from "./pixi_fakes.mjs";

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
