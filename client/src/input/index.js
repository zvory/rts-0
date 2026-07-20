// Input — mouse/keyboard -> selection, protocol commands, and build placement.
// See docs/design/client-ui.md §4.1 (export contract) and the gameplay rules below.
//
// Responsibilities:
//   - Left-click / left-drag selection box (own units preferred; buildings as a
//     fallback when no units are captured). Shift adds to the current selection.
//   - Context-sensitive right-click on a selection of own units:
//       attackable enemy entity -> cmd.attack
//       resource node (+ workers in selection) -> cmd.gather
//       otherwise      -> cmd.move (to a world point)
//   - Build placement mode (started by the HUD via ClientIntent): track the
//     hovered tile, validate the footprint, drive the renderer ghost via
//     ClientIntent.updatePlacement, confirm with a valid left-click, cancel with right/Esc.
//   - Keyboard: rendered command-card hotkeys activate buttons directly; custom
//     profiles change the key labels and matching. Production train/cancel buttons
//     honor native key repeat; Esc cancels placement/targeting.
//     Number keys recall control groups; double-tap jumps to the densest visible
//     cluster. Alt/Ctrl/Cmd+number replaces a group, and Shift+number adds to it;
//     on Windows, browser saves use Alt+number; standalone apps accept Alt/Ctrl/Cmd.
//   - Mouse wheel = camera zoom toward the cursor.
//   - Arrow-key pan state is OWNED here and exposed via `this.keys` so the camera can
//     read it in Camera.update(dt, input) — see the `keys` field documentation below.
//   - Middle-drag or Space+left-drag pans the camera without using build hotkeys.
//   - Touch drag/pinch pans and zooms the camera for mobile viewing; viewport
//     touches do not issue gameplay commands.
//   - Optional pointer-lock mode traps the browser cursor and drives a visible
//     virtual cursor so multi-monitor players can still edge-pan and click.
//
// Ground interaction and entity picking consume the last successfully presented
// SelectionScene so input always targets the pixels the player can actually see.

import { DRAG_THRESHOLD_PX } from "./constants.js";
import { isBuilding, isUnit } from "../protocol.js";
import {
  _browserExitPointerLockFn,
  _browserPointerLockElement,
  _browserPointerLockSupported,
  _browserRequestPointerLock,
  _elementDebugSummary,
  _exitBrowserPointerLock,
  _finishPointerLockRequest,
  _focusDebugState,
  _focusPointerLockTarget,
  _handlePointerLockChange,
  _handlePointerLockError,
  _pointerLockErrorSummary,
  _reportPointerLockFailure,
  _pointerLockTarget,
  _requestBrowserPointerLock,
  _requestBrowserPointerLockWithOptions,
  _waitForBrowserPointerLockResult,
  _waitForPointerLockPromise,
  pointerLockDebugSnapshot,
} from "./browser_pointer_lock.js";
import { _recordPointerLockTrace } from "./pointer_lock_diagnostics.js";
import {
  _activateCommandHotkey,
  _cancel,
  _issueTargetedCommand,
  _nearestCompletedMiningAnchor,
  _onRightClick,
  _quickCastCommandTarget,
  _refreshAttackTargetPreview,
  _refreshAbilityTargetPreview,
  _refreshAntiTankGunSetupPreview,
  _refreshResourceMiningPreview,
  _selectedGathererIds,
  _selectedOwnAntiTankGunIds,
  _selectedOwnLandUnitIds,
  _selectedOwnUnitIds,
  _selectedProducerBuildingIds,
  _selectedWorkerIds,
} from "./commands.js";
import { _handleBlur, _handleKeyDown, _handleKeyUp, _handleWheel } from "./camera_controls.js";
import { CameraNavigationInput } from "./camera_navigation.js";
import { ScreenOverlay } from "./screen_overlay.js";
import {
  _confirmPlacement,
  _beginTankTrapPlacementDrag,
  _cancelPlacementDrag,
  _confirmTankTrapLinePlacement,
  _footprintValid,
  _finishTankTrapPlacementDrag,
  _refreshPlacement,
  footprintValidAgainstEntities,
} from "./placement.js";
import {
  _controlGroupSlotFromKey,
  _handleControlGroupHotkey,
  _jumpToControlGroupCluster,
} from "./control_groups.js";
import {
  clearPostQuickCastSelectionGuard,
  consumePostQuickCastSelectionGuard,
  postQuickCastSelectionGuardActiveAt,
} from "./quick_cast_selection_guard.js";
import {
  _beginLabToolClick,
  _cancelActiveLabTool,
  _cancelLabToolForBoxSelect,
  _consumeLabToolWorldClick,
  _finishLabToolBoxSelection,
  _finishLabToolClick,
  _paintLabToolStroke,
  _refreshLabToolPreview,
} from "./lab_tools.js";
import {
  _handleNativeCursorEvent,
  _installNativeCursorBridge,
  _nativeCursorBounds,
  _nativePointerEvent,
  _setNativeCursorPoint,
  configureNativeCursorBounds,
} from "./native_cursor.js";
import { nativeDesktopCursorBridge } from "./cursor_lock.js";
import {
  _beginFormationGesture,
  _cancelFormationGesture,
  _finishFormationGesture,
  _refreshFormationGesture,
  _updateFormationGesture,
} from "./formation_gesture.js";
import {
  _prepareCursorLock,
  _setCursorLockState,
  exitPointerLock,
  installedAppRuntime,
  pointerLockSupported,
  requestPointerLock,
  togglePointerLock,
} from "./pointer_lock_controller.js";
import {
  _closestOwnUnitKindInViewport,
  _commitBoxSelection,
  _commitClickSelection,
  _dragGroundCoverage,
  _entityAtScreen,
  _groundAtScreen,
  _ownBuildingsOfKindInViewport,
  _resourceAtScreen,
  _selectionEntities,
  _selectionEntityById,
  _visibleSelectionIds,
  publishSelectionScene,
} from "./selection.js";

