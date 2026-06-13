import { CameraNavigationInput } from "./input/camera_navigation.js";

export class ReplayCameraInput {
  constructor(domElement, camera = null) {
    this.cameraNavigation = new CameraNavigationInput(domElement, camera, {
      installListeners: true,
      panKeyCodes: CameraNavigationInput.replayPanKeyCodes(),
    });
    this.keys = this.cameraNavigation.keys;
    Object.defineProperty(this, "mouse", {
      configurable: true,
      get: () => this.cameraNavigation.mouse,
      set: (value) => { this.cameraNavigation.mouse = value; },
    });
  }

  update() {}

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
  }
}
