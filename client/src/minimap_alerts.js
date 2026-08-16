import { EVENT, isBuilding, isUnit } from "./protocol.js";

export const DEFAULT_PING_MS = 900;
export const UNDER_ATTACK_PING_MS = 4400;
export const UNDER_ATTACK_STROBE_PHASE_MS = 300;
export const MINIMAP_BORDER_PULSE_MS = 700;

const UNDER_ATTACK_TARGET_RADIUS_TILES = 2;
const DEFAULT_PING_INITIAL_RADIUS_PX = 4;
const UNDER_ATTACK_PING_INITIAL_RADIUS_PX = 32;
const UNDER_ATTACK_PING_FINAL_RADIUS_PX = 6;
const DEFAULT_PING_STROKE_PX = 2;
const UNDER_ATTACK_PING_STROKE_PX = 6;
const ALERT_PING_INNER_RIM_COLOR = "rgba(255,255,255,0.95)";
const ALERT_PING_INNER_RIM_INSET_PX = 6;
const ALERT_PING_INNER_RIM_STROKE_PX = 3;

export function resolveUnderAttackTargetId({
  entities,
  events,
  x,
  y,
  tileSize,
  isOwnOwner,
}) {
  // Resolve while the emitting snapshot's events are still current. A throttled tab may ingest
  // another snapshot before rendering, losing the evidence that this particular hit was lethal.
  if (deathAt(events, x, y)) return null;
  const maxDistance2 = (Math.max(1, Number(tileSize) || 32) * UNDER_ATTACK_TARGET_RADIUS_TILES) ** 2;
  let nearestId = null;
  let nearestDistance2 = maxDistance2;

  for (const entity of entities || []) {
    if (!isUnderAttackTarget(entity, isOwnOwner)) continue;
    const dx = Number(entity.x) - x;
    const dy = Number(entity.y) - y;
    const distance2 = dx * dx + dy * dy;
    if (!Number.isFinite(distance2) || distance2 > nearestDistance2) continue;
    nearestId = entity.id;
    nearestDistance2 = distance2;
  }
  return nearestId;
}

export function underAttackFlashEntityIds({ pings, entities, now, isOwnOwner }) {
  const flashing = new Set();
  if (!Array.isArray(entities) || pings.length === 0) return flashing;

  for (const ping of pings) {
    const age = now - ping.startedAt;
    if (!ping.isUnderAttack || age < 0 || age >= UNDER_ATTACK_PING_MS) continue;
    const target = entities.find((entity) => entity?.id === ping.targetEntityId);
    if (
      isUnderAttackTarget(target, isOwnOwner) &&
      Math.floor(age / UNDER_ATTACK_STROBE_PHASE_MS) % 2 === 0
    ) {
      flashing.add(ping.targetEntityId);
    }
  }
  return flashing;
}

export function drawMinimapPings({ ctx, pings, now, worldToCanvas, borderPulseUntil, size }) {
  const activePings = pings.filter((ping) => now - ping.startedAt < pingDurationMs(ping));
  for (const ping of activePings) {
    const t = (now - ping.startedAt) / pingDurationMs(ping);
    const p = worldToCanvas(ping.x, ping.y);
    const radius = ping.isUnderAttack
      ? UNDER_ATTACK_PING_INITIAL_RADIUS_PX
        + (UNDER_ATTACK_PING_FINAL_RADIUS_PX - UNDER_ATTACK_PING_INITIAL_RADIUS_PX) * t
      : DEFAULT_PING_INITIAL_RADIUS_PX + 15 * t;
    ctx.save();
    ctx.globalAlpha = ping.isUnderAttack ? 1 - t * t : 1 - t;
    ctx.strokeStyle = ping.severity === "warn" ? "#ffd166" : "#ff4d4d";
    ctx.lineWidth = ping.isUnderAttack ? UNDER_ATTACK_PING_STROKE_PX : DEFAULT_PING_STROKE_PX;
    ctx.beginPath();
    ctx.arc(p.x, p.y, radius, 0, Math.PI * 2);
    ctx.stroke();
    if (ping.isUnderAttack) {
      ctx.strokeStyle = ALERT_PING_INNER_RIM_COLOR;
      ctx.lineWidth = ALERT_PING_INNER_RIM_STROKE_PX;
      ctx.beginPath();
      ctx.arc(p.x, p.y, Math.max(1, radius - ALERT_PING_INNER_RIM_INSET_PX), 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.restore();
  }
  if (now < borderPulseUntil) {
    const t = 1 - (borderPulseUntil - now) / MINIMAP_BORDER_PULSE_MS;
    ctx.save();
    ctx.globalAlpha = Math.max(0, 1 - t);
    ctx.strokeStyle = "#ff4d4d";
    ctx.lineWidth = 3;
    ctx.strokeRect(1.5, 1.5, size - 3, size - 3);
    ctx.restore();
  }
  return activePings;
}

function isUnderAttackTarget(entity, isOwnOwner) {
  return entity?.id != null &&
    !entity.visionOnly &&
    isOwnOwner(entity.owner) &&
    (isUnit(entity.kind) || isBuilding(entity.kind));
}

function deathAt(events, x, y) {
  return (events || []).some((event) =>
    event?.e === EVENT.DEATH &&
    Math.abs(Number(event.x) - x) <= 0.01 &&
    Math.abs(Number(event.y) - y) <= 0.01
  );
}

function pingDurationMs(ping) {
  return ping.isUnderAttack ? UNDER_ATTACK_PING_MS : DEFAULT_PING_MS;
}
