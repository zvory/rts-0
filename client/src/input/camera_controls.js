import { cmd, PASSABLE, isUnit, isBuilding, isResource, KIND } from "../protocol.js";
import { MINING_CC_RANGE_TILES, STATS, TANK_BODY, isProducerBuilding } from "../config.js";
import { DEFAULT_HIT_RADIUS, DEFAULT_TILE_SIZE, HIT_PAD_PX, OWN_HIT_BONUS, ZOOM_STEP } from "./constants.js";
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
      this.keys.up = true;
      ev.preventDefault();
      return;
    case "ArrowDown":
      this.keys.down = true;
      ev.preventDefault();
      return;
    case "ArrowLeft":
      this.keys.left = true;
      ev.preventDefault();
      return;
    case "ArrowRight":
      this.keys.right = true;
      ev.preventDefault();
      return;
    case "Escape":
      this._cancel();
      ev.preventDefault();
      return;
    case "Space":
      this._spacePan = true;
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
    if (ev.shiftKey && this.state.commandTarget && typeof this.state.holdCommandTarget === "function") {
      this.state.holdCommandTarget(this.state.commandTarget, ev.code, ev.shiftKey);
    }
    return;
  }
  if (ev.repeat) return;
  if (this._handleControlGroupHotkey(ev)) return;

  switch (ev.code) {
    case "KeyA":
      if (this._enterAttackMove({ shiftKey: ev.shiftKey })?.quickCast) {
        this._quickCastCommandTarget(ev);
      }
      if (ev.shiftKey && this.state.commandTarget === "attack" && typeof this.state.holdCommandTarget === "function") {
        this.state.holdCommandTarget("attack", "KeyA", ev.shiftKey);
      }
      ev.preventDefault();
      return;
    case "KeyS":
      this._issueStop();
      ev.preventDefault();
      return;
    default:
      return;
  }
}

export function _handleKeyUp(ev) {
  switch (ev.code) {
    case "ArrowUp":
      this.keys.up = false;
      ev.preventDefault();
      return;
    case "ArrowDown":
      this.keys.down = false;
      ev.preventDefault();
      return;
    case "ArrowLeft":
      this.keys.left = false;
      ev.preventDefault();
      return;
    case "ArrowRight":
      this.keys.right = false;
      ev.preventDefault();
      return;
    case "Space":
      this._spacePan = false;
      ev.preventDefault();
      return;
    case "ShiftLeft":
    case "ShiftRight":
      if (typeof this.state.releaseCommandTargetShift === "function") {
        this.state.releaseCommandTargetShift();
      }
      ev.preventDefault();
      return;
    case "KeyA":
      if (typeof this.state.releaseCommandTargetKey === "function") {
        this.state.releaseCommandTargetKey("KeyA", ev.shiftKey);
      } else if (this.state.commandTarget === "attack") {
        this.state.endCommandTarget();
      }
      ev.preventDefault();
      return;
    default:
      if (this.state.commandTarget && typeof this.state.releaseCommandTargetKey === "function") {
        this.state.releaseCommandTargetKey(ev.code, ev.shiftKey);
      }
      return;
  }
}

export function _handleBlur() {
  if (this.pointerLocked) this.exitPointerLock();
  this.keys.up = this.keys.down = this.keys.left = this.keys.right = false;
  this.mouse = null;
  this._spacePan = false;
  if (typeof this.state.endCommandTarget === "function") this.state.endCommandTarget();
  this._panDrag = null;
  if (this._drag) {
    this._drag = null;
    this._dragging = false;
    this.renderer.drawSelectionBox(null);
  }
}

export function _handleWheel(ev) {
  ev.preventDefault();
  const p = this._screenPos(ev);
  // Anchor the zoom on the cursor; setZoom clamps zoom AND re-clamps x/y so we
  // never reveal void outside the map near an edge.
  const factor = ev.deltaY < 0 ? 1 + ZOOM_STEP : 1 / (1 + ZOOM_STEP);
  this.camera.setZoom(this.camera.zoom * factor, p.x, p.y);
}
