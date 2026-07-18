import { DEFAULT_AI_PROFILE_ID, MAX_LOBBY_TEAMS, playableAiProfileId } from "./lobby_view.js";

const MAX_MATCH_SEATS = 4;
const PUBLIC_LOBBY_NAME_MAX_BYTES = 64;
const RESERVED_ROOM_PREFIXES = Object.freeze([
  "__dev_scenario__:",
  "__replay_artifact__:",
  "__match_replay__",
  "__replay_branch__",
  "__lab__:",
]);
const INTERNAL_OBSERVER_AI_PROFILE_IDS = Object.freeze(["ai_2_1", "ai_turtle"]);

function launchParams(locationLike) {
  return new URLSearchParams(locationLike?.search || "");
}

function parseLaunchBoolean(value, fallback) {
  if (value == null || value === "") return fallback;
  const normalized = String(value).trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(normalized)) return true;
  if (["0", "false", "no", "off"].includes(normalized)) return false;
  return fallback;
}

function byteLength(value) {
  if (typeof TextEncoder !== "undefined") return new TextEncoder().encode(value).length;
  return String(value).length;
}

function roomError(room) {
  if (!room) return "rtsRoom is empty.";
  if (byteLength(room) > PUBLIC_LOBBY_NAME_MAX_BYTES) return "rtsRoom is too long.";
  if (/[\u0000-\u001f\u007f]/.test(room)) return "rtsRoom contains unsupported characters.";
  if (RESERVED_ROOM_PREFIXES.some((prefix) => room.startsWith(prefix))) {
    return "rtsRoom uses a reserved room prefix.";
  }
  return "";
}

function generatedLaunchRoom(now = Date.now()) {
  const suffix = Math.max(0, Number(now) || 0).toString(36);
  return `ai-selfplay-${suffix}`;
}

function safeLaunchName(raw, fallback) {
  const value = String(raw || "").trim();
  if (!value) return fallback;
  if (value.length > 40 || /[\u0000-\u001f\u007f]/.test(value)) return fallback;
  return value;
}

function safeLaunchMap(raw, errors) {
  const value = String(raw || "").trim();
  if (!value) return "";
  if (value.length > 64 || /[\u0000-\u001f\u007f]/.test(value)) {
    errors.push("rtsMap is invalid.");
    return "";
  }
  return value;
}

function normalizeRole(raw, errors) {
  const value = String(raw || "spectator").trim().toLowerCase();
  if (value === "spectator" || value === "observer") return "spectator";
  if (value === "player" || value === "commander") return "player";
  errors.push("rtsRole must be spectator or player.");
  return "spectator";
}

function parseTeamId(raw, label, errors) {
  const value = Number(String(raw || "").trim());
  if (!Number.isInteger(value) || value < 1 || value > MAX_LOBBY_TEAMS) {
    errors.push(`${label} team must be 1-${MAX_LOBBY_TEAMS}.`);
    return null;
  }
  return value;
}

function aiSpecsFromParams(params) {
  return params
    .getAll("rtsAi")
    .flatMap((value) => String(value || "").split(","))
    .map((value) => value.trim())
    .filter(Boolean);
}

function parseAiSpec(raw, index, role, errors) {
  const [left, right, extra] = raw.split(":");
  if (extra != null) {
    errors.push(`rtsAi entry "${raw}" has too many fields.`);
    return null;
  }
  let teamToken = left;
  let profileToken = right;
  if (right == null) {
    if (/^[1-9][0-9]*$/.test(left)) {
      profileToken = DEFAULT_AI_PROFILE_ID;
    } else {
      teamToken = String(index + 1);
      profileToken = left;
    }
  }
  const teamId = parseTeamId(teamToken, `rtsAi entry "${raw}"`, errors);
  if (teamId == null) return null;
  const safeProfile = String(profileToken || DEFAULT_AI_PROFILE_ID).trim();
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(safeProfile)) {
    errors.push(`rtsAi entry "${raw}" has an invalid profile id.`);
    return null;
  }
  const aiProfileId = role === "spectator" && INTERNAL_OBSERVER_AI_PROFILE_IDS.includes(safeProfile)
    ? safeProfile
    : playableAiProfileId(safeProfile);
  return {
    teamId,
    aiProfileId,
  };
}

