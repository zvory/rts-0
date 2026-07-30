export function desktopCursorAutoLockCanRun(match, root = globalThis) {
  if (!match.desktopCursorAutoLockEnabled) return false;
  if (!match.input || match.input.pointerLocked) return false;
  if (match.settings?.isOpen() || match.tabMenu?.isOpen()) return false;
  const doc = root.document;
  if (doc?.hidden) return false;
  if (typeof doc?.hasFocus === "function" && !doc.hasFocus()) return false;
  return true;
}

export function handleInteractiveMenuStateChange(match, open, relockDelayMs) {
  if (open) {
    match.clearDesktopCursorAutoLockTimer();
    if (match.input?.pointerLocked) void match.input.exitPointerLock();
    return;
  }
  if (match.settings?.isOpen() || match.tabMenu?.isOpen()) return;
  match.scheduleDesktopCursorAutoLock("interactive-menu-closed", relockDelayMs);
}
