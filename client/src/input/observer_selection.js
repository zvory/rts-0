import { DRAG_THRESHOLD_PX } from "./constants.js";
import { ScreenOverlay } from "./screen_overlay.js";
import {
  _closestOwnUnitKindInViewport,
  _commitBoxSelection,
  _commitClickSelection,
  _entityAtScreen,
  _ownBuildingsOfKindInViewport,
  _selectionEntityById,
  _selectableProxiesInDragRect,
  publishSelectionScene,
} from "./selection.js";

/** Browser-local selection gestures for passive observers; exposes no command API. */
export class ObserverSelectionInput {
  constructor(state) {
    this.state = state;
    this.controlPolicy = null;
    this.selectionScene = null;
    this.screenOverlay = new ScreenOverlay(() => {});
    this.drag = null;
    this.dragging = false;
  }

  publishSelectionScene(scene) {
    return publishSelectionScene.call(this, scene);
  }

  handleMouseDown(ev, point) {
    if (ev.button !== 0) return false;
    this.drag = { x0: point.x, y0: point.y, x1: point.x, y1: point.y };
    this.dragging = false;
    return true;
  }

  handleMouseMove(_ev, point) {
    if (!this.drag) return false;
    this.drag.x1 = point.x;
    this.drag.y1 = point.y;
    if (!this.dragging && this._dragDistance() >= DRAG_THRESHOLD_PX) this.dragging = true;
    if (this.dragging) this.screenOverlay.setMarquee(this._normalizedDragRect());
    return true;
  }

  handleMouseUp(ev, point) {
    if (ev.button !== 0 || !this.drag) return false;
    this.drag.x1 = point.x;
    this.drag.y1 = point.y;
    const drag = this.drag;
    const dragging = this.dragging;
    this.drag = null;
    this.dragging = false;
    this.screenOverlay.clearMarquee();
    if (dragging) this._commitBoxSelection(drag, ev.shiftKey);
    else this._commitClickSelection(point, ev.shiftKey, ev.ctrlKey || ev.metaKey);
    return true;
  }

  destroy() {
    this.drag = null;
    this.dragging = false;
    this.screenOverlay.destroy();
  }

  _dragDistance() {
    return Math.hypot(this.drag.x1 - this.drag.x0, this.drag.y1 - this.drag.y0);
  }

  _normalizedDragRect() {
    return {
      x: Math.min(this.drag.x0, this.drag.x1),
      y: Math.min(this.drag.y0, this.drag.y1),
      w: Math.abs(this.drag.x1 - this.drag.x0),
      h: Math.abs(this.drag.y1 - this.drag.y0),
    };
  }

  _entityAtScreen(...args) {
    return _entityAtScreen.call(this, ...args);
  }

  _selectionEntityById(id) {
    return _selectionEntityById.call(this, id);
  }

  _closestOwnUnitKindInViewport(...args) {
    return _closestOwnUnitKindInViewport.call(this, ...args);
  }

  _ownBuildingsOfKindInViewport(kind) {
    return _ownBuildingsOfKindInViewport.call(this, kind);
  }

  _commitClickSelection(...args) {
    return _commitClickSelection.call(this, ...args);
  }

  _commitBoxSelection(...args) {
    return _commitBoxSelection.call(this, ...args);
  }

  _selectableProxiesInDragRect(...args) {
    return _selectableProxiesInDragRect.call(this, ...args);
  }
}
