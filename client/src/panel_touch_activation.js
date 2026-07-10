const DEFAULT_SYNTHETIC_CLICK_SUPPRESS_MS = 750;

export function createImmediateTouchButtonActivation(onActivate, options = {}) {
  const activate = typeof onActivate === "function" ? onActivate : () => {};
  const now = typeof options.now === "function" ? options.now : () => Date.now();
  const syntheticClickSuppressMs = finitePositive(options.syntheticClickSuppressMs) ||
    DEFAULT_SYNTHETIC_CLICK_SUPPRESS_MS;
  let activePointerId = null;
  let pendingSyntheticClick = null;

  function activateFromTouch(event) {
    if (!isTouchPointer(event) || activePointerId == null || !samePointerId(activePointerId, event)) return;
    activePointerId = null;
    pendingSyntheticClick = {
      at: now(),
      pointerType: String(event?.pointerType || ""),
    };
    event.preventDefault?.();
    event.stopPropagation?.();
    if (!releasedInside(event)) return;
    activate(event);
  }

  function cancelTouch(event) {
    if (activePointerId == null || !samePointerId(activePointerId, event)) return;
    activePointerId = null;
  }

  return {
    pointerdown(event) {
      if (!isTouchPointer(event) || !isPrimaryPointer(event)) return;
      activePointerId = event.pointerId ?? null;
    },
    pointerup: activateFromTouch,
    pointercancel: cancelTouch,
    pointerleave: cancelTouch,
    click(event) {
      if (isMatchingSyntheticClick(event, pendingSyntheticClick, now(), syntheticClickSuppressMs)) {
        pendingSyntheticClick = null;
        event?.preventDefault?.();
        event?.stopPropagation?.();
        return;
      }
      activate(event);
    },
    reset() {
      activePointerId = null;
      pendingSyntheticClick = null;
    },
  };
}

function isMatchingSyntheticClick(event, pending, currentTime, suppressMs) {
  if (!pending) return false;
  const elapsed = currentTime - pending.at;
  if (!Number.isFinite(elapsed) || elapsed < 0 || elapsed >= suppressMs) return false;

  const pointerType = String(event?.pointerType || "");
  if (pointerType) return pointerType === pending.pointerType;
  if (event?.sourceCapabilities?.firesTouchEvents === true) return true;
  if (event?.sourceCapabilities?.firesTouchEvents === false) return false;
  // Keyboard and programmatic button activation produce detail === 0. Older
  // browsers may expose the compatibility click as a MouseEvent without a
  // pointerType, in which case a positive detail is the best available signal.
  return Number(event?.detail) > 0;
}

function releasedInside(event) {
  const currentTarget = event?.currentTarget;
  const clientX = event?.clientX;
  const clientY = event?.clientY;
  if (
    Number.isFinite(clientX) &&
    Number.isFinite(clientY) &&
    typeof currentTarget?.getBoundingClientRect === "function"
  ) {
    const rect = currentTarget.getBoundingClientRect();
    return (
      clientX >= rect.left &&
      clientX <= rect.right &&
      clientY >= rect.top &&
      clientY <= rect.bottom
    );
  }

  const target = event?.target;
  if (!target || !currentTarget?.contains) return true;
  return target === currentTarget || currentTarget.contains(target);
}

function isPrimaryPointer(event) {
  if (event?.button != null && event.button !== 0) return false;
  if (event?.isPrimary === false) return false;
  return true;
}

function samePointerId(pointerId, event) {
  return pointerId == null || event?.pointerId == null || pointerId === event.pointerId;
}

function isTouchPointer(event) {
  const pointerType = String(event?.pointerType || "");
  return pointerType === "touch" || pointerType === "pen";
}

function finitePositive(value) {
  return Number.isFinite(value) && value > 0 ? value : null;
}