export { footprintValidAgainstEntities };

const CONTEXT_MENU_EVENT_OPTIONS = { capture: true };
const TOUCH_EVENT_OPTIONS = { passive: false };
const CONTEXT_MENU_SUPPRESS_MS = 500;

function isMacPlatform() {
  const nav = globalThis.navigator;
  const platform = `${nav?.userAgentData?.platform || nav?.platform || ""}`;
  if (/\bMac/i.test(platform)) return true;
  return /\bMacintosh\b|\bMac OS X\b/i.test(`${nav?.userAgent || ""}`);
}

/**
 * Translates raw DOM pointer/keyboard gestures on the viewport into selection
 * mutations (on `state`) and protocol commands (via `commandInteraction.issueCommand`).
 */
export class Input {
  /**
   * @param {HTMLElement} domElement the #viewport element that receives listeners
   * @param {import("../camera.js").Camera} camera world<->screen transforms & zoom
   * @param {import("../state.js").GameState} state selection + entities
   * @param {{issueCommand(command: object, options?:object): object|boolean}} commandInteraction gameplay command interaction.
   * @param {(rect:object|null)=>void} drawMarquee backend screen-overlay operation
   * @param {import("../fog.js").Fog} fog kept for parity / future hit-test filtering
   * @param {import("../audio.js").Audio} [audio] optional audio engine for local cues
   * @param {import("./router.js").MatchInputRouter} [inputRouter] optional UI input router
   * @param {import("../hotkey_profiles.js").HotkeyProfileService} [hotkeyProfiles] active hotkey profile service.
   * @param {import("../client_intent.js").ClientIntent} [clientIntent] browser-local command/placement intent facade.
   * @param {object} [labToolController] active lab setup tool callback seam.
   * @param {{commandId:string,activate:()=>boolean}[]} [globalHotkeyActions] injected non-command-card actions.
   * @param {object} [desktopCursor] optional native desktop cursor bridge injected by the shell.
   * @param {object} [controlPolicy] read-only ownership and command-surface projection.
   */
  constructor(
    domElement,
    camera,
    state,
    commandInteraction,
    drawMarquee,
    fog,
    audio,
    inputRouter = null,
    hotkeyProfiles = null,
    clientIntent = null,
    labToolController = null,
    globalHotkeyActions = [],
    desktopCursor = null,
    controlPolicy = null,
  ) {
    this.dom = domElement;
    this.renderElement = domElement.querySelector?.("canvas") || null;
    this.camera = camera;
    this.state = state;
    this.commandInteraction = commandInteraction;
    this.controlPolicy = controlPolicy;
    this.screenOverlay = new ScreenOverlay(drawMarquee);
    this.fog = fog;
    this.audio = audio || null;
    this.inputRouter = inputRouter;
    this.hotkeyProfiles = hotkeyProfiles;
    this.clientIntent = clientIntent;
    this.labToolController = labToolController;
    this.desktopCursor = desktopCursor || nativeDesktopCursorBridge();
    this.globalHotkeyActions = Array.isArray(globalHotkeyActions) ? globalHotkeyActions : [];

    this.cameraNavigation = new CameraNavigationInput(domElement, camera);
    this.keys = this.cameraNavigation.keys;
    Object.defineProperty(this, "mouse", {
      configurable: true,
      get: () => this.cameraNavigation.mouse,
      set: (value) => { this.cameraNavigation.mouse = value; },
    });
    Object.defineProperty(this, "_spacePan", {
      configurable: true,
      get: () => this.cameraNavigation.spacePan,
      set: (value) => { this.cameraNavigation.spacePan = !!value; },
    });
    Object.defineProperty(this, "_panDrag", {
      configurable: true,
      get: () => this.cameraNavigation.panDrag,
      set: (value) => { this.cameraNavigation.panDrag = value; },
    });

    // Active left-drag selection box, in screen pixels, or null when not dragging.
    // { x0, y0, x1, y1 } where (x0,y0) is the press anchor.
    this._drag = null;
    this._placementDrag = null;
    this._formationGesture = null;
    // Whether the current left press has moved far enough to count as a box drag.
    this._dragging = false;
    // Last completed single click: { x, y, t } in screen pixels + timestamp ms.
    this._lastClick = null;
    // One-shot selection suppression after an unqueued quick-cast at the cursor.
    this._postQuickCastSelectionGuard = null;
    // Current Shift modifier state for hover previews that need queued-command semantics.
    this._shiftKeyDown = false;
    this._shiftKeysDown = new Set();
    // Last recalled control-group slot for number-key double-tap camera jumps.
    this._lastControlGroupTap = null;
    // Cursor-lock state. While locked, `this.mouse` is a viewport-local virtual
    // cursor updated from movementX/movementY and drawn above the canvas.
    this.pointerLocked = false;
    this._cursorLockMode = null;
    this._pointerLockCursor = null;
    this._pendingPointerLockCursor = null;
    this._suppressNextContextMenuUntil = 0;
    this._pointerLockAttempt = 0;
    this._lastPointerLockFocusAttempt = null;
    this._lastPointerLockRequest = null;
    this._lastPointerLockFailureAttempt = null;
    this._pointerLockRequestInFlight = null;
    this._pointerLockTrace = [];
    this._pointerLockTraceSequence = 0;
    this._pointerLockShellLog = { attempted: 0, succeeded: 0, failed: 0, lastError: null };
    this._nativeButtonsMask = 0;
    this.onPointerLockChange = null;
    this.onPointerLockError = null;

    // Bound handlers retained so destroy() can remove the exact references.
    this._onMouseDown = this._handleMouseDown.bind(this);
    this._onMouseMove = this._handleMouseMove.bind(this);
    this._onMouseUp = this._handleMouseUp.bind(this);
    this._onContextMenu = this._handleContextMenu.bind(this);
    this._onAuxClick = this._handleAuxClick.bind(this);
    this._onWheel = this._handleWheel.bind(this);
    this._onTouchStart = this._handleTouchStart.bind(this);
    this._onTouchMove = this._handleTouchMove.bind(this);
    this._onTouchEnd = this._handleTouchEnd.bind(this);
    this._onTouchCancel = this._handleTouchCancel.bind(this);
    this._onKeyDown = this._handleKeyDown.bind(this);
    this._onKeyUp = this._handleKeyUp.bind(this);
    this._onBlur = this._handleBlur.bind(this);
    this._onPointerLockChange = this._handlePointerLockChange.bind(this);
    this._onPointerLockError = this._handlePointerLockError.bind(this);
    this._onNativeCursorEvent = this._handleNativeCursorEvent.bind(this);
    this._removeNativeCursorListener = null;

    this._install();
    this._installPointerLockCursor();
    this._installNativeCursorBridge();
  }

