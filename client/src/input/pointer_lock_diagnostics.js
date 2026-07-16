const POINTER_LOCK_TRACE_LIMIT = 80;
const POINTER_LOCK_SHELL_MESSAGE_LIMIT = 560;
const POINTER_LOCK_TRACE_STRING_LIMIT = 240;

export function _recordPointerLockTrace(phase, details = {}) {
  const trace = Array.isArray(this._pointerLockTrace) ? this._pointerLockTrace : [];
  this._pointerLockTrace = trace;
  this._pointerLockTraceSequence = Number.isFinite(this._pointerLockTraceSequence)
    ? this._pointerLockTraceSequence + 1
    : 1;
  const record = {
    sequence: this._pointerLockTraceSequence,
    at: new Date().toISOString(),
    attempt: Number.isFinite(this._pointerLockAttempt) ? this._pointerLockAttempt : 0,
    phase: boundedString(phase, 80),
    details: safeTraceValue(details),
  };
  trace.push(record);
  if (trace.length > POINTER_LOCK_TRACE_LIMIT) {
    trace.splice(0, trace.length - POINTER_LOCK_TRACE_LIMIT);
  }
  publishPointerLockTrace(this);
  persistPointerLockTrace(this, record);
  return record;
}

export function pointerLockTraceSnapshot(input) {
  const records = Array.isArray(input?._pointerLockTrace)
    ? input._pointerLockTrace.map((record) => safeTraceValue(record))
    : [];
  return {
    records,
    shellLog: safeTraceValue(input?._pointerLockShellLog || emptyShellLogStatus()),
  };
}

function persistPointerLockTrace(input, record) {
  const root = typeof globalThis === "undefined" ? null : globalThis;
  if (root?.__RTS_DESKTOP_RUNTIME?.shell !== "tauri") return;
  const status = pointerLockShellLogStatus(input);
  status.attempted += 1;
  const invoke = tauriInvokeFn(root);
  if (typeof invoke !== "function") {
    status.failed += 1;
    status.lastError = "Tauri invoke bridge is unavailable.";
    publishPointerLockTrace(input);
    return;
  }
  const message = boundedJson({
    sequence: record.sequence,
    attempt: record.attempt,
    ...record.details,
  });
  const event = `pointer_lock_${String(record.phase || "event").replace(/[^a-z0-9_-]+/gi, "_")}`;
  try {
    Promise.resolve(invoke("desktop_log_client_event", {
      event,
      message,
      url: root.location?.href || null,
    })).then(() => {
      status.succeeded += 1;
      status.lastError = null;
      publishPointerLockTrace(input);
    }, (err) => {
      status.failed += 1;
      status.lastError = errorText(err);
      publishPointerLockTrace(input);
    });
  } catch (err) {
    status.failed += 1;
    status.lastError = errorText(err);
    publishPointerLockTrace(input);
  }
}

function pointerLockShellLogStatus(input) {
  if (!input._pointerLockShellLog || typeof input._pointerLockShellLog !== "object") {
    input._pointerLockShellLog = emptyShellLogStatus();
  }
  return input._pointerLockShellLog;
}

function emptyShellLogStatus() {
  return { attempted: 0, succeeded: 0, failed: 0, lastError: null };
}

function publishPointerLockTrace(input) {
  const root = typeof globalThis === "undefined" ? null : globalThis;
  if (!root) return;
  const snapshot = pointerLockTraceSnapshot(input);
  try {
    root.__rtsPointerLockTrace = snapshot;
  } catch {
    // The in-memory Input trace remains authoritative if the page global is read-only.
  }
}

function tauriInvokeFn(root) {
  const candidates = [
    root.__TAURI_INTERNALS__?.invoke,
    root.__TAURI__?.core?.invoke,
    root.__TAURI__?.tauri?.invoke,
    root.__TAURI__?.invoke,
  ];
  return candidates.find((candidate) => typeof candidate === "function") || null;
}

function boundedJson(value) {
  let json;
  try {
    json = JSON.stringify(safeTraceValue(value));
  } catch (err) {
    json = JSON.stringify({ serializationError: errorText(err) });
  }
  if (json.length <= POINTER_LOCK_SHELL_MESSAGE_LIMIT) return json;
  const truncated = JSON.stringify({
    truncated: true,
    preview: json.slice(0, POINTER_LOCK_SHELL_MESSAGE_LIMIT - 40),
  });
  return truncated.length <= POINTER_LOCK_SHELL_MESSAGE_LIMIT
    ? truncated
    : truncated.slice(0, POINTER_LOCK_SHELL_MESSAGE_LIMIT);
}

function safeTraceValue(value, depth = 0) {
  if (value == null || typeof value === "boolean" || typeof value === "number") return value;
  if (typeof value === "string") return boundedString(value, POINTER_LOCK_TRACE_STRING_LIMIT);
  if (value instanceof Error) {
    return {
      name: boundedString(value.name, 80),
      message: boundedString(value.message, POINTER_LOCK_TRACE_STRING_LIMIT),
    };
  }
  if (depth >= 4) return "[depth-limit]";
  if (Array.isArray(value)) {
    return value.slice(0, 16).map((entry) => safeTraceValue(entry, depth + 1));
  }
  if (typeof value === "object") {
    const result = {};
    for (const [key, entry] of Object.entries(value).slice(0, 32)) {
      if (typeof entry === "function" || entry === undefined) continue;
      result[boundedString(key, 80)] = safeTraceValue(entry, depth + 1);
    }
    return result;
  }
  return boundedString(String(value), POINTER_LOCK_TRACE_STRING_LIMIT);
}

function boundedString(value, limit) {
  const text = String(value ?? "");
  return text.length <= limit ? text : `${text.slice(0, Math.max(0, limit - 3))}...`;
}

function errorText(err) {
  return boundedString(err?.message || err?.name || String(err), POINTER_LOCK_TRACE_STRING_LIMIT);
}
