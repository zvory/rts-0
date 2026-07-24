// tests/client_contracts/artillery_targeting_contracts.mjs
// Focused artillery target-locking and preview contracts imported by ../client_contracts.mjs.

import { assert, assertApprox } from "./assertions.mjs";
import {
  ABILITIES,
  ARTILLERY_BLANKET_RADIUS_TILES,
  ARTILLERY_FIELD_OF_FIRE_RAD,
  ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES,
  ARTILLERY_MAX_RANGE_TILES,
  ARTILLERY_MIN_FIRE_RADIUS_TILES,
  ARTILLERY_MIN_RANGE_TILES,
} from "../../client/src/config.js";
import { ClientIntent } from "../../client/src/client_intent.js";
import { Input } from "../../client/src/input/index.js";
import {
  artilleryFireRadiusTiles,
  artilleryMinFireRadiusTiles,
} from "../../client/src/input/artillery_targeting.js";
import {
  ABILITY,
  cmd,
  KIND,
  ORDER_STAGE,
  SETUP,
  UPGRADE,
} from "../../client/src/protocol.js";
import {
  _drawAbilityTargetPreview,
} from "../../client/src/renderer/feedback.js";
import { _drawSelectedUnitRanges } from "../../client/src/renderer/unit_ranges.js";

import { RecordingGraphics } from "./pixi_fakes.mjs";

{
  const center = { x: 100, y: 100 };
  const close = { x: 101, y: 100 };
  assert(
    artilleryFireRadiusTiles(center, close, 1) === ARTILLERY_MIN_FIRE_RADIUS_TILES &&
      artilleryFireRadiusTiles(
        center,
        close,
        1,
        artilleryMinFireRadiusTiles([UPGRADE.BALLISTIC_TABLES]),
      ) === ARTILLERY_FIRE_CONTROL_MIN_FIRE_RADIUS_TILES,
    "Artillery Fire radius selection uses the six-tile base minimum and three-tile Fire Control minimum",
  );
}

{
  const artilleryPreviewInput = Object.create(Input.prototype);
  artilleryPreviewInput.mouse = { x: 500, y: 300 };
  artilleryPreviewInput.state = {
    playerId: 1,
    map: { width: 64, height: 64, tileSize: 32 },
    selectedEntities: () => [{
      id: 91,
      owner: 1,
      kind: KIND.ARTILLERY,
      x: 200,
      y: 200,
      facing: 0,
      setupState: SETUP.DEPLOYED,
    }],
  };
  artilleryPreviewInput.clientIntent = new ClientIntent();
  artilleryPreviewInput.clientIntent.beginCommandTarget({
    kind: "ability",
    ability: ABILITY.POINT_FIRE,
  });
  artilleryPreviewInput.camera = {
    projectionSnapshot: () => ({
      groundAtScreen: ({ x, y }) => ({ x: x + 100, y: y - 100 }),
    }),
  };
  artilleryPreviewInput._groundAtScreen = () => ({ x: 200, y: 600 });
  artilleryPreviewInput._refreshAbilityTargetPreview();
  const artilleryPreview = artilleryPreviewInput.clientIntent.abilityTargetPreview;
  assert(
    artilleryPreview?.rawMouseX === 600 && artilleryPreview?.rawMouseY === 200 &&
      Math.abs(artilleryPreview.artilleryLocks?.[0]?.facing) < 0.001,
    "artillery fire preview follows the current renderer projection instead of a stale selection scene",
  );
}

