import { CameraNavigationInput } from "./input/camera_navigation.js";
import { ObserverSelectionInput } from "./input/observer_selection.js";

export class ReplayCameraInput {
  constructor(domElement, camera = null, state = null) {
    this.observerSelection = state ? new ObserverSelectionInput(state) : null;
    this.screenOverlay = this.observerSelection?.screenOverlay || null;
    this.cameraNavigation = new CameraNavigationInput(domElement, camera, {
      installListeners: true,
      panKeyCodes: CameraNavigationInput.replayPanKeyCodes(),
      onUnconsumedMouseDown: (ev, point) => this.observerSelection?.handleMouseDown(ev, point),
      onUnconsumedMouseMove: (ev, point) => this.observerSelection?.handleMouseMove(ev, point),
      onUnconsumedMouseUp: (ev, point) => this.observerSelection?.handleMouseUp(ev, point),
    });
    this.keys = this.cameraNavigation.keys;
    Object.defineProperty(this, "mouse", {
      configurable: true,
      get: () => this.cameraNavigation.mouse,
      set: (value) => { this.cameraNavigation.mouse = value; },
    });
  }

  update() {}

  publishSelectionScene(scene) {
    return this.observerSelection?.publishSelectionScene(scene) ?? false;
  }

  handleMouseMove(ev) {
    return this.cameraNavigation.handleMouseMove(ev);
  }

  handleMouseLeave() {
    return this.cameraNavigation.handleMouseLeave();
  }

  handleWheel(ev) {
    return this.cameraNavigation.handleWheel(ev);
  }

  handleKeyDown(ev) {
    return this.cameraNavigation.handleKeyDown(ev);
  }

  handleKeyUp(ev) {
    return this.cameraNavigation.handleKeyUp(ev);
  }

  releaseKeys() {
    this.cameraNavigation.release();
  }

  pointerLockSupported() {
    return false;
  }

  installedAppRuntime() {
    return false;
  }

  destroy() {
    this.cameraNavigation.destroy();
    this.observerSelection?.destroy();
  }
}
