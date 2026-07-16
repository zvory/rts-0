export function recordPointerLockDiagnostic(match, err = null) {
  if (!match.input?.installedAppRuntime()) return;
  const snapshot = {
    at: new Date().toISOString(),
    error: pointerLockErrorSummary(err),
    support: match.input.pointerLockDebugSnapshot(),
  };
  if (typeof window !== "undefined") window.__rtsPointerLockDebug = snapshot;
  showPointerLockDiagnostic(snapshot);
  if (match.pointerLockDiagnosticShown) return;
  match.pointerLockDiagnosticShown = true;
  console.warn("[RTS_POINTER_LOCK_INSTALLED_APP]", snapshot);
  match.toast(pointerLockDiagnosticToast(snapshot));
}

export function showPointerLockDiagnostic(snapshot) {
  if (typeof document === "undefined") return;
  let panel = document.getElementById("desktop-cursor-diagnostic");
  if (!panel) {
    panel = document.createElement("pre");
    panel.id = "desktop-cursor-diagnostic";
    panel.style.position = "fixed";
    panel.style.left = "12px";
    panel.style.bottom = "12px";
    panel.style.zIndex = "99999";
    panel.style.maxWidth = "760px";
    panel.style.maxHeight = "240px";
    panel.style.overflow = "auto";
    panel.style.padding = "10px 12px";
    panel.style.margin = "0";
    panel.style.border = "1px solid rgba(255,255,255,0.35)";
    panel.style.background = "rgba(18, 22, 28, 0.94)";
    panel.style.color = "#f4f7fb";
    panel.style.font = "12px ui-monospace, SFMono-Regular, Menlo, monospace";
    panel.style.whiteSpace = "pre-wrap";
    panel.style.pointerEvents = "none";
    document.body.appendChild(panel);
  }
  const native = snapshot?.support?.nativeCursor || {};
  const trace = snapshot?.support?.trace || {};
  const recentTrace = Array.isArray(trace.records) ? trace.records.slice(-12) : [];
  panel.textContent = [
    "Installed-app cursor lock diagnostic",
    `error: ${pointerLockDiagnosticToast(snapshot)}`,
    `attempts: ${snapshot?.support?.attempts ?? 0}`,
    `documentHasFocus: ${snapshot?.support?.documentHasFocus ?? "unknown"}`,
    `activeElement: ${JSON.stringify(snapshot?.support?.activeElement || null)}`,
    `lockTarget: ${JSON.stringify(snapshot?.support?.lockTarget || null)}`,
    `pointerLockElementMatches: ${!!snapshot?.support?.pointerLockElementMatches}`,
    `requestPointerLock: ${snapshot?.support?.requestPointerLock || "missing"}`,
    `exitPointerLock: ${snapshot?.support?.exitPointerLock || "missing"}`,
    `nativeBridgePresent: ${!!snapshot?.support?.nativeCursorBridgePresent}`,
    `nativeSupported: ${native.supported !== false}`,
    `nativeBackend: ${native.backend || "none"}`,
    `nativeLastError: ${native.lastError || "none"}`,
    `tauriGlobals: ${(snapshot?.support?.tauriGlobals || []).join(", ") || "none"}`,
    `desktopRuntime: ${JSON.stringify(snapshot?.support?.desktopRuntime || null)}`,
    `lastFocusAttempt: ${JSON.stringify(snapshot?.support?.lastFocusAttempt || null)}`,
    `lastRequest: ${JSON.stringify(snapshot?.support?.lastRequest || null)}`,
    `shellLog: ${JSON.stringify(trace.shellLog || null)}`,
    "recentTrace:",
    ...recentTrace.map((entry) => JSON.stringify(entry)),
  ].join("\n");
}

export function pointerLockDiagnosticToast(snapshot) {
  const native = snapshot?.support?.nativeCursor;
  const runtime = snapshot?.support?.desktopRuntime;
  const nativeRequired = runtime?.nativeCursorCapture === true || runtime?.pointerLockDisabled === true;
  const nativeError = native?.lastError || null;
  const error = snapshot?.error?.message || snapshot?.error?.name || null;
  if (nativeError) return `Installed-app cursor lock failed: ${nativeError}`;
  if (nativeRequired && native?.supported === false) return "Installed-app cursor lock failed: native bridge missing.";
  if (error) return `Installed-app cursor lock failed: ${error}`;
  return "Installed-app cursor lock failed.";
}

export function pointerLockErrorSummary(err) {
  if (!err) return null;
  if (err instanceof Error) return { name: err.name, message: err.message };
  if (typeof err === "object") {
    return {
      type: err.type || null,
      name: err.name || null,
      message: err.message || null,
    };
  }
  return { message: String(err) };
}
