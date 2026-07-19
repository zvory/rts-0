import { ABILITY } from "../protocol.js";
import { ZOOM_STEP } from "./constants.js";
import { commandHotkeyCodeFromEvent, isTextEntry } from "./placement.js";

export function _handleKeyDown(ev) {
  if (ev.code === "ShiftLeft" || ev.code === "ShiftRight") {
    this._shiftKeysDown?.add(ev.code);
    this._shiftKeyDown = true;
  }
  // Never hijack typing in inputs (lobby name field, etc.). Modifier state still
  // needs to stay current if focus changes while the key remains held.
  if (ev.isComposing || isTextEntry(ev.target)) return;

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
      // Cursor lock should not steal Esc from the normal gameplay/UI cancel stack.
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

  if (activateGlobalHotkey(this, ev)) return;

  const commandHotkey = this._activateCommandHotkey(ev);
  if (commandHotkey) {
    if (commandHotkey.armed?.quickCast) {
      this._quickCastCommandTarget(ev);
    }
    const intent = clientIntent(this);
    const repeatedWorldAbilityTarget = repeatedWorldAbilityHotkeyTarget(intent?.commandTarget);
    if (
      intent?.commandTarget &&
      typeof intent.holdCommandTarget === "function" &&
      (ev.shiftKey || repeatedWorldAbilityTarget)
    ) {
      intent.holdCommandTarget(intent.commandTarget, ev.code, ev.shiftKey, {
        preserveTapOnRelease: repeatedWorldAbilityTarget && !ev.shiftKey,
      });
    }
    return;
  }
  if (ev.repeat) return;
  if (this._handleControlGroupHotkey(ev)) return;
}

function activateGlobalHotkey(input, ev) {
  if (ev.repeat || ev.altKey || ev.ctrlKey || ev.metaKey) return false;
  const code = commandHotkeyCodeFromEvent(ev);
  if (!code) return false;
  for (const action of input.globalHotkeyActions || []) {
    const resolved = input.hotkeyProfiles?.hotkeyCodeForCommand?.(action.commandId) || "";
    if (resolved !== code) continue;
    action.activate?.();
    ev.preventDefault?.();
    return true;
  }
  return false;
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
      this._shiftKeysDown?.delete(ev.code);
      this._shiftKeyDown = this._shiftKeysDown
        ? this._shiftKeysDown.size > 0
        : !!ev.shiftKey;
      if (this._shiftKeyDown) {
        ev.preventDefault();
        return;
      }
      if (typeof clientIntent(this)?.releaseCommandTargetShift === "function") {
        clientIntent(this).releaseCommandTargetShift();
      }
      if (clientIntent(this)?.placement && typeof clientIntent(this).endPlacement === "function") {
        this._cancelPlacementDrag?.();
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
  this._cancelFormationGesture?.();
  if (this.pointerLocked) this.exitPointerLock();
  this._shiftKeysDown?.clear();
  this._shiftKeyDown = false;
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
    this._cancelPlacementDrag?.();
    clientIntent(this).endPlacement();
  }
  if (this._drag) {
    this._drag = null;
    this._dragging = false;
    this.screenOverlay?.clearMarquee?.();
  }
}

function clientIntent(input) {
  return input?.clientIntent || null;
}

export function _handleWheel(ev) {
  if (this.pointerLocked && this.inputRouter?.wheel && typeof this._eventScreenPos === "function") {
    const p = this._eventScreenPos(ev);
    if (this.inputRouter.wheel(this._routedPointerEvent(ev, p, "locked"))) return;
  }
  if (this.cameraNavigation) {
    this.cameraNavigation.handleWheel(ev);
    return;
  }
  ev.preventDefault();
  const p = this._screenPos(ev);
  const factor = ev.deltaY < 0 ? 1 + ZOOM_STEP : 1 / (1 + ZOOM_STEP);
  this.camera?.dollyBy?.(factor, p);
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
