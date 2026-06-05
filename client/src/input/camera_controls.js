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

  if (ev.repeat) return;
  if (this._activateCommandHotkey(ev)) return;

  switch (ev.code) {
    case "KeyA":
      this._enterAttackMove();
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
    default:
      return;
  }
}

export function _handleBlur() {
  if (this.pointerLocked) this.exitPointerLock();
  this.keys.up = this.keys.down = this.keys.left = this.keys.right = false;
  this.mouse = null;
  this._spacePan = false;
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
