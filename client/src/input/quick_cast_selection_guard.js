import {
  POST_QUICK_CAST_SELECTION_GUARD_MS,
  POST_QUICK_CAST_SELECTION_GUARD_PX,
} from "./constants.js";

export function armPostQuickCastSelectionGuard(input, p, now = performanceNow()) {
  if (!input || !validPoint(p)) {
    clearPostQuickCastSelectionGuard(input);
    return;
  }
  input._postQuickCastSelectionGuard = { x: p.x, y: p.y, t: now };
}

export function clearPostQuickCastSelectionGuard(input) {
  if (input) input._postQuickCastSelectionGuard = null;
}

export function postQuickCastSelectionGuardActiveAt(input, p, now = performanceNow()) {
  const guard = input?._postQuickCastSelectionGuard;
  if (!guard || !validPoint(p)) return false;
  if (now - guard.t > POST_QUICK_CAST_SELECTION_GUARD_MS) {
    clearPostQuickCastSelectionGuard(input);
    return false;
  }
  if (Math.hypot(p.x - guard.x, p.y - guard.y) > POST_QUICK_CAST_SELECTION_GUARD_PX) {
    clearPostQuickCastSelectionGuard(input);
    return false;
  }
  return true;
}

export function consumePostQuickCastSelectionGuard(input, p) {
  const suppress = postQuickCastSelectionGuardActiveAt(input, p);
  clearPostQuickCastSelectionGuard(input);
  return suppress;
}

function validPoint(p) {
  return !!p && Number.isFinite(p.x) && Number.isFinite(p.y);
}

function performanceNow() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}
