import {VECTOR_FIELD_ADEPT_SIZE_TILES, VECTOR_FIELD_MASTER_SIZE_TILES} from "../config.js";
import {ABILITY} from "../protocol.js";
export const targetCenter = intent => intent?.artilleryFireCenter || intent?.vectorFieldCenter;
export const firstTargetPress = intent => [ABILITY.POINT_FIRE, ABILITY.VECTOR_FIELD]
  .includes(intent?.commandTarget?.ability) && !targetCenter(intent);

export function vectorFieldSize(carriers, center, rangePx) {
  const caster = carriers.find(c => Math.hypot(center.x-c.x,center.y-c.y) <= rangePx) || carriers[0];
  return caster?.daoMaster ? VECTOR_FIELD_MASTER_SIZE_TILES : VECTOR_FIELD_ADEPT_SIZE_TILES;
}
