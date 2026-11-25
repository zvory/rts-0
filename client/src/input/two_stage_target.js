import {ABILITY} from "../protocol.js";
export const targetCenter = intent => intent?.artilleryFireCenter || intent?.vectorFieldCenter;
export const firstTargetPress = intent => [ABILITY.POINT_FIRE, ABILITY.VECTOR_FIELD]
  .includes(intent?.commandTarget?.ability) && !targetCenter(intent);
