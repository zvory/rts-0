import { ABILITY } from "../protocol.js";
import { ZOOM_STEP } from "./constants.js";
import { isTextEntry } from "./placement.js";

export function _handleKeyDown(ev) {
  // Never hijack typing in inputs (lobby name field, etc.).
  if (isTextEntry(ev.target)) return;

  if (ev.code === "Escape" && this.pointerLocked) {
    this.exitPointerLock();
    ev.preventDefault();
    return;
  }

  switch (ev.code) {
    case "ArrowUp":
    case "ArrowDown":
    case "ArrowLeft":
    case "ArrowRight":
      if (this.cameraNavigation) this.cameraNavigation.handleKeyDown(ev);
      else setPanKey(this.keys, ev.code, true);
      ev.preventDefault();
      return;
    case "Escape":
      this._cancel();
      ev.preventDefault();
      return;
    case "Space":
      if (this.cameraNavigation) this.cameraNavigation.handleKeyDown(ev);
      else this._spacePan = true;
      ev.preventDefault();
      return;
    default:
      break;
  }

  const commandHotkey = this._activateCommandHotkey(ev);
  if (commandHotkey) {
    if (commandHotkey.armed?.quickCast) {
      this._quickCastCommandTarget(ev);
    }
    const intent = clientIntent(this);
    if (
      intent?.commandTarget &&
      typeof intent.holdCommandTarget === "function" &&
      (ev.shiftKey || repeatedWorldAbilityHotkeyTarget(intent.commandTarget))
    ) {
      intent.holdCommandTarget(intent.commandTarget, ev.code, ev.shiftKey);
    }
    return;
  }
  if (ev.repeat) return;
  if (this._handleControlGroupHotkey(ev)) return;
}

function repeatedWorldAbilityHotkeyTarget(target) {
  return target?.kind === "ability" && (
    target.ability === ABILITY.MORTAR_FIRE ||
    target.ability === ABILITY.SMOKE
  );
}

export function _handleKeyUp(ev) {
  switch (ev.code) {
    case "ArrowUp":
    case "ArrowDown":
    case "ArrowLeft":
    case "ArrowRight":
      if (this.cameraNavigation) this.cameraNavigation.handleKeyUp(ev);
      else setPanKey(this.keys, ev.code, false);
      ev.preventDefault();
      return;
    case "Space":
      if (this.cameraNavigation) this.cameraNavigation.handleKeyUp(ev);
      else this._spacePan = false;
      ev.preventDefault();
      return;
    case "ShiftLeft":
    case "ShiftRight":
      if (typeof clientIntent(this)?.releaseCommandTargetShift === "function") {
        clientIntent(this).releaseCommandTargetShift();
      }
      if (clientIntent(this)?.placement && typeof clientIntent(this).endPlacement === "function") {
        clientIntent(this).endPlacement();
      }
      ev.preventDefault();
      return;
    default:
      if (clientIntent(this)?.commandTarget && typeof clientIntent(this).releaseCommandTargetKey === "function") {
        clientIntent(this).releaseCommandTargetKey(ev.code, ev.shiftKey);
      }
      return;
  }
}

export function _handleBlur() {
  if (this.pointerLocked) this.exitPointerLock();
  if (this.cameraNavigation) {
    this.cameraNavigation.release();
  } else {
    if (this.keys) this.keys.up = this.keys.down = this.keys.left = this.keys.right = false;
    this.mouse = null;
    this._spacePan = false;
    this._panDrag = null;
  }
  if (typeof clientIntent(this)?.endCommandTarget === "function") clientIntent(this).endCommandTarget();
  if (clientIntent(this)?.placement && typeof clientIntent(this).endPlacement === "function") {
    clientIntent(this).endPlacement();
  }
  if (this._drag) {
    this._drag = null;
    this._dragging = false;
    this.renderer.drawSelectionBox(null);
  }
}

function clientIntent(input) {
  return input?.clientIntent || null;
}

export function _handleWheel(ev) {
  if (this.cameraNavigation) {
    this.cameraNavigation.handleWheel(ev);
    return;
  }
  ev.preventDefault();
  const p = this._screenPos(ev);
  const factor = ev.deltaY < 0 ? 1 + ZOOM_STEP : 1 / (1 + ZOOM_STEP);
  this.camera.setZoom(this.camera.zoom * factor, p.x, p.y);
}

function setPanKey(keys, code, down) {
  if (!keys) return;
  switch (code) {
    case "ArrowUp":
      keys.up = down;
      return;
    case "ArrowDown":
      keys.down = down;
      return;
    case "ArrowLeft":
      keys.left = down;
      return;
    case "ArrowRight":
      keys.right = down;
      return;
    default:
      return;
  }
}
