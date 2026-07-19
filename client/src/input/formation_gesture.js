import { cmd } from "../protocol.js";
import { buildFormationLinePreview, appendFormationLinePoint } from "./formation_line.js";
import { DEFAULT_TILE_SIZE } from "./constants.js";

const FORMATION_LINE_MIN_TILES = 3;

export function _beginFormationGesture(p, ev = {}) {
  const world = this._groundAtScreen(p.x, p.y);
  const units = this._selectedOwnUnitIds();
  const intent = this._intent?.();
  const eligible = !!world &&
    units.length > 0 &&
    !intent?.activeLabTool &&
    !intent?.placement &&
    !intent?.commandTarget;
  this._formationGesture = {
    startWorld: world ? { x: world.x, y: world.y } : null,
    maxExtentWorld: 0,
    points: world ? [{ x: world.x, y: world.y }] : [],
    units: units.slice(),
    entities: eligible
      ? (this.state?.selectedEntities?.() || []).filter((entity) => units.includes(entity.id))
      : [],
    eligible,
    promoted: false,
    queued: !!ev.shiftKey,
  };
  return true;
}

export function _updateFormationGesture(p, ev = {}) {
  const gesture = this._formationGesture;
  if (!gesture) return false;
  gesture.queued = !!ev.shiftKey;
  if (!gesture.eligible) return true;
  const world = this._groundAtScreen(p.x, p.y);
  if (world) {
    appendFormationLinePoint(gesture.points, world);
    updatePromotion(this, gesture, world);
  }
  refreshPreview(this, gesture);
  return true;
}

export function _finishFormationGesture(p, ev = {}) {
  const gesture = this._formationGesture;
  if (!gesture) return false;
  this._formationGesture = null;
  if (!gesture.eligible) {
    this._intent?.()?.clearFormationMovePreview?.();
    this._onRightClick(p, ev);
    return true;
  }
  const world = this._groundAtScreen(p.x, p.y);
  if (world) {
    appendFormationLinePoint(gesture.points, world, { force: true });
    updatePromotion(this, gesture, world);
  }
  if (!gesture.promoted) {
    this._intent?.()?.clearFormationMovePreview?.();
    this._onRightClick(p, ev);
    return true;
  }
  const preview = buildFormationLinePreview(gesture.points, gesture.entities);
  this._intent?.()?.clearFormationMovePreview?.();
  if (preview.points.length < 2 || gesture.units.length === 0) return true;
  const queued = !!ev.shiftKey;
  this.commandInteraction.issueCommand(cmd.formationMove(gesture.units, preview.points, queued));
  const endpoint = preview.points[preview.points.length - 1];
  this._addCommandFeedback?.("move", endpoint.x, endpoint.y, queued);
  return true;
}

export function _cancelFormationGesture() {
  const hadGesture = !!this._formationGesture;
  this._formationGesture = null;
  this._intent?.()?.clearFormationMovePreview?.();
  return hadGesture;
}

export function _refreshFormationGesture() {
  const gesture = this._formationGesture;
  if (!gesture?.promoted || !this.mouse) return false;
  const world = this._groundAtScreen(this.mouse.x, this.mouse.y);
  if (world) appendFormationLinePoint(gesture.points, world);
  refreshPreview(this, gesture);
  return true;
}

function refreshPreview(input, gesture) {
  input._intent?.()?.updateFormationMovePreview?.(
    buildFormationLinePreview(gesture.points, gesture.promoted ? gesture.entities : []),
  );
}

function updatePromotion(input, gesture, world) {
  if (!gesture.startWorld) return;
  gesture.maxExtentWorld = Math.max(
    gesture.maxExtentWorld,
    Math.hypot(world.x - gesture.startWorld.x, world.y - gesture.startWorld.y),
  );
  const tileSize = input.state?.map?.tileSize || DEFAULT_TILE_SIZE;
  if (gesture.maxExtentWorld >= tileSize * FORMATION_LINE_MIN_TILES) gesture.promoted = true;
}
