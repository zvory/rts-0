import { ZOOM_STEP } from "./constants.js";
import { isTextEntry } from "./placement.js";

const DEFAULT_PAN_KEY_CODES = Object.freeze({
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
});

const REPLAY_PAN_KEY_CODES = Object.freeze({
  ...DEFAULT_PAN_KEY_CODES,
  KeyW: "up",
  KeyS: "down",
  KeyA: "left",
  KeyD: "right",
});

const WINDOW_KEY_EVENT_OPTIONS = true;
const TOUCH_EVENT_OPTIONS = { passive: false };
const SYNTHETIC_MOUSE_SUPPRESS_MS = 700;

export class CameraNavigationInput {
  constructor(domElement, camera, {
    installListeners = false,
    panKeyCodes = DEFAULT_PAN_KEY_CODES,
    enableSpacePan = true,
    windowRef = globalThis.window,
  } = {}) {
    this.domElement = domElement;
    this.camera = camera;
    this.panKeyCodes = panKeyCodes;
    this.enableSpacePan = enableSpacePan;
    this.window = windowRef;
    this.keys = { up: false, down: false, left: false, right: false };
    this.mouse = null;
    this.spacePan = false;
    this.panDrag = null;
    this.touchGesture = null;
    this.touchIds = new Set();
    this.suppressMouseUntil = 0;
    this._installed = false;

    this._onMouseDown = this.handleMouseDown.bind(this);
    this._onMouseMove = this.handleMouseMove.bind(this);
    this._onMouseUp = this.handleMouseUp.bind(this);
    this._onMouseLeave = this.handleMouseLeave.bind(this);
    this._onWheel = this.handleWheel.bind(this);
    this._onTouchStart = this.handleTouchStart.bind(this);
    this._onTouchMove = this.handleTouchMove.bind(this);
    this._onTouchEnd = this.handleTouchEnd.bind(this);
    this._onTouchCancel = this.handleTouchCancel.bind(this);
    this._onKeyDown = this.handleKeyDown.bind(this);
    this._onKeyUp = this.handleKeyUp.bind(this);
    this._onBlur = this.release.bind(this);

    if (installListeners) this.install();
  }

  static replayPanKeyCodes() {
    return REPLAY_PAN_KEY_CODES;
  }

  install() {
    if (this._installed) return;
    this.domElement.addEventListener("mousedown", this._onMouseDown);
    this.window.addEventListener("mousemove", this._onMouseMove);
    this.window.addEventListener("mouseup", this._onMouseUp);
    this.domElement.addEventListener("mouseleave", this._onMouseLeave);
    this.domElement.addEventListener("wheel", this._onWheel, { passive: false });
    this.domElement.addEventListener("touchstart", this._onTouchStart, TOUCH_EVENT_OPTIONS);
    this.window.addEventListener("touchmove", this._onTouchMove, TOUCH_EVENT_OPTIONS);
    this.window.addEventListener("touchend", this._onTouchEnd, TOUCH_EVENT_OPTIONS);
    this.window.addEventListener("touchcancel", this._onTouchCancel, TOUCH_EVENT_OPTIONS);
    this.window.addEventListener("keydown", this._onKeyDown, WINDOW_KEY_EVENT_OPTIONS);
    this.window.addEventListener("keyup", this._onKeyUp, WINDOW_KEY_EVENT_OPTIONS);
    this.window.addEventListener("blur", this._onBlur);
    this._installed = true;
  }

  destroy() {
    if (!this._installed) {
      this.release();
      return;
    }
    this.domElement.removeEventListener("mousedown", this._onMouseDown);
    this.window.removeEventListener("mousemove", this._onMouseMove);
    this.window.removeEventListener("mouseup", this._onMouseUp);
    this.domElement.removeEventListener("mouseleave", this._onMouseLeave);
    this.domElement.removeEventListener("wheel", this._onWheel);
    this.domElement.removeEventListener("touchstart", this._onTouchStart, TOUCH_EVENT_OPTIONS);
    this.window.removeEventListener("touchmove", this._onTouchMove, TOUCH_EVENT_OPTIONS);
    this.window.removeEventListener("touchend", this._onTouchEnd, TOUCH_EVENT_OPTIONS);
    this.window.removeEventListener("touchcancel", this._onTouchCancel, TOUCH_EVENT_OPTIONS);
    this.window.removeEventListener("keydown", this._onKeyDown, WINDOW_KEY_EVENT_OPTIONS);
    this.window.removeEventListener("keyup", this._onKeyUp, WINDOW_KEY_EVENT_OPTIONS);
    this.window.removeEventListener("blur", this._onBlur);
    this._installed = false;
    this.release();
  }