{
  const commands = [];
  const artillery = {
    id: 49,
    owner: 1,
    kind: KIND.ARTILLERY,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
    setupFacing: 0,
  };
  const input = Object.create(Input.prototype);
  input.mouse = { x: 900, y: 100 };
  input.state = {
    playerId: 1,
    map: { tileSize: 32 },
    selectedEntities: () => [artillery],
  };
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.POINT_FIRE });
  input.commandInteraction = { issueCommand: (command) => commands.push(command) };
  input._groundAtScreen = (x, y) => ({ x, y });
  input._selectedOwnUnitIds = () => [artillery.id];
  input._addCommandFeedback = () => {};

  input._quickCastCommandTarget({ shiftKey: false });
  assert(
    commands.length === 0 &&
      input.clientIntent.artilleryFireCenter?.x === 900 &&
      input.clientIntent.commandTarget?.ability === ABILITY.POINT_FIRE,
    "quick-cast keeps unified Artillery Fire armed after selecting its center",
  );
  input.mouse = { x: 900 + 6 * 32, y: 100 };
  input._quickCastCommandTarget({ shiftKey: false });
  assert(
    commands[0]?.c === "artilleryFire" &&
      commands[0].radiusTiles === 6 &&
      input.clientIntent.commandTarget === null,
    "quick-cast issues Artillery Fire only after selecting its radius",
  );
}

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
  pointFireInput.commandInteraction = { issueCommand: (command) => artilleryCommands.push(command) };
  pointFireInput._groundAtScreen = (x, y) => ({ x, y });
  pointFireInput._selectedOwnUnitIds = () => [selectedArtillery.id];
  const closeRawPoint = {
    x: selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 - 8,
    y: selectedArtillery.y,
  };
  const firstClickResult = pointFireInput._issueTargetedCommand(closeRawPoint, { shiftKey: true });
  assert(
    firstClickResult === false &&
      artilleryCommands.length === 0 &&
      pointFireInput.clientIntent.artilleryFireCenter?.x === closeRawPoint.x,
    "Artillery Fire first click stores the raw center without issuing a command",
  );
  const radiusPoint = { x: closeRawPoint.x + 6 * 32, y: closeRawPoint.y };
  pointFireInput._issueTargetedCommand(radiusPoint, { shiftKey: true });
  assert(
    artilleryCommands[0]?.c === "artilleryFire" &&
      artilleryCommands[0].units[0] === selectedArtillery.id &&
      artilleryCommands[0].x === closeRawPoint.x &&
      artilleryCommands[0].radiusTiles === 6 &&
      artilleryCommands[0].queued === true,
    "Artillery Fire second click sends the raw center and selected radius",
  );
  assert(
    artilleryFeedback[0]?.kind === "artillery" &&
      artilleryFeedback[0].radiusTiles === 6 &&
      artilleryFeedback[0].x === selectedArtillery.x + ARTILLERY_MIN_RANGE_TILES * 32 &&
      artilleryFeedback[0].y === selectedArtillery.y,
    "Artillery Fire targeting shows the selected circle at the locked effective center",
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
  pointFireInput._issueTargetedCommand(
    { x: queuedRawPoint.x + 4 * 32, y: queuedRawPoint.y },
    { shiftKey: true },
  );
  assert(
    artilleryCommands[2]?.c === "artilleryFire" &&
      artilleryCommands[2].x === queuedRawPoint.x &&
      artilleryCommands[2].radiusTiles === 6 &&
      artilleryCommands[2].queued === true,
    "Queued Artillery Fire sends its raw center and selected radius to the server",
  );
  assertApprox(
    artilleryFeedback[2]?.x,
    futureOrigin.x + ARTILLERY_MIN_RANGE_TILES * 32,
    0.001,
    "Queued Artillery Fire feedback locks from the projected movement endpoint",
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
    ARTILLERY_MIN_RANGE_TILES === 10 &&
      ARTILLERY_MAX_RANGE_TILES === 35 &&
      ARTILLERY_FIELD_OF_FIRE_RAD === 30 * Math.PI / 180,
    "Artillery targeting mirrors the 10-35 tile range band and 30-degree field of fire",
  );

  pointFireInput.state.selectedEntities = () => [queuedMovingArtillery];
  pointFireInput.clientIntent.clearPlannedOrders();
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

  const locallyPlannedArtillery = {
    ...selectedArtillery,
    id: 46,
    x: 200,
    y: 200,
    orderPlan: [],
  };
  const localMove = { x: 720, y: 256 };
  const localSetupTarget = { x: localMove.x, y: localMove.y + 320 };
  pointFireInput.state.selectedEntities = () => [locallyPlannedArtillery];
  pointFireInput._selectedOwnUnitIds = () => [locallyPlannedArtillery.id];
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.move([locallyPlannedArtillery.id], localMove.x, localMove.y, false),
    [locallyPlannedArtillery],
    { sent: true, clientSeq: 90 },
  );
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.setupAntiTankGuns([locallyPlannedArtillery.id], localSetupTarget.x, localSetupTarget.y, true),
    [locallyPlannedArtillery],
    { sent: true, clientSeq: 91 },
  );
  pointFireInput._shiftKeyDown = true;
  pointFireInput.mouse = { x: localMove.x, y: localMove.y };
  pointFireInput._refreshAbilityTargetPreview();
  assert(
    pointFireInput.clientIntent.abilityTargetPreview?.artilleryLocks?.[0]?.originX === localMove.x &&
      pointFireInput.clientIntent.abilityTargetPreview?.artilleryLocks?.[0]?.originY === localMove.y,
    "Queued Point Fire preview uses local movement before authoritative orderPlan echo",
  );
  assertApprox(
    pointFireInput.clientIntent.abilityTargetPreview?.mouseY,
    localMove.y + ARTILLERY_MIN_RANGE_TILES * 32,
    0.001,
    "Queued Point Fire preview uses the frozen setup facing for zero-length target rays",
  );
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.blanketFire([locallyPlannedArtillery.id], localMove.x, localMove.y, 6, true),
    [locallyPlannedArtillery],
    { sent: true, clientSeq: 92 },
  );
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.move([locallyPlannedArtillery.id], localMove.x + 64, localMove.y, true),
    [locallyPlannedArtillery],
    { sent: true, clientSeq: 93 },
  );
  const localPlanAfterTerminal = pointFireInput.clientIntent.plannedOrderPlanForEntity(locallyPlannedArtillery);
  assert(
    localPlanAfterTerminal.some((stage) => stage.kind === ORDER_STAGE.BLANKET_FIRE) &&
      !localPlanAfterTerminal.some((stage) => stage.kind === ORDER_STAGE.MOVE && stage.x === localMove.x + 64),
    "Client planned order stages do not append behind terminal queued Artillery Fire",
  );
  const rejectedSetupArtillery = { ...locallyPlannedArtillery, id: 47 };
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.move([rejectedSetupArtillery.id], localMove.x, localMove.y, false),
    [rejectedSetupArtillery],
    { sent: true, clientSeq: 100 },
  );
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.setupAntiTankGuns([rejectedSetupArtillery.id], localSetupTarget.x, localSetupTarget.y, true),
    [rejectedSetupArtillery],
    { sent: true, clientSeq: 101 },
  );
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.blanketFire([rejectedSetupArtillery.id], localMove.x, localMove.y, 6, true),
    [rejectedSetupArtillery],
    { sent: true, clientSeq: 102 },
  );
  pointFireInput.clientIntent.clearPlannedOrdersForClientSeq(101);
  const rejectionPlan = pointFireInput.clientIntent.plannedOrderPlanForEntity(rejectedSetupArtillery);
  assert(
    rejectionPlan.length === 1 && rejectionPlan[0].kind === ORDER_STAGE.MOVE,
    "Rejected queued setup clears dependent local fire previews for that artillery",
  );
  pointFireInput.clientIntent.reconcilePlannedOrders([
    {
      ...locallyPlannedArtillery,
      orderPlan: [{ kind: ORDER_STAGE.MOVE, x: localMove.x, y: localMove.y }],
    },
  ], { acknowledgedClientSeq: 93 });
  assert(
    pointFireInput.clientIntent.plannedOrderPlanForEntity(locallyPlannedArtillery).length === 0,
    "Authoritative orderPlan mismatch clears stale local queued setup and fire stages",
  );

  const terminalAuthorityArtillery = {
    ...selectedArtillery,
    id: 48,
    orderPlan: [{ kind: ORDER_STAGE.POINT_FIRE, x: 300, y: 300 }],
  };
  const replacementMove = { x: 520, y: 540 };
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.move([terminalAuthorityArtillery.id], replacementMove.x, replacementMove.y, false),
    [terminalAuthorityArtillery],
    { sent: true, clientSeq: 110 },
  );
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.setupAntiTankGuns([terminalAuthorityArtillery.id], replacementMove.x, replacementMove.y + 96, true),
    [terminalAuthorityArtillery],
    { sent: true, clientSeq: 111 },
  );
  const replacementPlan = pointFireInput.clientIntent.plannedOrderPlanForEntity(terminalAuthorityArtillery);
  assert(
    replacementPlan[0]?.kind === ORDER_STAGE.MOVE &&
      replacementPlan[0].x === replacementMove.x &&
      replacementPlan[1]?.kind === ORDER_STAGE.SETUP_ANTI_TANK_GUNS &&
      !replacementPlan.some((stage) => stage.kind === ORDER_STAGE.POINT_FIRE),
    "Unqueued local movement replaces an old terminal authoritative plan before queued setup is appended",
  );

  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.move([locallyPlannedArtillery.id], localMove.x, localMove.y, false),
    [locallyPlannedArtillery],
    { sent: true, clientSeq: 120 },
  );
  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.move([rejectedSetupArtillery.id], localMove.x, localMove.y, false),
    [rejectedSetupArtillery],
    { sent: true, clientSeq: 121 },
  );
  pointFireInput.clientIntent.clearPlannedOrdersOutsideSelection(new Set([locallyPlannedArtillery.id]));
  assert(
    pointFireInput.clientIntent.plannedOrderPlanForEntity(locallyPlannedArtillery).length === 1 &&
      pointFireInput.clientIntent.plannedOrderPlanForEntity(rejectedSetupArtillery).length === 0,
    "Selection reconciliation preserves local plans for ids kept in a Set selection",
  );

  pointFireInput.clientIntent.recordPlannedCommand(
    cmd.move([rejectedSetupArtillery.id], localMove.x, localMove.y, false),
    [rejectedSetupArtillery],
    Promise.resolve({ sent: true }),
  );
  assert(
    pointFireInput.clientIntent.plannedOrderPlanForEntity(rejectedSetupArtillery).length === 0,
    "Async lab-style command results are not recorded as durable local order plans without a client sequence",
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

{
  const commands = [];
  const artillery = {
    id: 81,
    owner: 1,
    kind: KIND.ARTILLERY,
    x: 100,
    y: 100,
    setupState: SETUP.DEPLOYED,
    setupFacing: 0,
  };
  const input = Object.create(Input.prototype);
  input.pointerLocked = false;
  input._panDrag = null;
  input._formationGesture = null;
  input._placementDrag = null;
  input.state = {
    playerId: 1,
    upgrades: [],
    map: { tileSize: 32 },
    selectedEntities: () => [artillery],
  };
  input.clientIntent = new ClientIntent();
  input.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.POINT_FIRE });
  input.commandInteraction = { issueCommand: (command) => commands.push(command) };
  input._groundAtScreen = (x, y) => ({ x, y });
  input._selectedOwnUnitIds = () => [artillery.id];
  input._addCommandFeedback = () => {};
  input._routeLockedPointerMove = () => false;
  input._routeLockedPointerUp = () => false;
  input._finishTankTrapPlacementDrag = () => false;
  input._eventScreenPos = (ev) => ({ x: ev.clientX, y: ev.clientY });
  input._trackMouse = () => {};

  input._onLeftDown({ x: 900, y: 100 }, { shiftKey: false });
  assert(
    commands.length === 0 && input._artilleryFireGesture && input.clientIntent.artilleryFireCenter?.x === 900,
    "holding the first Artillery Fire press stores its center without firing",
  );
  input._shiftKeysDown = new Set();
  input.cameraNavigation = { release() {} };
  input._drag = null;
  input._handleBlur();
  assert(
    input._artilleryFireGesture === null &&
      input.clientIntent.artilleryFireCenter === null &&
      input.clientIntent.commandTarget === null,
    "window blur cancels an interrupted battlefield Artillery Fire drag",
  );
  input.cameraNavigation = null;

  input.clientIntent.beginCommandTarget({ kind: "ability", ability: ABILITY.POINT_FIRE });
  input._onLeftDown({ x: 900, y: 100 }, { shiftKey: false });
  input._handlePointerMoveAt(
    { preventDefault() {} },
    { x: 900 + 8 * 32, y: 100 },
  );
  input._handleMouseUp({
    button: 0,
    clientX: 900 + 8 * 32,
    clientY: 100,
    shiftKey: false,
    preventDefault() {},
  });
  assert(
    commands[0]?.c === "artilleryFire" &&
      commands[0].x === 900 &&
      commands[0].radiusTiles === 8 &&
      input.clientIntent.commandTarget === null,
    "dragging the first Artillery Fire press fires with the chosen radius on release",
  );
}
