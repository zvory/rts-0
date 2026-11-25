import {ABILITY} from "../protocol.js";
export const targetCenter = intent => intent?.artilleryFireCenter || intent?.vectorFieldCenter;
export const firstTargetPress = intent => [ABILITY.POINT_FIRE, ABILITY.EKAT_MAGIC_ANCHOR]
  .includes(intent?.commandTarget?.ability) && !targetCenter(intent);