  handleMouseDown(ev, point = this.screenPos(ev)) {
    if (this.shouldSuppressMouseEvent(ev)) return true;
    this.trackMouse(point);
    if (ev.button !== 1 && !(this.enableSpacePan && ev.button === 0 && this.spacePan)) return false;
    this.startPanDrag(point, ev.button);
    ev.preventDefault?.();
    return true;
  }

  handleMouseMove(ev, point = this.screenPos(ev)) {
    if (this.shouldSuppressMouseEvent(ev)) return true;
    this.trackMouse(point);
    if (!this.panDrag) return false;
    this.camera?.panByScreenDelta?.({
      x: point.x - this.panDrag.x,
      y: point.y - this.panDrag.y,
    });
    this.panDrag.x = point.x;
    this.panDrag.y = point.y;
    ev.preventDefault?.();
    return true;
  }

  handleMouseUp(ev) {
    if (this.shouldSuppressMouseEvent(ev)) return true;
    if (!this.panDrag || ev.button !== this.panDrag.button) return false;
    this.panDrag = null;
    ev.preventDefault?.();
    return true;
  }

  handleMouseLeave() {
    this.mouse = null;
  }

  handleWheel(ev) {
    if (this.shouldSuppressMouseEvent(ev)) return true;
    if (!this.camera || typeof this.camera.dollyBy !== "function") return false;
    ev.preventDefault?.();
    const point = this.screenPos(ev);
    this.trackMouse(point);
    const factor = ev.deltaY < 0 ? 1 + ZOOM_STEP : 1 / (1 + ZOOM_STEP);
    this.camera.dollyBy(factor, point);
    return true;
  }

  handleTouchStart(ev) {
    if (!this.camera) return false;
    this.trackStartedTouches(ev);
    const points = this.touchPoints(ev.touches, { activeOnly: true });
    if (points.length <= 0) return false;
    this.suppressSyntheticMouse();
    this.startTouchGesture(points);
    ev.preventDefault?.();
    return true;
  }

  handleTouchMove(ev) {
    if (!this.touchGesture) return false;
    const points = this.touchPoints(ev.touches, { activeOnly: true });
    this.suppressSyntheticMouse();
    if (points.length <= 0) {
      this.finishTouchGesture();
    } else {
      this.updateTouchGesture(points);
    }
    ev.preventDefault?.();
    return true;
  }

  handleTouchEnd(ev) {
    if (!this.touchGesture) return false;
    this.releaseChangedTouches(ev);
    const points = this.touchPoints(ev.touches, { activeOnly: true });
    this.suppressSyntheticMouse();
    if (points.length > 0) this.startTouchGesture(points);
    else this.finishTouchGesture();
    ev.preventDefault?.();
    return true;
  }

  handleTouchCancel(ev) {
    if (!this.touchGesture) return false;
    this.suppressSyntheticMouse();
    this.finishTouchGesture();
    ev.preventDefault?.();
    return true;
  }

  handleKeyDown(ev) {
    return this.setKey(ev, true);
  }

  handleKeyUp(ev) {
    return this.setKey(ev, false);
  }

  setKey(ev, down) {
    if (isTextEntry(ev.target)) return false;
    if (this.enableSpacePan && ev.code === "Space") {
      this.spacePan = down;
      ev.preventDefault?.();
      return true;
    }
    const direction = this.panKeyCodes[ev.code];
    if (!direction) return false;
    this.keys[direction] = down;
    ev.preventDefault?.();
    return true;
  }

  release() {
    this.keys.up = false;
    this.keys.down = false;
    this.keys.left = false;
    this.keys.right = false;
    this.mouse = null;
    this.spacePan = false;
    this.panDrag = null;
    this.touchGesture = null;
    this.touchIds.clear();
  }

  screenPos(ev) {
    const rect = this.domElement.getBoundingClientRect();
    return { x: ev.clientX - rect.left, y: ev.clientY - rect.top };
  }

  trackMouse(point) {
    this.mouse = this.insideViewport(point) ? point : null;
  }