  // --- Lifecycle ----------------------------------------------------------

  _install() {
    const el = this.dom;
    if (!el.hasAttribute("tabindex")) el.tabIndex = -1;
    const lockTarget = this._pointerLockTarget();
    if (lockTarget !== el && typeof lockTarget.hasAttribute === "function" && !lockTarget.hasAttribute("tabindex")) {
      lockTarget.tabIndex = -1;
    }
    el.addEventListener("mousedown", this._onMouseDown);
    // Move/up on window so a drag that leaves the viewport still tracks & releases.
    window.addEventListener("mousemove", this._onMouseMove);
    window.addEventListener("mouseup", this._onMouseUp);
    el.addEventListener("contextmenu", this._onContextMenu, CONTEXT_MENU_EVENT_OPTIONS);
    el.addEventListener("auxclick", this._onAuxClick);
    el.addEventListener("wheel", this._onWheel, { passive: false });
    el.addEventListener("touchstart", this._onTouchStart, TOUCH_EVENT_OPTIONS);
    window.addEventListener("touchmove", this._onTouchMove, TOUCH_EVENT_OPTIONS);
    window.addEventListener("touchend", this._onTouchEnd, TOUCH_EVENT_OPTIONS);
    window.addEventListener("touchcancel", this._onTouchCancel, TOUCH_EVENT_OPTIONS);
    window.addEventListener("keydown", this._onKeyDown);
    window.addEventListener("keyup", this._onKeyUp);
    window.addEventListener("blur", this._onBlur);
    document.addEventListener("pointerlockchange", this._onPointerLockChange);
    document.addEventListener("pointerlockerror", this._onPointerLockError);
    document.addEventListener("webkitpointerlockchange", this._onPointerLockChange);
    document.addEventListener("webkitpointerlockerror", this._onPointerLockError);
  }

  /** Remove all installed listeners (e.g. on game teardown / screen change). */
  destroy() {
    this._cancelFormationGesture();
    this.exitPointerLock();
    this.screenOverlay?.destroy?.();
    this.clientIntent?.clearPlannedOrders?.();
    const el = this.dom;
    el.removeEventListener("mousedown", this._onMouseDown);
    window.removeEventListener("mousemove", this._onMouseMove);
    window.removeEventListener("mouseup", this._onMouseUp);
    el.removeEventListener("contextmenu", this._onContextMenu, CONTEXT_MENU_EVENT_OPTIONS);
    el.removeEventListener("auxclick", this._onAuxClick);
    el.removeEventListener("wheel", this._onWheel);
    el.removeEventListener("touchstart", this._onTouchStart, TOUCH_EVENT_OPTIONS);
    window.removeEventListener("touchmove", this._onTouchMove, TOUCH_EVENT_OPTIONS);
    window.removeEventListener("touchend", this._onTouchEnd, TOUCH_EVENT_OPTIONS);
    window.removeEventListener("touchcancel", this._onTouchCancel, TOUCH_EVENT_OPTIONS);
    window.removeEventListener("keydown", this._onKeyDown);
    window.removeEventListener("keyup", this._onKeyUp);
    window.removeEventListener("blur", this._onBlur);
    document.removeEventListener("pointerlockchange", this._onPointerLockChange);
    document.removeEventListener("pointerlockerror", this._onPointerLockError);
    document.removeEventListener("webkitpointerlockchange", this._onPointerLockChange);
    document.removeEventListener("webkitpointerlockerror", this._onPointerLockError);
    if (this._removeNativeCursorListener) {
      this._removeNativeCursorListener();
      this._removeNativeCursorListener = null;
    }
    this.cameraNavigation.destroy();
    if (this._pointerLockCursor) {
      this._pointerLockCursor.remove();
      this._pointerLockCursor = null;
    }
  }

