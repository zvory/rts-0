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
//     Esc cancels placement/targeting; A and S also fall back to attack-move and stop.
//   - Mouse wheel = camera zoom toward the cursor.
//   - Arrow-key pan state is OWNED here and exposed via `this.keys` so the camera can
//     read it in Camera.update(dt, input) — see the `keys` field documentation below.
//   - Middle-drag or Space+left-drag pans the camera without using build hotkeys.
//
// All world hit-testing goes through camera.screenToWorld. Entities are hit-tested
// against the interpolated positions from state so clicks line up with what is drawn.

import { cmd, PASSABLE, isUnit, isBuilding, isResource, KIND } from "./protocol.js";
import { STATS } from "./config.js";

/**
 * Translates raw DOM pointer/keyboard gestures on the viewport into selection
 * mutations (on `state`) and protocol commands (via `net.command`).
 */
export class Input {
  /**
   * @param {HTMLElement} domElement the #viewport element that receives listeners
   * @param {import("./camera.js").Camera} camera world<->screen transforms & zoom
   * @param {import("./state.js").GameState} state selection + placement + entities
   * @param {import("./net.js").Net} net command sender
   * @param {import("./renderer.js").Renderer} renderer for drawSelectionBox
   * @param {import("./fog.js").Fog} fog kept for parity / future hit-test filtering
   */
  constructor(domElement, camera, state, net, renderer, fog) {
    this.dom = domElement;
    this.camera = camera;
    this.state = state;
    this.net = net;
    this.renderer = renderer;
    this.fog = fog;

    /**
     * Continuous pan-key state, read by Camera.update(dt, input). Booleans for the
     * four cardinal directions; the camera maps these to a pan velocity. Arrow keys
     * feed the flags. This is the shared input-state object the
     * design refers to (DESIGN.md §4.1 camera/input seam).
     * @type {{up:boolean,down:boolean,left:boolean,right:boolean}}
     */
    this.keys = { up: false, down: false, left: false, right: false };

    /**
     * Last known cursor position in screen (viewport-local) pixels. Used by update()
     * for placement hover and by edge logic the camera may consult.
     * @type {{x:number,y:number}}
     */
    this.mouse = { x: 0, y: 0 };

    // Active left-drag selection box, in screen pixels, or null when not dragging.
    // { x0, y0, x1, y1 } where (x0,y0) is the press anchor.
    this._drag = null;
    // Whether the current left press has moved far enough to count as a box drag.
    this._dragging = false;
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
    if (this.state.placement) this._refreshPlacement();
  }

  // --- Coordinate helpers -------------------------------------------------

  /** Cursor position relative to the viewport element, in CSS pixels. */
  _screenPos(ev) {
    const r = this.dom.getBoundingClientRect();
    return { x: ev.clientX - r.left, y: ev.clientY - r.top };
  }

  /** World point under the current screen cursor. */
  _worldAt(sx, sy) {
    return this.camera.screenToWorld(sx, sy);
  }

  // --- Mouse: press / move / release --------------------------------------

  _handleMouseDown(ev) {
    const p = this._screenPos(ev);
    this.mouse = p;
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
    this.mouse = p;

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
  }

  _handleMouseUp(ev) {
    if (this._panDrag && ev.button === this._panDrag.button) {
      this._panDrag = null;
      ev.preventDefault();
      return;
    }
    if (ev.button !== 0) return;
    const p = this._screenPos(ev);
    this.mouse = p;
    if (!this._drag) return;

    const wasDragging = this._dragging;
    const drag = this._drag;
    this._drag = null;
    this._dragging = false;
    this.renderer.drawSelectionBox(null);

    if (wasDragging) {
      this._commitBoxSelection(drag, ev.shiftKey);
    } else {
      this._commitClickSelection(p, ev.shiftKey);
    }
  }

