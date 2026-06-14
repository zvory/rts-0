import { DEFAULT_FACTION_ID, PREDICTION_PROTOCOL_VERSION } from "./protocol.js";

const SUPPORTED_PREDICTION_FACTION_IDS = Object.freeze([DEFAULT_FACTION_ID]);

export function predictionCompatibility(payload, { clientBuildId = defaultClientBuildId() } = {}) {
  const localFactionId = localPlayerFactionId(payload);
  const serverVersion = Number(payload?.predictionVersion) || 0;
  const serverBuildId = typeof payload?.predictionBuildId === "string" ? payload.predictionBuildId : "";
  const base = {
    clientVersion: PREDICTION_PROTOCOL_VERSION,
    serverVersion,
    clientBuildId: clientBuildId || null,
    serverBuildId: serverBuildId || null,
    localFactionId,
    supportedFactionIds: SUPPORTED_PREDICTION_FACTION_IDS.slice(),
  };

  if (!payload?.spectator && localFactionId && !SUPPORTED_PREDICTION_FACTION_IDS.includes(localFactionId)) {
    return {
      ok: false,
      reason: "unsupported-local-faction",
      ...base,
    };
  }
  if (serverVersion !== PREDICTION_PROTOCOL_VERSION) {
    return {
      ok: false,
      reason: serverVersion ? "prediction-version-mismatch" : "prediction-unavailable",
      ...base,
    };
  }
  if (clientBuildId && serverBuildId && clientBuildId !== serverBuildId) {
    return {
      ok: false,
      reason: "prediction-build-mismatch",
      ...base,
    };
  }
  return {
    ok: true,
    reason: null,
    ...base,
  };
}

export function predictionBlockedReason({ enabled, replayViewer, spectator, compatibility }) {
  if (!enabled) return "user-disabled";
  if (replayViewer) return "replay-viewer";
  if (spectator) return "spectator";
  if (compatibility && !compatibility.ok) return compatibility.reason || "compatibility-mismatch";
  return null;
}

export function localPlayerFactionId(payload) {
  if (!payload || payload.spectator) return null;
  const playerId = Number(payload.playerId);
  if (!Number.isFinite(playerId)) return null;
  const player = (payload.players || []).find((candidate) => Number(candidate?.id) === playerId);
  if (!player) return null;
  return normalizedFactionId(player.factionId);
}

function normalizedFactionId(value) {
  return typeof value === "string" && value.length > 0 ? value : DEFAULT_FACTION_ID;
}

function defaultClientBuildId() {
  if (typeof globalThis.__RTS_BUILD__ === "string" && globalThis.__RTS_BUILD__ !== "unknown") {
    return globalThis.__RTS_BUILD__;
  }
  const scripts = typeof document !== "undefined" ? Array.from(document.scripts || []) : [];
  for (const script of scripts) {
    const src = script?.src || "";
    if (!src.includes("/src/main.js")) continue;
    try {
      const version = new URL(src, window.location.href).searchParams.get("v");
      if (version) return version;
    } catch {
      // Ignore malformed script URLs and fall through to unknown build compatibility.
    }
  }
  return null;
}
