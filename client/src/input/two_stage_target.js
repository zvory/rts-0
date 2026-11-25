import {VECTOR_FIELD_ADEPT_SIZE_TILES, VECTOR_FIELD_MASTER_SIZE_TILES} from "../config.js";
import {ABILITY, cmd} from "../protocol.js";
export const targetCenter = intent => intent?.artilleryFireCenter || intent?.vectorFieldCenter;
export const firstTargetPress = intent => [ABILITY.POINT_FIRE, ABILITY.VECTOR_FIELD]
  .includes(intent?.commandTarget?.ability) && !targetCenter(intent);

export function vectorFieldSize(carriers, center, rangePx) {
  const caster = carriers.find(c => Math.hypot(center.x-c.x,center.y-c.y) <= rangePx) || carriers[0];
  return caster?.daoMaster ? VECTOR_FIELD_MASTER_SIZE_TILES : VECTOR_FIELD_ADEPT_SIZE_TILES;
}

// Both viewport and minimap keep the first point armed until a usable direction is chosen.
export function vectorFieldTargetCommand(intent, units, world) {
  if (!intent.vectorFieldCenter) {
    intent.vectorFieldCenter = { x: world.x, y: world.y };
    return null;
  }
  const center = intent.vectorFieldCenter;
  const dx = world.x - center.x, dy = world.y - center.y;
  if (Math.hypot(dx, dy) < 4) return null;
  intent.vectorFieldCenter = null;
  return cmd.vectorField(units, center.x, center.y, dx, dy);
}