function parseAiSlots(params, role, errors) {
  const specs = aiSpecsFromParams(params);
  const rawSlots = specs.length ? specs : [`1:${DEFAULT_AI_PROFILE_ID}`, `2:${DEFAULT_AI_PROFILE_ID}`];
  const activeHumanSeats = role === "player" ? 1 : 0;
  if (rawSlots.length + activeHumanSeats > MAX_MATCH_SEATS) {
    errors.push(`rtsAi seats exceed the ${MAX_MATCH_SEATS}-player match limit.`);
    return [];
  }
  return rawSlots
    .map((raw, index) => parseAiSpec(raw, index, role, errors))
    .filter(Boolean);
}

export function matchLaunchConfig(locationLike = window.location, { now = Date.now() } = {}) {
  const params = launchParams(locationLike);
  const mode = String(params.get("rtsLaunch") || "").trim().toLowerCase();
  if (!mode) return null;
  if (mode !== "match") return null;

  const errors = [];
  const role = normalizeRole(params.get("rtsRole"), errors);
  const explicitRoom = String(params.get("rtsRoom") || "").trim();
  const room = explicitRoom || generatedLaunchRoom(now);
  const error = roomError(room);
  if (error) errors.push(error);

  const ai = parseAiSlots(params, role, errors);
  return {
    kind: "match",
    room,
    name: safeLaunchName(params.get("rtsName"), role === "spectator" ? "Spectator" : "Commander"),
    role,
    spectator: role === "spectator",
    start: parseLaunchBoolean(params.get("rtsStart"), true),
    map: safeLaunchMap(params.get("rtsMap"), errors),
    ai,
    errors,
  };
}

function playersFromLobby(lobby) {
  return Array.isArray(lobby?.players) ? lobby.players : [];
}

function aiPlayersFromLobby(lobby) {
  return playersFromLobby(lobby).filter((player) => !!player?.isAi);
}

function activeHumansFromLobby(lobby) {
  return playersFromLobby(lobby).filter((player) => !player?.isAi && !player?.isSpectator);
}

export function nextMatchLaunchAction(config, lobby, playerId) {
  if (!config || config.errors?.length) return { type: "none" };
  if (!lobby || String(lobby.room || "") !== config.room) return { type: "none" };
  const players = playersFromLobby(lobby);
  const mine = players.find((player) => Number(player?.id) === Number(playerId));
  if (!mine) return { type: "wait", message: "Waiting for launch lobby membership..." };
  if (Number(lobby.hostId) !== Number(playerId)) {
    return {
      type: "fail",
      message: `Cannot automate "${config.room}" because this browser is not the host.`,
    };
  }

  if (config.spectator && !mine.isSpectator) {
    return { type: "setSpectator", spectator: true };
  }
  if (!config.spectator && mine.isSpectator) {
    return { type: "setSpectator", spectator: false };
  }

  const activeHumans = activeHumansFromLobby(lobby);
  const unexpectedHumans = config.spectator
    ? activeHumans
    : activeHumans.filter((player) => Number(player?.id) !== Number(playerId));
  if (unexpectedHumans.length) {
    return {
      type: "fail",
      message: `Cannot automate "${config.room}" because it already has active human seats.`,
    };
  }

  if (config.map && String(lobby.map || "") !== config.map) {
    const maps = Array.isArray(lobby.maps) ? lobby.maps : [];
    if (!maps.some((entry) => entry?.name === config.map)) {
      return {
        type: "fail",
        message: `Cannot automate "${config.room}" because map "${config.map}" is unavailable.`,
      };
    }
    return { type: "selectMap", map: config.map };
  }

  const aiPlayers = aiPlayersFromLobby(lobby);
  if (aiPlayers.length > config.ai.length) {
    return {
      type: "fail",
      message: `Cannot automate "${config.room}" because it already has extra AI seats.`,
    };
  }

  for (let index = 0; index < aiPlayers.length; index += 1) {
    const current = aiPlayers[index];
    const desired = config.ai[index];
    if (!desired) break;
    if (Number(current.teamId) !== Number(desired.teamId)) {
      return { type: "setTeam", id: current.id, teamId: desired.teamId };
    }
    if (current.aiProfileId !== desired.aiProfileId) {
      return { type: "setAiProfile", id: current.id, aiProfileId: desired.aiProfileId };
    }
  }

  if (aiPlayers.length < config.ai.length) {
    const desired = config.ai[aiPlayers.length];
    return { type: "addAi", teamId: desired.teamId, aiProfileId: desired.aiProfileId };
  }

  if (!config.spectator && !mine.ready) {
    return { type: "ready", ready: true };
  }

  if (!config.start) return { type: "done", message: "Launch lobby is ready." };
  if (lobby.canStart) return { type: "start" };
  return { type: "wait", message: "Waiting for launch lobby to become startable..." };
}
