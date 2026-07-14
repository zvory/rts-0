import { ABILITIES } from "../config.js";
import { ABILITY, KIND } from "../protocol.js";
import { finiteNumber } from "./shared.js";

export function _drawBreakthroughAuras(view, entities = []) {
  if (!view || typeof view.selectedEntities !== "function" || !Array.isArray(entities)) return;
  const definition = ABILITIES[ABILITY.BREAKTHROUGH];
  const tileSize = (this._map && this._map.tileSize) || 32;
  const radiusPx = (definition?.radiusTiles || 0) * tileSize;
  if (radiusPx <= 0) return;

  const drawnIds = new Set();
  for (const entity of entities) {
    if (entity.kind !== KIND.COMMAND_CAR || auraExpiresIn(entity) <= 0) continue;
    if (!finiteNumber(entity.x) || !finiteNumber(entity.y)) continue;
    drawBreakthroughAura(this._feedbackGfx, entity.x, entity.y, radiusPx, 0.78);
    drawnIds.add(entity.id);
  }

  for (const entity of view.selectedEntities()) {
    if (entity.kind !== KIND.COMMAND_CAR || drawnIds.has(entity.id)) continue;
    if (!finiteNumber(entity.x) || !finiteNumber(entity.y)) continue;
    const active = auraExpiresIn(entity) > 0;
    drawBreakthroughAura(this._feedbackGfx, entity.x, entity.y, radiusPx, active ? 0.78 : 0.32);
  }
}

export function drawBreakthroughAura(g, x, y, radiusPx, alpha = 0.8) {
  g.lineStyle(2.5, 0xf2d16b, alpha);
  g.drawCircle(x, y, radiusPx);
}

function auraExpiresIn(entity) {
  if (Number.isFinite(entity?.breakthroughAuraTicks)) return entity.breakthroughAuraTicks;
  if (!Array.isArray(entity?.abilities)) return 0;
  const ability = entity.abilities.find((entry) => entry?.ability === ABILITY.BREAKTHROUGH);
  return Number.isFinite(ability?.expiresIn) ? ability.expiresIn : 0;
}
