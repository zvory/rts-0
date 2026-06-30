// tests/client_contracts/artillery_targeting_contracts.mjs
// Focused artillery target-locking and preview contracts imported by ../client_contracts.mjs.

import { assert, assertApprox } from "./assertions.mjs";
import {
  ABILITIES,
  ARTILLERY_BLANKET_RADIUS_TILES,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_RANGE_TILES,
} from "../../client/src/config.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import {
  ABILITY,
  KIND,
  ORDER_STAGE,
  SETUP,
} from "../../client/src/protocol.js";
import {
  _drawAbilityTargetPreview,
} from "../../client/src/renderer/feedback.js";
import { _drawSelectedUnitRanges } from "../../client/src/renderer/unit_ranges.js";

import { RecordingGraphics } from "./pixi_fakes.mjs";

{
  const artilleryCommands = [];
  const artilleryFeedback = [];
  const selectedArtillery = {
    id: 44,
    owner: 1,
    kind: KIND.ARTILLERY,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
    setupFacing: Math.PI,
  };
  const pointFireInput = Object.create(Input.prototype);
  pointFireInput.mouse = { x: 900, y: 100 };
  pointFireInput.state = {
    playerId: 1,
    map: { tileSize: 32 },
    selectedEntities: () => [selectedArtillery],
  };
  pointFireInput.clientIntent = new ClientIntent();
  pointFireInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.POINT_FIRE });
  pointFireInput.clientIntent.addCommandFeedback = (kind, x, y, queued, radiusTiles) => {
    artilleryFeedback.push({ kind, x, y, queued, radiusTiles });
  };
  pointFireInput.commandIssuer = { issueCommand: (command) => artilleryCommands.push(command) };
  pointFireInput._worldAt = (x, y) => ({ x, y });
  pointFireInput._selectedOwnUnitIds = () => [selectedArtillery.id];
  const closeRawPoint = {
    x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8,
    y: selectedArtillery.y,
  };
  pointFireInput._issueTargetedCommand(closeRawPoint, { shiftKey: true });
  assert(
    artilleryCommands[0]?.c === "useAbility" &&
      artilleryCommands[0].ability === ABILITY.POINT_FIRE &&
      artilleryCommands[0].units[0] === selectedArtillery.id &&
      artilleryCommands[0].x === closeRawPoint.x &&
      artilleryCommands[0].queued === true,
    "Point Fire targeting sends the raw click in the dedicated ability command",
  );
  assert(
    artilleryFeedback[0]?.kind === "artillery" &&
      artilleryFeedback[0].radiusTiles === ABILITIES[ABILITY.POINT_FIRE].radiusTiles &&
      artilleryFeedback[0].x === selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 &&
      artilleryFeedback[0].y === selectedArtillery.y,
    "Point Fire targeting shows command feedback at the locked effective point with splash radius",
  );

  pointFireInput.clientIntent.endCommandTarget();
  pointFireInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.BLANKET_FIRE });
  const farRawPoint = {
    x: selectedArtillery.x + ARTILLERY_MAX_RANGE_TILES * 32 + 16,
    y: selectedArtillery.y,
  };
  pointFireInput._issueTargetedCommand(farRawPoint, { shiftKey: false });
  assert(
    artilleryCommands[1]?.ability === ABILITY.BLANKET_FIRE &&
      artilleryCommands[1].x === farRawPoint.x &&
      artilleryCommands[1].queued !== true,
    "Blanket Fire targeting sends the raw click through the normal ability command",
  );
  assert(
    artilleryFeedback[1]?.kind === "artillery" &&
      artilleryFeedback[1].radiusTiles === ARTILLERY_BLANKET_RADIUS_TILES &&
      artilleryFeedback[1].x === selectedArtillery.x + ARTILLERY_MAX_RANGE_TILES * 32,
    "Blanket Fire command feedback marks the locked center and blanket radius",
  );

  const futureOrigin = { x: 640, y: 100 };
  const queuedMovingArtillery = {
    ...selectedArtillery,
    id: 45,
    orderPlan: [{ kind: ORDER_STAGE.MOVE, x: futureOrigin.x, y: futureOrigin.y }],
  };
  pointFireInput.state.selectedEntities = () => [queuedMovingArtillery];
  pointFireInput._selectedOwnUnitIds = () => [queuedMovingArtillery.id];
  pointFireInput.clientIntent.endCommandTarget();
  pointFireInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.POINT_FIRE });
  const queuedRawPoint = {
    x: futureOrigin.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8,
    y: futureOrigin.y,
  };
  pointFireInput._issueTargetedCommand(queuedRawPoint, { shiftKey: true });
  assert(
    artilleryCommands[2]?.ability === ABILITY.POINT_FIRE &&
      artilleryCommands[2].x === queuedRawPoint.x &&
      artilleryCommands[2].queued === true,
    "Queued Point Fire targeting still sends the raw click to the server",
  );
  assertApprox(
    artilleryFeedback[2]?.x,
    futureOrigin.x + ARTILLERY_MIN_RANGE_TILES * 32,
    0.001,
    "Queued Point Fire feedback locks from the projected movement endpoint",
  );

  pointFireInput.clientIntent.endCommandTarget();
  pointFireInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.POINT_FIRE });
  pointFireInput.state.selectedEntities = () => [selectedArtillery];
  pointFireInput._selectedOwnUnitIds = () => [selectedArtillery.id];
  pointFireInput.mouse = {
    x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8,
    y: selectedArtillery.y,
  };
  pointFireInput._refreshAbilityTargetPreview();
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.hoverInRange === true &&
      pointFireInput.clientIntent.abilityTargetPreview?.hoverInsideMinRange === false,
    "Point Fire preview accepts minimum-range locking clicks",
  );
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.minRangePx === ARTILLERY_MIN_RANGE_TILES * 32,
    "Point Fire preview exposes minimum range in pixels",
  );
  assertApprox(
    pointFireInput.clientIntent.abilityTargetPreview.mouseX,
    selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32,
    0.001,
    "Point Fire preview reticle locks inside-minimum hovers to the effective point",
  );
  assert(
    ARTILLERY_MIN_RANGE_TILES === 25 && ARTILLERY_MAX_RANGE_TILES === 55,
    "Artillery point-fire range mirrors the 25-55 tile balance band",
  );

  pointFireInput.state.selectedEntities = () => [queuedMovingArtillery];
  pointFireInput._shiftKeyDown = true;
  pointFireInput.mouse = queuedRawPoint;
  pointFireInput._refreshAbilityTargetPreview();
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.artilleryLocks?.[0]?.originX === futureOrigin.x,
    "Queued Point Fire preview uses the projected movement endpoint as the lock origin",
  );
  assertApprox(
    pointFireInput.clientIntent.abilityTargetPreview.mouseX,
    futureOrigin.x + ARTILLERY_MIN_RANGE_TILES * 32,
    0.001,
    "Queued Point Fire preview reticle locks from the projected movement endpoint",
  );
  pointFireInput._shiftKeyDown = false;
  pointFireInput._refreshAbilityTargetPreview();
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.hoverInRange === false &&
      pointFireInput.clientIntent.abilityTargetPreview?.artilleryLocks?.length === 0,
    "Unqueued Point Fire preview does not lock a gun whose active order-plan marker is movement",
  );

  pointFireInput.state.selectedEntities = () => [selectedArtillery];
  const deployedArtillery = { ...selectedArtillery, setupState: SETUP.DEPLOYED, setupFacing: 0 };
  const artilleryConeGfx = new RecordingGraphics();
  _drawSelectedUnitRanges.call(
    { _feedbackGfx: artilleryConeGfx, _map: { tileSize: 32 } },
    { playerId: 1, showUnitRangesEnabled: true, selectedEntities: () => [deployedArtillery] },
  );
  const artilleryConeArcs = artilleryConeGfx.calls.filter((call) => call[0] === "arc");
  assert(
    artilleryConeArcs.some((call) => call[3] === ARTILLERY_MAX_RANGE_TILES * 32),
    "Artillery field-of-fire cone preview uses the mirrored maximum range",
  );
  assert(
    artilleryConeArcs.some((call) => call[3] === ARTILLERY_MIN_RANGE_TILES * 32 && call[6] === true),
    "Artillery field-of-fire cone preview cuts out the mirrored minimum range",
  );

  pointFireInput.mouse = {
    x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 + 16,
    y: selectedArtillery.y,
  };
  pointFireInput._refreshAbilityTargetPreview();
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.hoverInRange === true &&
      pointFireInput.clientIntent.abilityTargetPreview?.hoverInsideMinRange === false,
    "Point Fire preview accepts targets past minimum range",
  );
  pointFireInput.mouse = {
    x: selectedArtillery.x + ARTILLERY_MAX_RANGE_TILES * 32 + 16,
    y: selectedArtillery.y,
  };
  pointFireInput._refreshAbilityTargetPreview();
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.hoverInRange === true,
    "Point Fire preview accepts maximum-range locking clicks",
  );
  assertApprox(
    pointFireInput.clientIntent.abilityTargetPreview.mouseX,
    selectedArtillery.x + ARTILLERY_MAX_RANGE_TILES * 32,
    0.001,
    "Point Fire preview reticle locks beyond-maximum hovers to the effective point",
  );
  const targetingConeGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: targetingConeGfx, _map: { tileSize: 32 } },
    { abilityTargetPreview: pointFireInput.clientIntent.abilityTargetPreview },
  );
  const targetingConeArcs = targetingConeGfx.calls.filter((call) => call[0] === "arc");
  assert(
    targetingConeArcs.some((call) => call[3] === ARTILLERY_MAX_RANGE_TILES * 32),
    "Point Fire targeting cone uses the mirrored maximum range",
  );
  assert(
    targetingConeArcs.some((call) => call[3] === ARTILLERY_MIN_RANGE_TILES * 32 && call[6] === true),
    "Point Fire targeting cone cuts out the mirrored minimum range",
  );

  const previewGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: previewGfx },
    { abilityTargetPreview: { ...pointFireInput.clientIntent.abilityTargetPreview, carriers: [] } },
  );
  const validHorizontalStroke = previewGfx.calls.some(
    (call, i, calls) =>
      call[0] === "moveTo" &&
      call[2] === pointFireInput.clientIntent.abilityTargetPreview.mouseY &&
      calls[i + 1]?.[0] === "lineTo" &&
      calls[i + 1]?.[2] === pointFireInput.clientIntent.abilityTargetPreview.mouseY,
  );
  assert(validHorizontalStroke, "Point Fire valid cursor keeps the crosshair stroke");

  pointFireInput.mouse = {
    x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8,
    y: selectedArtillery.y,
  };
  pointFireInput._refreshAbilityTargetPreview();
  const lockedGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: lockedGfx },
    { abilityTargetPreview: { ...pointFireInput.clientIntent.abilityTargetPreview, carriers: [] } },
  );
  const lockedHorizontalStroke = lockedGfx.calls.some(
    (call, i, calls) =>
      call[0] === "moveTo" &&
      call[2] === pointFireInput.clientIntent.abilityTargetPreview.mouseY &&
      calls[i + 1]?.[0] === "lineTo" &&
      calls[i + 1]?.[2] === pointFireInput.clientIntent.abilityTargetPreview.mouseY,
  );
  assert(lockedHorizontalStroke, "Point Fire minimum-range locking cursor keeps the crosshair stroke");

  pointFireInput.clientIntent.endCommandTarget();
  pointFireInput.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.BLANKET_FIRE });
  pointFireInput.mouse = {
    x: selectedArtillery.x + ARTILLERY_MAX_RANGE_TILES * 32 + 64,
    y: selectedArtillery.y,
  };
  pointFireInput._refreshAbilityTargetPreview();
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.artilleryLocks?.[0]?.x ===
      selectedArtillery.x + ARTILLERY_MAX_RANGE_TILES * 32,
    "Blanket Fire preview locks the blanket center per artillery gun",
  );
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.radiusPx === ARTILLERY_BLANKET_RADIUS_TILES * 32,
    "Blanket Fire preview exposes the mirrored blanket radius",
  );
  const blanketPreviewGfx = new RecordingGraphics();
  _drawAbilityTargetPreview.call(
    { _feedbackGfx: blanketPreviewGfx, _map: { tileSize: 32 } },
    { abilityTargetPreview: pointFireInput.clientIntent.abilityTargetPreview },
  );
  assert(
    blanketPreviewGfx.calls.some((call) =>
      call[0] === "moveTo" &&
        call[1] ===
          pointFireInput.clientIntent.abilityTargetPreview.mouseX +
            ARTILLERY_BLANKET_RADIUS_TILES * 32 &&
        call[2] === pointFireInput.clientIntent.abilityTargetPreview.mouseY),
    "Blanket Fire preview draws the 15-tile blanket radius around the locked center",
  );
}
