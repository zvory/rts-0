/**
 * Reduce an untrusted predictor record to the display fields the local model
 * explicitly supports. Returning a fresh object prevents future predictor
 * fields from silently gaining authority in the client.
 */
export function normalizePredictionPatch(candidate) {
  if (!Number.isInteger(candidate?.id) || !Number.isFinite(candidate.x) || !Number.isFinite(candidate.y)) {
    return null;
  }
  const patch = { id: candidate.id, x: candidate.x, y: candidate.y };
  if (candidate.motion === "move" || candidate.motion === "idle") patch.motion = candidate.motion;
  return patch;
}

export function normalizeProgressPatch(candidate) {
  if (!Number.isInteger(candidate?.id)) return null;
  if (candidate.kind !== "construction" && candidate.kind !== "production") return null;
  if (typeof candidate.identity !== "string" || candidate.identity.length === 0) return null;
  if (!Number.isFinite(candidate.fraction) || candidate.fraction < 0 || candidate.fraction >= 1) return null;
  return {
    id: candidate.id,
    kind: candidate.kind,
    identity: candidate.identity,
    fraction: Math.min(0.98, candidate.fraction),
  };
}

/**
 * Compose the explicitly modeled prediction fields onto an authoritative display entity.
 * This is deliberately an allowlist: a prediction patch cannot replace the entity or
 * overwrite gameplay fields it does not simulate.
 */
export function composePredictionPatch(entity, patch) {
  const out = { ...entity, x: patch.x, y: patch.y, predicted: true };
  if (patch.motion === "move" || patch.motion === "idle") out.state = patch.motion;
  return out;
}

export function progressPatchMatches(entity, patch, playerId, { isBuilding }) {
  if (!entity || entity.owner !== playerId || !isBuilding(entity.kind)) return false;
  if (patch.kind === "construction") {
    return entity.buildActive === true &&
      Number.isFinite(entity.buildProgress) && entity.buildProgress >= 0 && entity.buildProgress < 1 &&
      patch.identity === `build:${entity.kind}`;
  }
  const identity = productionIdentity(entity);
  return entity.buildProgress == null &&
    entity.prodWaiting !== true &&
    Number.isInteger(entity.prodQueue) && entity.prodQueue > 0 &&
    Number.isFinite(entity.prodProgress) && entity.prodProgress >= 0 && entity.prodProgress < 1 &&
    identity != null && patch.identity === identity;
}

export function composeProgressPatch(entity, patch) {
  if (patch.kind === "construction") {
    if (!(patch.fraction > entity.buildProgress)) return entity;
    return {
      ...entity,
      buildProgress: patch.fraction,
      progressPredicted: true,
      buildProgressPredicted: true,
    };
  }
  if (!(patch.fraction > entity.prodProgress)) return entity;
  return { ...entity, prodProgress: patch.fraction, progressPredicted: true };
}

function productionIdentity(entity) {
  if (typeof entity?.prodUpgrade === "string" && entity.prodUpgrade) return `upgrade:${entity.prodUpgrade}`;
  if (typeof entity?.prodKind === "string" && entity.prodKind) return `unit:${entity.prodKind}`;
  return null;
}
