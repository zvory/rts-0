import { isBuilding } from "../protocol.js";

export function isConstructionScaffold(entity) {
  return isBuilding(entity?.kind) &&
    typeof entity?.buildProgress === "number" &&
    entity.buildProgress < 1;
}

export function buildingProgressStatus(entity) {
  if (isConstructionScaffold(entity)) {
    const fraction = Number.isFinite(entity.hp) && Number.isFinite(entity.maxHp) && entity.maxHp > 0
      ? entity.hp / entity.maxHp
      : entity.buildProgress;
    return { kind: "construction", fraction };
  }
  if (isBuilding(entity?.kind) && typeof entity?.deconstructProgress === "number") {
    return { kind: "deconstruction", fraction: entity.deconstructProgress };
  }
  return null;
}