  insideViewport(point) {
    const width = Number.isFinite(this.domElement.clientWidth)
      ? this.domElement.clientWidth
      : this.domElement.getBoundingClientRect().width;
    const height = Number.isFinite(this.domElement.clientHeight)
      ? this.domElement.clientHeight
      : this.domElement.getBoundingClientRect().height;
    return point.x >= 0 && point.y >= 0 && point.x <= width && point.y <= height;
  }

  startPanDrag(point, button) {
    this.panDrag = { x: point.x, y: point.y, button };
  }

  startTouchGesture(points) {
    this.mouse = null;
    this.panDrag = null;
    if (points.length >= 2) {
      const center = midpoint(points[0], points[1]);
      this.touchGesture = {
        mode: "pinch",
        centerX: center.x,
        centerY: center.y,
        distance: distance(points[0], points[1]),
      };
      return;
    }
    this.touchGesture = { mode: "pan", x: points[0].x, y: points[0].y };
  }

  updateTouchGesture(points) {
    if (points.length >= 2) {
      this.updatePinchGesture(points[0], points[1]);
      return;
    }
    this.updateTouchPanGesture(points[0]);
  }

  updateTouchPanGesture(point) {
    if (this.touchGesture?.mode !== "pan") {
      this.startTouchGesture([point]);
      return;
    }
    this.camera?.panByScreenDelta?.({
      x: point.x - this.touchGesture.x,
      y: point.y - this.touchGesture.y,
    });
    this.touchGesture = { mode: "pan", x: point.x, y: point.y };
  }

  updatePinchGesture(a, b) {
    if (this.touchGesture?.mode !== "pinch") {
      this.startTouchGesture([a, b]);
      return;
    }
    const center = midpoint(a, b);
    const nextDistance = distance(a, b);
    this.camera?.panByScreenDelta?.({
      x: center.x - this.touchGesture.centerX,
      y: center.y - this.touchGesture.centerY,
    });
    if (
      nextDistance > 0 &&
      this.touchGesture.distance > 0 &&
      typeof this.camera?.dollyBy === "function"
    ) {
      const factor = nextDistance / this.touchGesture.distance;
      if (Number.isFinite(factor) && factor > 0) {
        this.camera.dollyBy(factor, center);
      }
    }
    this.touchGesture = {
      mode: "pinch",
      centerX: center.x,
      centerY: center.y,
      distance: nextDistance,
    };
  }

  finishTouchGesture() {
    this.touchGesture = null;
    this.mouse = null;
    this.touchIds.clear();
  }

  trackStartedTouches(ev) {
    const started = touchArray(ev.changedTouches);
    const touches = started.length > 0 ? started : touchArray(ev.touches);
    for (let i = 0; i < touches.length; i += 1) {
      const touch = touches[i];
      if (this.touchStartedInViewport(touch)) {
        this.touchIds.add(touchIdentifier(touch, i));
      }
    }
  }

  releaseChangedTouches(ev) {
    const ended = touchArray(ev.changedTouches);
    for (let i = 0; i < ended.length; i += 1) {
      this.touchIds.delete(touchIdentifier(ended[i], i));
    }
    if (ended.length === 0 && touchArray(ev.touches).length === 0) {
      this.touchIds.clear();
    }
  }

  touchStartedInViewport(touch) {
    const target = touch?.target;
    if (!target || typeof this.domElement.contains !== "function") return true;
    return target === this.domElement || this.domElement.contains(target);
  }

  touchPoints(touches, { activeOnly = false } = {}) {
    return touchArray(touches)
      .filter((touch, index) => !activeOnly || this.touchIds.has(touchIdentifier(touch, index)))
      .map((touch) => this.screenPos(touch))
      .filter((point) => Number.isFinite(point.x) && Number.isFinite(point.y));
  }

  suppressSyntheticMouse() {
    this.suppressMouseUntil = nowMs() + SYNTHETIC_MOUSE_SUPPRESS_MS;
  }

  shouldSuppressMouseEvent(ev) {
    if (nowMs() > this.suppressMouseUntil) return false;
    ev.preventDefault?.();
    return true;
  }
}

function midpoint(a, b) {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

function distance(a, b) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function touchArray(touches) {
  return Array.from(touches || []);
}

function touchIdentifier(touch, fallback) {
  return Number.isFinite(touch?.identifier) ? touch.identifier : fallback;
}

function nowMs() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}
