// Input — mouse/keyboard -> selection, protocol commands, and build placement.
// See DESIGN.md §4.1 (export contract) and the gameplay rules below.
//
// Responsibilities:
//   - Left-click / left-drag selection box (own units preferred; buildings as a
//     fallback when no units are captured). Shift adds to the current selection.
//   - Context-sensitive right-click on a selection of own units:
//       enemy entity   -> cmd.attack
//       resource node (+ workers in selection) -> cmd.gather
//       otherwise      -> cmd.move (to a world point)
//   - Build placement mode (started by the HUD via state.beginPlacement): track the
//     hovered tile, validate the footprint, drive the renderer ghost via
//     state.updatePlacement, confirm with a valid left-click, cancel with right/Esc.
//   - Keyboard: command-card grid hotkeys (QWE/ASD/ZXC) activate buttons directly;
//     Esc cancels placement/targeting; S also falls back to stop when no card button is active.
//   - Mouse wheel = camera zoom toward the cursor.
//   - Arrow-key pan state is OWNED here and exposed via `this.keys` so the camera can
//     read it in Camera.update(dt, input) — see the `keys` field documentation below.
//   - Middle-drag or Space+left-drag pans the camera without using build hotkeys.
//
// All world hit-testing goes through camera.screenToWorld. Entities are hit-tested
// against the interpolated positions from state so clicks line up with what is drawn.

import { DRAG_THRESHOLD_PX } from "./constants.js";
import {
  _activateCommandHotkey,
  _cancel,
  _enterAttackMove,
  _issueStop,
  _issueTargetedCommand,
  _nearestOwnCompletedCityCentre,
  _onRightClick,
  _refreshAtGunSetupPreview,
  _refreshResourceMiningPreview,
  _selectedOwnAtGunIds,
  _selectedOwnUnitIds,
  _selectedProducerBuildingIds,
  _selectedWorkerIds,
} from "./commands.js";
import { _handleBlur, _handleKeyDown, _handleKeyUp, _handleWheel } from "./camera_controls.js";
import {
  _confirmPlacement,
  _footprintValid,
  _refreshPlacement,
  footprintValidAgainstEntities,
} from "./placement.js";
import {
  _closestIdsToPoint,
  _closestOwnUnitKindInViewport,
  _commitBoxSelection,
  _commitClickSelection,
  _entityAtWorld,
  _entityIntersectsRect,
  _resourceAtWorld,
  _ownBuildingsOfKindInViewport,
  _worldPointHitsEntity,
} from "./selection.js";

export { footprintValidAgainstEntities };

/**
 * Translates raw DOM pointer/keyboard gestures on the viewport into selection
 * mutations (on `state`) and protocol commands (via `net.command`).
 */
export class Input {
  /**
   * @param {HTMLElement} domElement the #viewport element that receives listeners
   * @param {import("../camera.js").Camera} camera world<->screen transforms & zoom
   * @param {import("../state.js").GameState} state selection + placement + entities
   * @param {import("../net.js").Net} net command sender
   * @param {import("../renderer/index.js").Renderer} renderer for drawSelectionBox
   * @param {import("../fog.js").Fog} fog kept for parity / future hit-test filtering
   * @param {import("../audio.js").Audio} [audio] optional audio engine for local cues
   */
  constructor(domElement, camera, state, net, renderer, fog, audio) {
    this.dom = domElement;
    this.camera = camera;
    this.state = state;
    this.net = net;
    this.renderer = renderer;
    this.fog = fog;
    this.audio = audio || null;

    /**
     * Continuous pan-key state, read by Camera.update(dt, input). Booleans for the
     * four cardinal directions; the camera maps these to a pan velocity. Arrow keys
     * feed the flags. This is the shared input-state object the
     * design refers to (DESIGN.md §4.1 camera/input seam).
     * @type {{up:boolean,down:boolean,left:boolean,right:boolean}}
     */
    this.keys = { up: false, down: false, left: false, right: false };

    /**
     * Last known cursor position in screen (viewport-local) pixels, or null when
     * the pointer has not entered the viewport. Used by update() for placement
     * hover and by edge logic the camera may consult.
     * @type {{x:number,y:number}|null}
     */
    this.mouse = null;

    // Active left-drag selection box, in screen pixels, or null when not dragging.
    // { x0, y0, x1, y1 } where (x0,y0) is the press anchor.
    this._drag = null;
    // Whether the current left press has moved far enough to count as a box drag.
    this._dragging = false;
    // Last completed single click: { x, y, t } in screen pixels + timestamp ms.
    this._lastClick = null;
    // Space held: left-drag pans instead of selecting/placing.
    this._spacePan = false;
    // Active direct camera pan, in screen pixels, or null when not panning.
    // { x, y, button } where button is the pointer button that started the pan.
    this._panDrag = null;

    // Bound handlers retained so destroy() can remove the exact references.
    this._onMouseDown = this._handleMouseDown.bind(this);
    this._onMouseMove = this._handleMouseMove.bind(this);
    this._onMouseUp = this._handleMouseUp.bind(this);
    this._onContextMenu = this._handleContextMenu.bind(this);
    this._onAuxClick = this._handleAuxClick.bind(this);
    this._onWheel = this._handleWheel.bind(this);
    this._onKeyDown = this._handleKeyDown.bind(this);
    this._onKeyUp = this._handleKeyUp.bind(this);
    this._onBlur = this._handleBlur.bind(this);

    this._install();
  }

