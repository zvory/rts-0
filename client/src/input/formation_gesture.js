import { cmd } from "../protocol.js";
import { buildFormationLinePreview, appendFormationLinePoint } from "./formation_line.js";
import { DRAG_THRESHOLD_PX } from "./constants.js";

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
    startScreen: { x: p.x, y: p.y },
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
  if (!gesture.promoted) {
    const distance = Math.hypot(p.x - gesture.startScreen.x, p.y - gesture.startScreen.y);
    if (distance < DRAG_THRESHOLD_PX) return true;
    gesture.promoted = true;
  }
  const world = this._groundAtScreen(p.x, p.y);
  if (world) appendFormationLinePoint(gesture.points, world);
  refreshPreview(this, gesture);
  return true;
}

export function _finishFormationGesture(p, ev = {}) {
  const gesture = this._formationGesture;
  if (!gesture) return false;
  this._formationGesture = null;
  if (!gesture.promoted) {
    this._intent?.()?.clearFormationMovePreview?.();
    this._onRightClick(p, ev);
    return true;
  }

  const world = this._groundAtScreen(p.x, p.y);
  if (world) appendFormationLinePoint(gesture.points, world, { force: true });
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
    buildFormationLinePreview(gesture.points, gesture.entities),
  );
}
