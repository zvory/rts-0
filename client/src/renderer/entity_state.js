import { isBuilding } from "../protocol.js";

export function isConstructionScaffold(entity) {
  return isBuilding(entity?.kind) &&
    typeof entity?.buildProgress === "number" &&
    entity.buildProgress < 1;
}

export function buildingProgressStatus(entity) {
  if (isConstructionScaffold(entity)) {
    return { kind: "construction", fraction: entity.buildProgress };
  }
  if (isBuilding(entity?.kind) && typeof entity?.deconstructProgress === "number") {
    return { kind: "deconstruction", fraction: entity.deconstructProgress };
  }
  return null;
}
