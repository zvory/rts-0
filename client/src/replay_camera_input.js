import { ZOOM_STEP } from "./input/constants.js";

export class ReplayCameraInput {
  constructor(domElement, camera = null) {
    this.domElement = domElement;
    this.camera = camera;
    this.keys = { up: false, down: false, left: false, right: false };
    this.mouse = null;

    this.onMouseMove = this.handleMouseMove.bind(this);
    this.onMouseLeave = this.handleMouseLeave.bind(this);
    this.onWheel = this.handleWheel.bind(this);
    this.onKeyDown = this.handleKeyDown.bind(this);
    this.onKeyUp = this.handleKeyUp.bind(this);
    this.onBlur = this.releaseKeys.bind(this);

    domElement.addEventListener("mousemove", this.onMouseMove);
    domElement.addEventListener("mouseleave", this.onMouseLeave);
    domElement.addEventListener("wheel", this.onWheel, { passive: false });
    window.addEventListener("keydown", this.onKeyDown, true);
    window.addEventListener("keyup", this.onKeyUp, true);
    window.addEventListener("blur", this.onBlur);
  }

  update() {}

  handleMouseMove(ev) {
    const rect = this.domElement.getBoundingClientRect();
    this.mouse = {
      x: ev.clientX - rect.left,
      y: ev.clientY - rect.top,
    };
  }

  handleMouseLeave() {
    this.mouse = null;
  }

  handleWheel(ev) {
    if (!this.camera || typeof this.camera.setZoom !== "function") return;
    ev.preventDefault();
    const rect = this.domElement.getBoundingClientRect();
    const x = ev.clientX - rect.left;
    const y = ev.clientY - rect.top;
    const factor = ev.deltaY < 0 ? 1 + ZOOM_STEP : 1 / (1 + ZOOM_STEP);
    this.camera.setZoom(this.camera.zoom * factor, x, y);
  }

  handleKeyDown(ev) {
    this.setKey(ev, true);
  }

  handleKeyUp(ev) {
    this.setKey(ev, false);
  }

  setKey(ev, down) {
    if (isTextEntry(ev.target)) return;
    switch (ev.code) {
      case "ArrowUp":
      case "KeyW":
        this.keys.up = down;
        break;
      case "ArrowDown":
      case "KeyS":
        this.keys.down = down;
        break;
      case "ArrowLeft":
      case "KeyA":
        this.keys.left = down;
        break;
      case "ArrowRight":
      case "KeyD":
        this.keys.right = down;
        break;
      default:
        return;
    }
    ev.preventDefault();
  }

  releaseKeys() {
    this.keys.up = false;
    this.keys.down = false;
    this.keys.left = false;
    this.keys.right = false;
  }

  pointerLockSupported() {
    return false;
  }

  installedAppRuntime() {
    return false;
  }

  destroy() {
    this.domElement.removeEventListener("mousemove", this.onMouseMove);
    this.domElement.removeEventListener("mouseleave", this.onMouseLeave);
    this.domElement.removeEventListener("wheel", this.onWheel);
    window.removeEventListener("keydown", this.onKeyDown, true);
    window.removeEventListener("keyup", this.onKeyUp, true);
    window.removeEventListener("blur", this.onBlur);
    this.releaseKeys();
    this.mouse = null;
  }
}

function isTextEntry(target) {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
}
