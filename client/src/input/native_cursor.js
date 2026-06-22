export function _installNativeCursorBridge() {
  if (!this.desktopCursor || typeof this.desktopCursor.onEvent !== "function") return;
  this._removeNativeCursorListener = this.desktopCursor.onEvent(this._onNativeCursorEvent);
}

export function _nativeCursorBounds() {
  return {
    width: this.dom.clientWidth,
    height: this.dom.clientHeight,
  };
}

export function configureNativeCursorBounds() {
  if (this._cursorLockMode !== "native-macos" || typeof this.desktopCursor?.configure !== "function") return;
  void this.desktopCursor.configure(this._nativeCursorBounds());
}

export function _setNativeCursorPoint(detail) {
  const center = this._viewportCenter();
  const p = this._clampViewportPoint({
    x: Number.isFinite(detail?.x) ? detail.x : (this.mouse?.x ?? center.x),
    y: Number.isFinite(detail?.y) ? detail.y : (this.mouse?.y ?? center.y),
  });
  this.mouse = p;
  this._setPointerLockCursor(p, { immediate: true });
  return p;
}

export function _handleNativeCursorEvent(detail) {
  if (detail?.type === "capture" && detail.active === false) {
    if (this.pointerLocked && this._cursorLockMode === "native-macos") {
      this._setCursorLockState(false, null);
    }
    return;
  }
  if (!this.pointerLocked || this._cursorLockMode !== "native-macos") return;

  const p = this._setNativeCursorPoint(detail);
  const ev = this._nativePointerEvent(detail, p);
  switch (detail?.type) {
    case "move":
      this._handlePointerMoveAt(ev, p);
      return;
    case "down":
      this._handleMouseDown(ev);
      return;
    case "up":
      this._handleMouseUp(ev);
      return;
    case "wheel":
      this._handleWheel(ev);
      return;
    default:
      return;
  }
}

export function _nativePointerEvent(detail, p) {
  const rect = this.dom.getBoundingClientRect();
  const button = Number.isFinite(detail?.button) ? detail.button : 0;
  const deltaY = Number.isFinite(detail?.deltaY) ? detail.deltaY : 0;
  const deltaX = Number.isFinite(detail?.deltaX) ? detail.deltaX : 0;
  return {
    clientX: rect.left + p.x,
    clientY: rect.top + p.y,
    viewportX: p.x,
    viewportY: p.y,
    movementX: Number.isFinite(detail?.dx) ? detail.dx : 0,
    movementY: Number.isFinite(detail?.dy) ? detail.dy : 0,
    button,
    deltaX,
    deltaY,
    shiftKey: !!detail?.shiftKey,
    ctrlKey: !!detail?.ctrlKey,
    metaKey: !!detail?.metaKey,
    altKey: !!detail?.altKey,
    nativeCursor: true,
    preventDefault() {},
    stopPropagation() {},
  };
}