  _intent() {
    return this.clientIntent;
  }

  isShiftHeld() {
    return this._shiftKeyDown;
  }

  _commandTarget() {
    return this._intent()?.commandTarget;
  }

  _placement() {
    return this._intent()?.placement;
  }

  _labTool() {
    return this._intent()?.activeLabTool || null;
  }

  _addCommandFeedback(kind, x, y, append = false, radiusTiles = null) {
    const now = performance.now();
    if (kind === "mortar" && Number.isFinite(x) && Number.isFinite(y) && Array.isArray(this.state?.pendingMortarTargets)) {
      this.state.pendingMortarTargets.push({ x, y, createdAt: now });
      this.state.pendingMortarTargets = this.state.pendingMortarTargets.filter(
        (p) => now - p.createdAt <= 700,
      );
    }
    return this._intent()?.addCommandFeedback?.(
      kind,
      x,
      y,
      append,
      radiusTiles,
      now,
      commandFeedbackOwner(this.state, this.controlPolicy),
    );
  }

  /**
   * Per-frame continuous work. Pan-key handling lives on the camera (it reads
   * `this.keys`); placement hover is refreshed here so the ghost tracks the cursor
   * even when the mouse is still and only the camera is moving.
   * @param {number} dt seconds since last frame (unused today; kept for the main loop)
   */
  update(dt) {
    void dt;
    this._flushPointerLockCursor();
    if (this.inputRouter?.activePreviewSurface?.()) {
      this._cancelFormationGesture();
      return;
    }
    if (this._formationGesture?.promoted) {
      this._refreshFormationGesture();
      return;
    }
    if (this._labTool()) {
      this._intent()?.updateAttackTargetPreview?.(null);
      this._intent()?.updateResourceMiningPreview?.(null);
      this._intent()?.updateAntiTankGunSetupPreview?.(null);
      this._intent()?.updateAbilityTargetPreview?.(null);
      this._refreshLabToolPreview();
      return;
    }
    this._intent()?.updateLabToolPreview?.(null);
    if (this._placement()) {
      this._intent()?.updateAttackTargetPreview?.(null);
      this._intent()?.updateResourceMiningPreview?.(null);
      this._intent()?.updateAntiTankGunSetupPreview?.(null);
      this._refreshPlacement();
      return;
    }
    if (this._commandTarget() === "setupAntiTankGuns") {
      this._intent()?.updateAttackTargetPreview?.(null);
      this._intent()?.updateResourceMiningPreview?.(null);
      this._intent()?.updateAbilityTargetPreview?.(null);
      this._refreshAntiTankGunSetupPreview();
      return;
    }
    if (this._commandTarget()?.kind === "ability") {
      this._intent()?.updateAttackTargetPreview?.(null);
      this._intent()?.updateResourceMiningPreview?.(null);
      this._intent()?.updateAntiTankGunSetupPreview?.(null);
      this._refreshAbilityTargetPreview();
      return;
    }
    this._intent()?.updateAbilityTargetPreview?.(null);
    this._intent()?.updateAntiTankGunSetupPreview?.(null);
    this._refreshAttackTargetPreview();
    this._refreshResourceMiningPreview();
  }

  // --- Coordinate helpers -------------------------------------------------

  /** Cursor position relative to the viewport element, in CSS pixels. */
  _screenPos(ev) {
    if (this.cameraNavigation) return this.cameraNavigation.screenPos(ev);
    const r = this.dom.getBoundingClientRect();
    return { x: ev.clientX - r.left, y: ev.clientY - r.top };
  }

  /** Cursor position for gameplay: real browser cursor, or virtual cursor while locked. */
  _eventScreenPos(ev) {
    if (this.pointerLocked) return this.mouse || this._viewportCenter();
    return this._screenPos(ev);
  }

  /** True when a viewport-local point is inside the viewport bounds. */
  _insideViewport(p) {
    if (this.cameraNavigation) return this.cameraNavigation.insideViewport(p);
    return p.x >= 0 && p.y >= 0 && p.x <= this.dom.clientWidth && p.y <= this.dom.clientHeight;
  }

  /** Update the camera-facing mouse position from a viewport-local point. */
  _trackMouse(p) {
    if (this.cameraNavigation) this.cameraNavigation.trackMouse(p);
    else this.mouse = this._insideViewport(p) ? p : null;
  }

