import { RESOURCE_AMOUNTS } from "./config.js";
import {
  DEFAULT_FACTION_ID,
  KIND,
  MOVEMENT_PATH_DIAGNOSTICS,
  PASSABLE,
} from "./protocol.js";

export function normalizeDiagnostics(diagnostics) {
  const movementPaths = Object.values(MOVEMENT_PATH_DIAGNOSTICS).includes(
    diagnostics?.movementPaths,
  )
    ? diagnostics.movementPaths
    : MOVEMENT_PATH_DIAGNOSTICS.NONE;
  return {
    movementPaths,
    observerAnalysis: diagnostics?.observerAnalysis === true,
  };
}

export function normalizeResource(node, index) {
  const kind = node.kind === KIND.OIL ? KIND.OIL : KIND.STEEL;
  return {
    id: typeof node.id === "number" ? node.id : -(index + 1),
    owner: 0,
    kind,
    x: node.x,
    y: node.y,
    hp: 1,
    maxHp: 1,
    state: "idle",
    remaining: node.remaining ?? RESOURCE_AMOUNTS[kind] ?? 0,
  };
}

export function normalizePlayer(player) {
  const id = Number(player?.id) >>> 0;
  const rawTeamId = Number(player?.teamId);
  return {
    ...player,
    id,
    teamId: Number.isInteger(rawTeamId) && rawTeamId > 0 ? rawTeamId >>> 0 : id,
    factionId:
      typeof player?.factionId === "string" && player.factionId.length > 0
        ? player.factionId
        : DEFAULT_FACTION_ID,
  };
}

export function playerById(players, id) {
  const playerId = Number(id);
  return players.find((player) => player.id === playerId) || null;
}

export function teamIdForPlayer(players, id) {
  const player = playerById(players, id);
  return player ? player.teamId : null;
}

export function isOwnOwner(playerId, owner) {
  return Number(owner) === playerId;
}

export function isAllyOwner(players, playerId, owner) {
  const ownerId = Number(owner);
  if (!Number.isInteger(ownerId) || ownerId === 0 || ownerId === playerId) return false;
  const ownTeam = teamIdForPlayer(players, playerId);
  const ownerTeam = teamIdForPlayer(players, ownerId);
  return ownTeam != null && ownerTeam != null && ownTeam !== 0 && ownTeam === ownerTeam;
}

export function isEnemyOwner(players, playerId, owner) {
  const ownerId = Number(owner);
  if (!Number.isInteger(ownerId) || ownerId === 0 || ownerId === playerId) return false;
  const ownTeam = teamIdForPlayer(players, playerId);
  const ownerTeam = teamIdForPlayer(players, ownerId);
  return ownTeam != null && ownerTeam != null && ownTeam !== ownerTeam;
}

export function isNeutralOwner(owner) {
  return Number(owner) === 0;
}

export function worldInBounds(map, wx, wy) {
  return (
    wx >= 0 &&
    wy >= 0 &&
    wx < map.width * map.tileSize &&
    wy < map.height * map.tileSize
  );
}

export function terrainAt(map, tileX, tileY) {
  if (tileX < 0 || tileY < 0 || tileX >= map.width || tileY >= map.height) {
    return null;
  }
  return map.terrain[tileY * map.width + tileX];
}

export function isPassable(map, tileX, tileY) {
  const terrain = terrainAt(map, tileX, tileY);
  if (terrain == null) return false;
  return !!PASSABLE[terrain];
}
