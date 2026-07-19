import { gfxCircle, gfxStroke } from "./native_graphics.js";
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
    // Active auras are projected world state. Keep them below fog so team-intel
    // (`visionOnly`) entities do not promote their effects to tactical feedback.
    drawBreakthroughAura(this._abilityObjectGfx, entity.x, entity.y, radiusPx, 0.78);
    drawnIds.add(entity.id);
  }

  for (const entity of view.selectedEntities()) {
    if (entity.kind !== KIND.COMMAND_CAR || drawnIds.has(entity.id)) continue;
    if (!finiteNumber(entity.x) || !finiteNumber(entity.y)) continue;
    const active = auraExpiresIn(entity) > 0;
    drawBreakthroughAura(
      active ? this._abilityObjectGfx : this._feedbackGfx,
      entity.x,
      entity.y,
      radiusPx,
      active ? 0.78 : 0.32,
    );
  }
}

export function drawBreakthroughAura(g, x, y, radiusPx, alpha = 0.8) {
  gfxStroke(g, 2.5, 0xf2d16b, alpha);
  gfxCircle(g, x, y, radiusPx);
}

function auraExpiresIn(entity) {
  return Number.isFinite(entity?.breakthroughAuraTicks) ? entity.breakthroughAuraTicks : 0;
}