  _viewportCenter() {
    return { x: this.dom.clientWidth / 2, y: this.dom.clientHeight / 2 };
  }

  _clampViewportPoint(p) {
    return {
      x: Math.max(0, Math.min(this.dom.clientWidth, p.x)),
      y: Math.max(0, Math.min(this.dom.clientHeight, p.y)),
    };
  }

  _installPointerLockCursor() {
    const cursor = document.createElement("div");
    cursor.className = "pointer-lock-cursor";
    cursor.hidden = true;
    this.dom.appendChild(cursor);
    this._pointerLockCursor = cursor;
  }

  _setPointerLockCursor(p, { immediate = false } = {}) {
    if (!this._pointerLockCursor) return;
    if (immediate) {
      this._paintPointerLockCursor(p);
      this._pendingPointerLockCursor = null;
      return;
    }
    this._pendingPointerLockCursor = { x: p.x, y: p.y };
  }

  _paintPointerLockCursor(p) {
    if (!this._pointerLockCursor) return;
    this._pointerLockCursor.style.transform = `translate(${p.x}px, ${p.y}px)`;
  }

  _flushPointerLockCursor() {
    if (!this._pointerLockCursor || !this._pendingPointerLockCursor) return;
    const p = this._pendingPointerLockCursor;
    this._paintPointerLockCursor(p);
    this._pendingPointerLockCursor = null;
  }

  _lockedMovementDelta(ev) {
    return {
      x: Number.isFinite(ev.movementX) ? ev.movementX : 0,
      y: Number.isFinite(ev.movementY) ? ev.movementY : 0,
    };
  }

  _moveLockedCursor(delta) {
    const base = this.mouse || this._viewportCenter();
    const p = this._clampViewportPoint({
      x: base.x + delta.x,
      y: base.y + delta.y,
    });
    this.mouse = p;
    this._setPointerLockCursor(p);
    return p;
  }

  _routedPointerEvent(ev, p, source) {
    const rect = this.dom.getBoundingClientRect();
    return {
      viewportX: p.x,
      viewportY: p.y,
      clientX: rect.left + p.x,
      clientY: rect.top + p.y,
      button: Number.isFinite(p.button) ? p.button : ev.button,
      buttons: Number.isFinite(ev.buttons) ? ev.buttons : undefined,
      deltaX: Number.isFinite(ev.deltaX) ? ev.deltaX : 0,
      deltaY: Number.isFinite(ev.deltaY) ? ev.deltaY : 0,
      deltaMode: Number.isFinite(ev.deltaMode) ? ev.deltaMode : 0,
      shiftKey: ev.shiftKey,
      ctrlKey: ev.ctrlKey,
      metaKey: ev.metaKey,
      altKey: ev.altKey,
      source,
      originalEvent: ev,
    };
  }

  _routeLockedPointerDown(ev, p) {
    if (!this.pointerLocked || !this.inputRouter) return false;
    return this.inputRouter.pointerDown(this._routedPointerEvent(ev, p, "locked"));
  }

  _routeLockedPointerMove(ev, p) {
    if (!this.pointerLocked || !this.inputRouter) return false;
    return this.inputRouter.pointerMove(this._routedPointerEvent(ev, p, "locked"));
  }

  _routeLockedPointerUp(ev, p) {
    if (!this.pointerLocked || !this.inputRouter) return false;
    return this.inputRouter.pointerUp(this._routedPointerEvent(ev, p, "locked"));
  }

  // --- Mouse: press / move / release --------------------------------------

  _handleMouseDown(ev) {
    if (this.cameraNavigation?.shouldSuppressMouseEvent?.(ev)) return;
    const p = this._eventScreenPos(ev);
    if (!this.pointerLocked) this._trackMouse(p);
    if (ev.button === 2) {
      clearPostQuickCastSelectionGuard(this);
      this._suppressNextContextMenuUntil = performance.now() + CONTEXT_MENU_SUPPRESS_MS;
      if (!this._routeLockedPointerDown(ev, { ...p, button: 2 })) this._beginFormationGesture(p, ev);
      ev.preventDefault();
      ev.stopPropagation();
      return;
    }
    if (ev.button !== 0) clearPostQuickCastSelectionGuard(this);
    if (ev.button !== 2 && this._routeLockedPointerDown(ev, p)) {
      ev.preventDefault();
      return;
    }
    if (this.cameraNavigation ? this.cameraNavigation.handleMouseDown(ev, p) : this._handleCameraPanMouseDownFallback(ev, p)) {
      return;
    }
    if (ev.button === 0) {
      this._onLeftDown(p, ev);
    }
    // Right (button 2) is handled on contextmenu so we also suppress the menu.
  }

  _handleMouseMove(ev) {
    if (this.cameraNavigation?.shouldSuppressMouseEvent?.(ev)) return;
    let p;
    if (this.pointerLocked) {
      const delta = this._lockedMovementDelta(ev);
      if (delta.x === 0 && delta.y === 0 && !this._panDrag && !this._drag && !this._formationGesture) return;
      p = this._moveLockedCursor(delta);
    } else {
      p = this._screenPos(ev);
      this._trackMouse(p);
    }
    this._handlePointerMoveAt(ev, p);
  }