  // --- Lifecycle ----------------------------------------------------------

  _install() {
    const el = this.dom;
    el.addEventListener("mousedown", this._onMouseDown);
    // Move/up on window so a drag that leaves the viewport still tracks & releases.
    window.addEventListener("mousemove", this._onMouseMove);
    window.addEventListener("mouseup", this._onMouseUp);
    el.addEventListener("contextmenu", this._onContextMenu);
    el.addEventListener("auxclick", this._onAuxClick);
    el.addEventListener("wheel", this._onWheel, { passive: false });
    window.addEventListener("keydown", this._onKeyDown);
    window.addEventListener("keyup", this._onKeyUp);
    window.addEventListener("blur", this._onBlur);
  }

  /** Remove all installed listeners (e.g. on game teardown / screen change). */
  destroy() {
    const el = this.dom;
    el.removeEventListener("mousedown", this._onMouseDown);
    window.removeEventListener("mousemove", this._onMouseMove);
    window.removeEventListener("mouseup", this._onMouseUp);
    el.removeEventListener("contextmenu", this._onContextMenu);
    el.removeEventListener("auxclick", this._onAuxClick);
    el.removeEventListener("wheel", this._onWheel);
    window.removeEventListener("keydown", this._onKeyDown);
    window.removeEventListener("keyup", this._onKeyUp);
    window.removeEventListener("blur", this._onBlur);
  }

  /**
   * Per-frame continuous work. Pan-key handling lives on the camera (it reads
   * `this.keys`); placement hover is refreshed here so the ghost tracks the cursor
   * even when the mouse is still and only the camera is moving.
   * @param {number} dt seconds since last frame (unused today; kept for the main loop)
   */
  update(dt) {
    void dt;
    if (this.state.placement) {
      this.state.updateResourceMiningPreview(null);
      this.state.updateAtGunSetupPreview(null);
      this._refreshPlacement();
      return;
    }
    if (this.state.commandTarget === "setupAtGuns") {
      this.state.updateResourceMiningPreview(null);
      this._refreshAtGunSetupPreview();
      return;
    }
    this.state.updateAtGunSetupPreview(null);
    this._refreshResourceMiningPreview();
  }

  // --- Coordinate helpers -------------------------------------------------

  /** Cursor position relative to the viewport element, in CSS pixels. */
  _screenPos(ev) {
    const r = this.dom.getBoundingClientRect();
    return { x: ev.clientX - r.left, y: ev.clientY - r.top };
  }

  /** True when a viewport-local point is inside the viewport bounds. */
  _insideViewport(p) {
    return p.x >= 0 && p.y >= 0 && p.x <= this.dom.clientWidth && p.y <= this.dom.clientHeight;
  }

  /** Update the camera-facing mouse position from a viewport-local point. */
  _trackMouse(p) {
    this.mouse = this._insideViewport(p) ? p : null;
  }

  /** World point under the current screen cursor, clamped to map bounds. */
  _worldAt(sx, sy) {
    const w = this.camera.screenToWorld(sx, sy);
    const map = this.state.map;
    if (map) {
      const maxX = map.width * map.tileSize;
      const maxY = map.height * map.tileSize;
      w.x = Math.max(0, Math.min(maxX - 1, w.x));
      w.y = Math.max(0, Math.min(maxY - 1, w.y));
    }
    return w;
  }

  // --- Mouse: press / move / release --------------------------------------

  _handleMouseDown(ev) {
    const p = this._screenPos(ev);
    this._trackMouse(p);
    if (ev.button === 1 || (ev.button === 0 && this._spacePan)) {
      this._startPanDrag(p, ev.button);
      ev.preventDefault();
      return;
    }
    if (ev.button === 0) {
      this._onLeftDown(p, ev);
    }
    // Right (button 2) is handled on contextmenu so we also suppress the menu.
  }