  _handleContextMenu(ev) {
    // Always suppress the native menu over the viewport; treat as a right-click.
    ev.preventDefault();
    const p = this._screenPos(ev);
    this.mouse = p;
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
   * preferred), or clear when clicking empty space. Shift adds to the selection.
   */
  _commitClickSelection(p, additive) {
    const world = this._worldAt(p.x, p.y);
    const hit = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ true);
    if (!hit) {
      if (!additive) this.state.clearSelection();
      return;
    }
    if (additive) this.state.addToSelection([hit.id]);
    else this.state.setSelection([hit.id]);
  }

  /**
   * Box release: select all OWN units fully/partly inside the box. If the box
   * captured no units, fall back to OWN buildings inside it. Shift adds.
   */
  _commitBoxSelection(drag, additive) {
    const a = this._worldAt(Math.min(drag.x0, drag.x1), Math.min(drag.y0, drag.y1));
    const b = this._worldAt(Math.max(drag.x0, drag.x1), Math.max(drag.y0, drag.y1));
    const minX = Math.min(a.x, b.x);
    const maxX = Math.max(a.x, b.x);
    const minY = Math.min(a.y, b.y);
    const maxY = Math.max(a.y, b.y);

    const entities = this.state.entitiesInterpolated(1);
    const me = this.state.playerId;

    const units = [];
    const buildings = [];
    for (const e of entities) {
      if (e.owner !== me) continue;
      if (!this._entityIntersectsRect(e, minX, minY, maxX, maxY)) continue;
      if (isUnit(e.kind)) units.push(e.id);
      else if (isBuilding(e.kind)) buildings.push(e.id);
    }

    const picked = units.length > 0 ? units : buildings;
    if (picked.length === 0) {
      if (!additive) this.state.clearSelection();
      return;
    }
    if (additive) this.state.addToSelection(picked);
    else this.state.setSelection(picked);
  }

  // --- Right-button logic (context-sensitive orders) ----------------------

  _onRightClick(p) {
    // During placement, right-click cancels.
    if (this.state.placement) {
      this.state.endPlacement();
      return;
    }
    // Right-click also cancels a pending command-card target (consistent with Esc).
    if (this.state.commandTarget) {
      this.state.endCommandTarget();
      return;
    }

    const ownUnits = this._selectedOwnUnitIds();
    if (ownUnits.length === 0) return; // nothing own selected -> ignore

    const world = this._worldAt(p.x, p.y);
    const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
    const me = this.state.playerId;

    if (target && target.owner !== me && target.owner !== 0 && !isResource(target.kind)) {
      // Enemy entity -> attack.
      this.net.command(cmd.attack(ownUnits, target.id));
      this.state.addCommandFeedback("attack", target.x, target.y);
      return;
    }
    if (target && isResource(target.kind)) {
      // Resource node -> gather, but only with the workers in the selection.
      const workers = this._selectedWorkerIds();
      if (workers.length > 0) {
        this.net.command(cmd.gather(workers, target.id));
        return;
      }
      // Selection has no workers: fall through to a move onto the node's position.
    }
    // Default -> move to the world point.
    this.net.command(cmd.move(ownUnits, world.x, world.y));
    this.state.addCommandFeedback("move", world.x, world.y);
  }

  _issueTargetedCommand(p) {
    const ownUnits = this._selectedOwnUnitIds();
    if (ownUnits.length === 0) return;
    const world = this._worldAt(p.x, p.y);
    if (this.state.commandTarget === "move") {
      this.net.command(cmd.move(ownUnits, world.x, world.y));
      this.state.addCommandFeedback("move", world.x, world.y);
      return;
    }

    const target = this._entityAtWorld(world.x, world.y, /*ownPreferred=*/ false);
    const me = this.state.playerId;
    if (target && target.owner !== me && target.owner !== 0 && !isResource(target.kind)) {
      this.net.command(cmd.attack(ownUnits, target.id));
      this.state.addCommandFeedback("attack", target.x, target.y);
      return;
    }

    this.net.command(cmd.attackMove(ownUnits, world.x, world.y));
    this.state.addCommandFeedback("attack", world.x, world.y);
  }

  // --- Selection queries --------------------------------------------------

  /** Ids of currently-selected entities owned by us that are units. */
  _selectedOwnUnitIds() {
    const me = this.state.playerId;
    return this.state
      .selectedEntities()
      .filter((e) => e.owner === me && isUnit(e.kind))
      .map((e) => e.id);
  }

  /** Ids of currently-selected own workers (subset used for gather/build). */
  _selectedWorkerIds() {
    const me = this.state.playerId;
    return this.state
      .selectedEntities()
      .filter((e) => e.owner === me && e.kind === KIND.WORKER)
      .map((e) => e.id);
  }

  // --- Entity hit-testing -------------------------------------------------

  /**
   * Pick the entity at a world point. Units/resources are tested against a circular
   * render radius (config STATS[kind].size); buildings against their footprint box.
   * When `ownPreferred`, a hit on an own entity wins over an overlapping foreign one,
   * and among equals the closest center is chosen. Forgiving by design (small pad).
   * @returns {object|null} the interpolated entity, or null.
   */
  _entityAtWorld(wx, wy, ownPreferred) {
    const entities = this.state.entitiesInterpolated(1);
    const me = this.state.playerId;
    const tileSize = this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE;

    let best = null;
    let bestScore = Infinity; // lower is better (distance, with ownership tiebreak)
    for (const e of entities) {
      if (!this._worldPointHitsEntity(e, wx, wy, tileSize)) continue;
      const dx = wx - e.x;
      const dy = wy - e.y;
      const dist = Math.hypot(dx, dy);
      // Bias toward own entities when requested by subtracting a large bonus.
      const ownBonus = ownPreferred && e.owner === me ? OWN_HIT_BONUS : 0;
      const score = dist - ownBonus;
      if (score < bestScore) {
        bestScore = score;
        best = e;
      }
    }
    return best;
  }

  /** True if a world point falls within an entity's hit area (circle or footprint). */
  _worldPointHitsEntity(e, wx, wy, tileSize) {
    const stat = STATS[e.kind];
    if (isBuilding(e.kind)) {
      const halfW = ((stat && stat.footW ? stat.footW : 1) * tileSize) / 2;
      const halfH = ((stat && stat.footH ? stat.footH : 1) * tileSize) / 2;
      return (
        wx >= e.x - halfW - HIT_PAD_PX &&
        wx <= e.x + halfW + HIT_PAD_PX &&
        wy >= e.y - halfH - HIT_PAD_PX &&
        wy <= e.y + halfH + HIT_PAD_PX
      );
    }
    const radius = (stat && stat.size ? stat.size : DEFAULT_HIT_RADIUS) + HIT_PAD_PX;
    return Math.hypot(wx - e.x, wy - e.y) <= radius;
  }

  /** True if an entity's hit area intersects an axis-aligned world rect. */
  _entityIntersectsRect(e, minX, minY, maxX, maxY) {
    const tileSize = this.state.map ? this.state.map.tileSize : DEFAULT_TILE_SIZE;
    const stat = STATS[e.kind];
    let halfW;
    let halfH;
    if (isBuilding(e.kind)) {
      halfW = ((stat && stat.footW ? stat.footW : 1) * tileSize) / 2;
      halfH = ((stat && stat.footH ? stat.footH : 1) * tileSize) / 2;
    } else {
      halfW = halfH = stat && stat.size ? stat.size : DEFAULT_HIT_RADIUS;
    }
    // Box-vs-box overlap (entity AABB vs selection rect).
    return (
      e.x + halfW >= minX &&
      e.x - halfW <= maxX &&
      e.y + halfH >= minY &&
      e.y - halfH <= maxY
    );
  }

  // --- Build placement ----------------------------------------------------

  /**
   * Recompute the hovered tile + validity from the current cursor and push it to
   * the renderer ghost via state.updatePlacement. Called on every move and each
   * frame while placement is active.
   */
  _refreshPlacement() {
    const place = this.state.placement;
    if (!place) return;
    const map = this.state.map;
    if (!map) return;

    const world = this._worldAt(this.mouse.x, this.mouse.y);
    const stat = STATS[place.building];
    const footW = stat && stat.footW ? stat.footW : 1;
    const footH = stat && stat.footH ? stat.footH : 1;

    // Snap so the footprint is centered on the cursor (top-left tile of the footprint).
    const tileX = Math.floor(world.x / map.tileSize - footW / 2 + 0.5);
    const tileY = Math.floor(world.y / map.tileSize - footH / 2 + 0.5);
    const valid = this._footprintValid(tileX, tileY, footW, footH, map);
    this.state.updatePlacement(tileX, tileY, valid);
  }

  /**
   * A footprint is valid when every tile it covers is in-bounds and passable,
   * and no existing entity (unit or building) occupies the same world area.
   */
  _footprintValid(tileX, tileY, footW, footH, map) {
    if (tileX < 0 || tileY < 0) return false;
    if (tileX + footW > map.width || tileY + footH > map.height) return false;
    for (let ty = tileY; ty < tileY + footH; ty++) {
      for (let tx = tileX; tx < tileX + footW; tx++) {
        const code = map.terrain[ty * map.width + tx];
        if (!PASSABLE[code]) return false;
      }
    }
    const ts = map.tileSize;
    const minX = tileX * ts;
    const minY = tileY * ts;
    const maxX = (tileX + footW) * ts;
    const maxY = (tileY + footH) * ts;
    for (const e of this.state.entitiesInterpolated(1)) {
      if (this._entityIntersectsRect(e, minX, minY, maxX, maxY)) return false;
    }
    return true;
  }

  /**
   * Confirm a build placement: if the current ghost is valid and we have a worker
   * selected, send cmd.build with that worker, then exit placement mode. Invalid
   * clicks are ignored (placement stays active so the player can reposition).
   */
  _confirmPlacement() {
    const place = this.state.placement;
    if (!place || !place.valid) return;
    const workers = this._selectedWorkerIds();
    if (workers.length === 0) {
      // No worker to build with; abandon placement rather than send a dead command.
      this.state.endPlacement();
      return;
    }
    const worker = workers[0];
    this.net.command(cmd.build(worker, place.building, place.tileX, place.tileY));
    this.state.endPlacement();
  }

  // --- Keyboard -----------------------------------------------------------

  _handleKeyDown(ev) {
    // Never hijack typing in inputs (lobby name field, etc.).
    if (isTextEntry(ev.target)) return;

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

  _handleKeyUp(ev) {
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

  /** Window blur: release all pan keys so the camera doesn't drift while away. */
  _handleBlur() {
    this.keys.up = this.keys.down = this.keys.left = this.keys.right = false;
    this._spacePan = false;
    this._panDrag = null;
    if (this._drag) {
      this._drag = null;
      this._dragging = false;
      this.renderer.drawSelectionBox(null);
    }
  }

  _activateCommandHotkey(ev) {
    const key = commandHotkeyFromEvent(ev);
    if (!key) return false;
    const card = document.getElementById("command-card");
    if (!card) return false;
    for (const btn of card.querySelectorAll("button[data-hotkey]")) {
      if ((btn.dataset.hotkey || "").toUpperCase() !== key) continue;
      ev.preventDefault();
      if (!btn.disabled) btn.click();
      return true;
    }
    return false;
  }

  _enterAttackMove() {
    // Only meaningful when own units are selected; otherwise it's a no-op arming.
    if (this._selectedOwnUnitIds().length === 0) return;
    this.state.beginCommandTarget("attack");
  }

  _issueStop() {
    const ownUnits = this._selectedOwnUnitIds();
    if (ownUnits.length === 0) return;
    this.net.command(cmd.stop(ownUnits));
  }

  /** Esc / right-click cancel: drop placement first, then targeting. */
  _cancel() {
    if (this.state.placement) {
      this.state.endPlacement();
      return;
    }
    this.state.endCommandTarget();
  }

  // --- Mouse wheel: zoom toward cursor ------------------------------------

  _handleWheel(ev) {
    ev.preventDefault();
    const p = this._screenPos(ev);
    // Anchor the zoom on the cursor; setZoom clamps zoom AND re-clamps x/y so we
    // never reveal void outside the map near an edge.
    const factor = ev.deltaY < 0 ? 1 + ZOOM_STEP : 1 / (1 + ZOOM_STEP);
    this.camera.setZoom(this.camera.zoom * factor, p.x, p.y);
  }
}

// --- Tunables & small helpers ---------------------------------------------

// Pixels the cursor must travel before a press becomes a box-drag (vs a click).
const DRAG_THRESHOLD_PX = 4;
// Forgiving extra padding around entity hit areas, in world px.
const HIT_PAD_PX = 3;
// Large distance bonus so an own entity always beats an overlapping foreign one.
const OWN_HIT_BONUS = 1e6;
// Fallbacks when an entity kind has no STATS entry (defensive; shouldn't happen).
const DEFAULT_HIT_RADIUS = 10;
const DEFAULT_TILE_SIZE = 32;
// Wheel zoom multiplier per notch.
const ZOOM_STEP = 0.12;

/** True if the event target is an editable text field we must not steal keys from. */
function isTextEntry(el) {
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || el.isContentEditable === true;
}

/** Command-card hotkeys are single letter keys, matched against button data-hotkey. */
function commandHotkeyFromEvent(ev) {
  if (!ev || typeof ev.code !== "string" || !ev.code.startsWith("Key")) return "";
  return ev.code.slice(3).toUpperCase();
}