  _handlePointerMoveAt(ev, p) {
    if (this._routeLockedPointerMove(ev, p)) {
      ev.preventDefault();
      return;
    }

    if (this.cameraNavigation ? this.cameraNavigation.handleMouseMove(ev, p) : this._handleCameraPanMouseMoveFallback(ev, p)) {
      return;
    }

    if (this._formationGesture) {
      this._updateFormationGesture(p, ev);
      ev.preventDefault?.();
      return;
    }

    if (this._drag) {
      this._drag.x1 = p.x;
      this._drag.y1 = p.y;
      // Promote to a real box once the cursor has moved past a small threshold.
      if (!this._dragging && this._dragDistance() >= DRAG_THRESHOLD_PX) {
        this._dragging = true;
        if (!this._drag.labToolPaintsOnDrag) {
          this._cancelLabToolForBoxSelect();
          if (this._drag.suppressPostQuickCastSelection) {
            this._drag.suppressPostQuickCastSelection = false;
            clearPostQuickCastSelectionGuard(this);
          }
        }
      }
      if (this._dragging) {
        if (this._drag.labToolPaintsOnDrag) {
          this._paintLabToolStroke(this._drag, p, ev);
        } else {
          this.screenOverlay?.setMarquee?.(this._normalizedDragRect());
        }
      }
    }

    // Hover/placement/ability previews are refreshed once per animation frame
    // in update(); pointer-lock mousemove can arrive much faster than that.
  }

  _handleMouseUp(ev) {
    if (this.cameraNavigation?.shouldSuppressMouseEvent?.(ev)) return;
    if (this.cameraNavigation ? this.cameraNavigation.handleMouseUp(ev) : this._handleCameraPanMouseUpFallback(ev)) {
      return;
    }
    if (ev.button === 2) {
      const p = this._eventScreenPos(ev);
      if (!this.pointerLocked) this._trackMouse(p);
      if (this._routeLockedPointerUp(ev, p)) this._cancelFormationGesture();
      else this._finishFormationGesture(p, ev);
      ev.preventDefault?.();
      return;
    }
    if (ev.button !== 0 && this.pointerLocked) {
      const p = this._eventScreenPos(ev);
      if (this._routeLockedPointerUp(ev, p)) ev.preventDefault();
      return;
    }
    if (ev.button !== 0) return;
    if (this._formationGesture) {
      const p = this._eventScreenPos(ev);
      if (!this.pointerLocked) this._trackMouse(p);
      this._finishFormationGesture(p, ev);
      ev.preventDefault?.();
      return;
    }
    if (this._finishTankTrapPlacementDrag(ev)) return;
    const p = this._eventScreenPos(ev);
    if (!this.pointerLocked) this._trackMouse(p);
    if (this._routeLockedPointerUp(ev, p)) {
      ev.preventDefault();
      return;
    }
    if (!this._drag) return;

    const wasDragging = this._dragging;
    const drag = this._drag;
    this._drag = null;
    this._dragging = false;
    this.screenOverlay?.clearMarquee?.();

    if (wasDragging) {
      this._lastClick = null;
      if (drag.labToolPaintsOnDrag) {
        this._paintLabToolStroke(drag, p, ev);
        return;
      }
      if (drag.labToolId && this._finishLabToolBoxSelection(drag, ev)) return;
      this._commitBoxSelection(drag, ev.shiftKey);
    } else if (drag.labToolId) {
      this._finishLabToolClick(drag, p, ev);
    } else if (drag.suppressPostQuickCastSelection && consumePostQuickCastSelectionGuard(this, p)) {
      this._lastClick = null;
    } else {
      const now = performance.now();
      const last = this._lastClick;
      const isDouble = last &&
        (now - last.t) < 300 &&
        Math.hypot(p.x - last.x, p.y - last.y) < 5;
      this._lastClick = isDouble ? null : { x: p.x, y: p.y, t: now };
      this._commitClickSelection(p, ev.shiftKey, (ev.ctrlKey || ev.metaKey) || isDouble);
    }
  }

  _handleContextMenu(ev) {
    if (this.cameraNavigation?.shouldSuppressMouseEvent?.(ev)) {
      ev.stopPropagation();
      return;
    }
    clearPostQuickCastSelectionGuard(this);
    // Always suppress the native menu over the viewport; treat as a right-click.
    ev.preventDefault();
    ev.stopPropagation();
    if (performance.now() <= this._suppressNextContextMenuUntil) {
      this._suppressNextContextMenuUntil = 0;
      return;
    }
    if (this._formationGesture) return;
    const p = this._eventScreenPos(ev);
    if (!this.pointerLocked) this._trackMouse(p);
    if (this._routeLockedPointerDown(ev, { ...p, button: 2 })) return;
    if (this._handleMacControlClickSelection(p, ev)) return;
    this._onRightClick(p, ev);
  }

  _handleAuxClick(ev) {
    if (ev.button === 1) ev.preventDefault();
  }

  _handleTouchStart(ev) {
    this._cancelTouchGameplayState();
    this.cameraNavigation?.handleTouchStart(ev);
  }

  _handleTouchMove(ev) {
    this.cameraNavigation?.handleTouchMove(ev);
  }