  _handleMouseMove(ev) {
    const p = this._screenPos(ev);
    this._trackMouse(p);

    if (this._panDrag) {
      this.camera.panByScreenDelta(p.x - this._panDrag.x, p.y - this._panDrag.y);
      this._panDrag.x = p.x;
      this._panDrag.y = p.y;
      ev.preventDefault();
      if (this.state.placement) this._refreshPlacement();
      return;
    }

    if (this._drag) {
      this._drag.x1 = p.x;
      this._drag.y1 = p.y;
      // Promote to a real box once the cursor has moved past a small threshold.
      if (!this._dragging && this._dragDistance() >= DRAG_THRESHOLD_PX) {
        this._dragging = true;
      }
      if (this._dragging) {
        this.renderer.drawSelectionBox(this._normalizedDragRect());
      }
    }

    if (this.state.placement) this._refreshPlacement();
    else this._refreshResourceMiningPreview();
  }

  _handleMouseUp(ev) {
    if (this._panDrag && ev.button === this._panDrag.button) {
      this._panDrag = null;
      ev.preventDefault();
      return;
    }
    if (ev.button !== 0) return;
    const p = this._screenPos(ev);
    this._trackMouse(p);
    if (!this._drag) return;

    const wasDragging = this._dragging;
    const drag = this._drag;
    this._drag = null;
    this._dragging = false;
    this.renderer.drawSelectionBox(null);

    if (wasDragging) {
      this._lastClick = null;
      this._commitBoxSelection(drag, ev.shiftKey);
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
    // Always suppress the native menu over the viewport; treat as a right-click.
    ev.preventDefault();
    const p = this._screenPos(ev);
    this._trackMouse(p);
    this._onRightClick(p);
  }

  _handleAuxClick(ev) {
    if (ev.button === 1) ev.preventDefault();
  }

  // --- Left-button logic --------------------------------------------------

  _onLeftDown(p, ev) {
    // Build placement: a valid left-click confirms the build with a selected worker.
    if (this.state.placement) {
      this._confirmPlacement();
      return;
    }
    // Command-card targeting: the next left-click issues the armed command.
    if (this.state.commandTarget) {
      this._issueTargetedCommand(p);
      this.state.endCommandTarget();
      return;
    }
    // Otherwise begin a (possible) selection drag from this anchor.
    this._drag = { x0: p.x, y0: p.y, x1: p.x, y1: p.y };
    this._dragging = false;
    void ev;
  }

  _startPanDrag(p, button) {
    this._panDrag = { x: p.x, y: p.y, button };
  }

  _dragDistance() {
    const dx = this._drag.x1 - this._drag.x0;
    const dy = this._drag.y1 - this._drag.y0;
    return Math.hypot(dx, dy);
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
  /** All own buildings of `kind` whose center lies in the viewport. Cap mirrors unit ctrl-click. */
  /** Up to 12 own units of `kind` in the viewport, closest to `anchor`. */
  /**
   * Box release: select all OWN units fully/partly inside the box. If the box
   * captured no units, fall back to OWN buildings inside it. Shift adds.
   */
  /** Return up to 12 ids from `ids`, ordered by distance to the screen anchor. */
  // --- Right-button logic (context-sensitive orders) ----------------------

  // --- Selection queries --------------------------------------------------

  /** Ids of currently-selected entities owned by us that are units. */
  /** Ids of currently-selected own unit-producing buildings (eligible for rally points). */
  /** Ids of currently-selected own workers (subset used for gather/build). */
  /** Ids of currently-selected own AT guns. */
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
   * the renderer ghost via state.updatePlacement. Called on every move and each
   * frame while placement is active.
   */
  /**
   * A footprint is valid when every tile it covers is in-bounds and passable,
   * and no existing entity (unit or building) occupies the same world area.
   */
  /**
   * Confirm a build placement: if the current ghost is valid and we have a worker
   * selected, send cmd.build with that worker, then exit placement mode. Invalid
   * clicks are ignored (placement stays active so the player can reposition).
   */
  // --- Keyboard -----------------------------------------------------------

  /** Window blur: release all pan keys so the camera doesn't drift while away. */
  /** Esc cancel: drop placement first, then targeting, then selection. */
  // --- Mouse wheel: zoom toward cursor ------------------------------------

}

Object.assign(Input.prototype, {
  _commitClickSelection,
  _ownBuildingsOfKindInViewport,
  _closestOwnUnitKindInViewport,
  _commitBoxSelection,
  _closestIdsToPoint,
  _onRightClick,
  _issueTargetedCommand,
  _selectedOwnUnitIds,
  _selectedProducerBuildingIds,
  _selectedWorkerIds,
  _selectedOwnAtGunIds,
  _refreshAtGunSetupPreview,
  _refreshResourceMiningPreview,
  _nearestOwnCompletedCityCentre,
  _entityAtWorld,
  _resourceAtWorld,
  _worldPointHitsEntity,
  _entityIntersectsRect,
  _refreshPlacement,
  _footprintValid,
  _confirmPlacement,
  _handleKeyDown,
  _handleKeyUp,
  _handleBlur,
  _activateCommandHotkey,
  _enterAttackMove,
  _issueStop,
  _cancel,
  _handleWheel,
});
