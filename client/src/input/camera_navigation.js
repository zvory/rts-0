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
    this._installed = false;

    this._onMouseDown = this.handleMouseDown.bind(this);
    this._onMouseMove = this.handleMouseMove.bind(this);
    this._onMouseUp = this.handleMouseUp.bind(this);
    this._onMouseLeave = this.handleMouseLeave.bind(this);
    this._onWheel = this.handleWheel.bind(this);
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
    this.window.removeEventListener("keydown", this._onKeyDown, WINDOW_KEY_EVENT_OPTIONS);
    this.window.removeEventListener("keyup", this._onKeyUp, WINDOW_KEY_EVENT_OPTIONS);
    this.window.removeEventListener("blur", this._onBlur);
    this._installed = false;
    this.release();
  }

  handleMouseDown(ev, point = this.screenPos(ev)) {
    this.trackMouse(point);
    if (ev.button !== 1 && !(this.enableSpacePan && ev.button === 0 && this.spacePan)) return false;
    this.startPanDrag(point, ev.button);
    ev.preventDefault?.();
    return true;
  }

  handleMouseMove(ev, point = this.screenPos(ev)) {
    this.trackMouse(point);
    if (!this.panDrag) return false;
    this.camera?.panByScreenDelta?.(point.x - this.panDrag.x, point.y - this.panDrag.y);
    this.panDrag.x = point.x;
    this.panDrag.y = point.y;
    ev.preventDefault?.();
    return true;
  }

  handleMouseUp(ev) {
    if (!this.panDrag || ev.button !== this.panDrag.button) return false;
    this.panDrag = null;
    ev.preventDefault?.();
    return true;
  }

  handleMouseLeave() {
    this.mouse = null;
  }

  handleWheel(ev) {
    if (!this.camera || typeof this.camera.setZoom !== "function") return false;
    ev.preventDefault?.();
    const point = this.screenPos(ev);
    this.trackMouse(point);
    const factor = ev.deltaY < 0 ? 1 + ZOOM_STEP : 1 / (1 + ZOOM_STEP);
    this.camera.setZoom(this.camera.zoom * factor, point.x, point.y);
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
}