  _handleTouchEnd(ev) {
    this.cameraNavigation?.handleTouchEnd(ev);
  }

  _handleTouchCancel(ev) {
    this._cancelTouchGameplayState();
    this.cameraNavigation?.handleTouchCancel(ev);
  }

  _cancelTouchGameplayState() {
    this._cancelFormationGesture();
    if (this._drag) {
      this._drag = null;
      this._dragging = false;
      this.screenOverlay?.clearMarquee?.();
    }
    this._placementDrag = null;
  }

  // --- Left-button logic --------------------------------------------------

  _onLeftDown(p, ev) {
    const activeLabTool = this._labTool();
    if (activeLabTool) {
      this._beginLabToolClick(p, ev, activeLabTool);
      return;
    }
    // Build placement: a valid left-click confirms the build with a selected worker.
    if (this._placement()) {
      clearPostQuickCastSelectionGuard(this);
      if (this._beginTankTrapPlacementDrag()) return;
      this._confirmPlacement(ev);
      return;
    }
    // Command-card targeting: the next left-click issues the armed command.
    if (this._commandTarget()) {
      clearPostQuickCastSelectionGuard(this);
      if (this._commandTarget() === "attack") {
        this._beginFormationGesture(p, ev, "attackMove");
        return;
      }
      if (this._issueTargetedCommand(p, ev) === false) return;
      const issued = typeof this._intent()?.issueCommandTarget === "function"
        ? this._intent().issueCommandTarget(ev)
        : { keepArmed: false };
      if (!issued.keepArmed) {
        this._intent()?.endCommandTarget?.();
      }
      return;
    }
    // Otherwise begin a (possible) selection drag from this anchor.
    const suppressPostQuickCastSelection = postQuickCastSelectionGuardActiveAt(this, p);
    this._drag = { x0: p.x, y0: p.y, x1: p.x, y1: p.y, suppressPostQuickCastSelection };
    this._dragging = false;
    void ev;
  }

  _handleMacControlClickSelection(p, ev) {
    if (!ev.ctrlKey || ev.metaKey || !isMacPlatform()) return false;
    if (this._placement() || this._commandTarget() || this._labTool()) return false;

    const hit = this._entityAtScreen(p, /*ownPreferred=*/ true);
    if (!hit) return false;
    const own = controllableOwner(this.state, hit.owner, this.controlPolicy);
    if (!own) return false;
    if (!isUnit(hit.kind) && !isBuilding(hit.kind)) return false;

    this._commitClickSelection(p, ev.shiftKey, true);
    return true;
  }

  _dragDistance() {
    const dx = this._drag.x1 - this._drag.x0;
    const dy = this._drag.y1 - this._drag.y0;
    return Math.hypot(dx, dy);
  }

  _handleCameraPanMouseDownFallback(ev, p) {
    if (ev.button !== 1 && !(ev.button === 0 && this._spacePan)) return false;
    this._panDrag = { x: p.x, y: p.y, button: ev.button };
    ev.preventDefault();
    return true;
  }

  _handleCameraPanMouseMoveFallback(ev, p) {
    if (!this._panDrag) return false;
    this.camera.panByScreenDelta({
      x: p.x - this._panDrag.x,
      y: p.y - this._panDrag.y,
    });
    this._panDrag.x = p.x;
    this._panDrag.y = p.y;
    ev.preventDefault();
    return true;
  }

  _handleCameraPanMouseUpFallback(ev) {
    if (!this._panDrag || ev.button !== this._panDrag.button) return false;
    this._panDrag = null;
    ev.preventDefault();
    return true;
  }

  /** Drag rect normalized to {x,y,w,h} in screen pixels (top-left origin). */
  _normalizedDragRect() {
    const d = this._drag;
    const x = Math.min(d.x0, d.x1);
    const y = Math.min(d.y0, d.y1);
    const w = Math.abs(d.x1 - d.x0);
    const h = Math.abs(d.y1 - d.y0);
    return { x, y, w, h };
  }

  /**
   * Single empty/entity click: select the one entity under the cursor (own
   * preferred), or clear when clicking empty space. Shift adds to the selection,
   * but shift-clicking an already-selected entity removes it from the selection.
   * Ctrl+click selects all own units of the same kind visible in the viewport.
   */
  /** All own buildings of `kind` whose center lies in the viewport. */
  /** Own units of `kind` in the viewport, closest to `anchor`. */
  /**
   * Box release: select all OWN units fully/partly inside the box. If the box
   * captured no units, fall back to OWN buildings inside it. Shift adds.
   */
  /** Return ids from `ids`, ordered by distance to the screen anchor. */
  // --- Right-button logic (context-sensitive orders) ----------------------

  // --- Selection queries --------------------------------------------------

  /** Ids of currently-selected entities owned by us that are units. */
  /** Ids of currently-selected own unit-producing buildings (eligible for rally points). */
  /** Ids of currently-selected own workers (subset used for gather/build). */
  /** Ids of currently-selected own anti-tank guns. */
  // --- Entity hit-testing -------------------------------------------------

