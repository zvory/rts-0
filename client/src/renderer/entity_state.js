import { isBuilding } from "../protocol.js";

export function isConstructionScaffold(entity) {
  return isBuilding(entity?.kind) &&
    typeof entity?.buildProgress === "number" &&
    entity.buildProgress < 1;
}