  /**
   * Pick the entity at a world point. Infantry/resources are tested against a
   * circular render radius, vehicles against their oriented hull, and buildings
   * against their footprint box.
   * When `ownPreferred`, a hit on an own entity wins over an overlapping foreign one,
   * and among equals the closest center is chosen. Forgiving by design (small pad).
   * @returns {object|null} the interpolated entity, or null.
   */
  /** True if a world point falls within an entity's hit area (circle or footprint). */
  /** True if an entity's hit area intersects an axis-aligned world rect. */
  // --- Build placement ----------------------------------------------------

  /**
   * Recompute the hovered tile + validity from the current cursor and push it to
   * the renderer ghost via ClientIntent.updatePlacement. Called on every move and each
   * frame while placement is active.
   */
  /**
   * A footprint is valid when every tile it covers is in-bounds and passable,
   * and no existing entity (unit or building) occupies the same world area.
   */
  /**
   * Confirm a build placement: if the current ghost is valid and we have workers
   * selected, send cmd.build with those workers, then exit placement mode. Invalid
   * clicks are ignored (placement stays active so the player can reposition).
   */
  // --- Keyboard -----------------------------------------------------------

  /** Window blur: release all pan keys so the camera doesn't drift while away. */
  /** Esc cancel: drop placement first, then targeting, then selection. */
  /** Number-key control groups: save/add/recall, with recall double-tap camera jump. */
  // --- Mouse wheel: zoom toward cursor ------------------------------------

}

function commandFeedbackOwner(state, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    const owner = typeof controlPolicy.feedbackOwner === "function"
      ? controlPolicy.feedbackOwner(state)
      : typeof controlPolicy.issueAsOwnerForSelection === "function"
        ? controlPolicy.issueAsOwnerForSelection(state.selectedEntities?.() || [])
        : null;
    const ownerId = Number(owner);
    return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
  }
  const ownerId = Number(state?.playerId);
  return Number.isInteger(ownerId) && ownerId > 0 ? ownerId : null;
}

function controllableOwner(state, owner, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    if (typeof controlPolicy.isCommandOwner === "function") {
      return !!controlPolicy.isCommandOwner(owner, state);
    }
    return !!controlPolicy.canControlOwner?.(owner, state);
  }
  return typeof state?.isOwnOwner === "function"
    ? state.isOwnOwner(owner)
    : Number(owner) === state?.playerId;
}

Object.assign(Input.prototype, {
  _commitClickSelection,
  _ownBuildingsOfKindInViewport,
  _closestOwnUnitKindInViewport,
  _commitBoxSelection,
  publishSelectionScene,
  _groundAtScreen,
  _entityAtScreen,
  _resourceAtScreen,
  _selectionEntityById,
  _selectionEntities,
  _dragGroundCoverage,
  _visibleSelectionIds,
  _onRightClick,
  _issueTargetedCommand,
  _quickCastCommandTarget,
  _selectedOwnUnitIds,
  _selectedOwnLandUnitIds,
  _selectedProducerBuildingIds,
  _selectedGathererIds,
  _selectedWorkerIds,
  _selectedOwnAntiTankGunIds,
  _refreshAttackTargetPreview,
  _refreshAntiTankGunSetupPreview,
  _refreshAbilityTargetPreview,
  _refreshResourceMiningPreview,
  _nearestCompletedMiningAnchor,
  _refreshPlacement,
  _footprintValid,
  _confirmPlacement,
  _confirmTankTrapLinePlacement,
  _beginTankTrapPlacementDrag,
  _finishTankTrapPlacementDrag,
  _cancelPlacementDrag,
  _beginLabToolClick,
  _cancelActiveLabTool,
  _cancelLabToolForBoxSelect,
  _consumeLabToolWorldClick,
  _finishLabToolBoxSelection,
  _finishLabToolClick,
  _paintLabToolStroke,
  _refreshLabToolPreview,
  _installNativeCursorBridge,
  _nativeCursorBounds,
  configureNativeCursorBounds,
  _setNativeCursorPoint,
  _handleNativeCursorEvent,
  _nativePointerEvent,
  _controlGroupSlotFromKey,
  _handleControlGroupHotkey,
  _jumpToControlGroupCluster,
  _handleKeyDown,
  _handleKeyUp,
  _handleBlur,
  _browserPointerLockSupported,
  _pointerLockTarget,
  _browserRequestPointerLock,
  _browserExitPointerLockFn,
  _browserPointerLockElement,
  _elementDebugSummary,
  pointerLockDebugSnapshot,
  _focusPointerLockTarget,
  _focusDebugState,
  _requestBrowserPointerLock,
  _requestBrowserPointerLockWithOptions,
  _waitForPointerLockPromise,
  _finishPointerLockRequest,
  _pointerLockErrorSummary,
  _reportPointerLockFailure,
  _recordPointerLockTrace,
  _waitForBrowserPointerLockResult,
  _exitBrowserPointerLock,
  _handlePointerLockChange,
  _handlePointerLockError,
  pointerLockSupported,
  installedAppRuntime,
  _prepareCursorLock,
  requestPointerLock,
  exitPointerLock,
  togglePointerLock,
  _setCursorLockState,
  _activateCommandHotkey,
  _cancel,
  _handleWheel,
  _beginFormationGesture,
  _updateFormationGesture,
  _finishFormationGesture,
  _cancelFormationGesture,
  _refreshFormationGesture,
});
