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
  if (Number.isFinite(candidate.facing)) patch.facing = candidate.facing;
  if (candidate.motion === "move" || candidate.motion === "idle") patch.motion = candidate.motion;
  return patch;
}

/**
 * Compose the explicitly modeled prediction fields onto an authoritative display entity.
 * This is deliberately an allowlist: a prediction patch cannot replace the entity or
 * overwrite gameplay fields it does not simulate.
 */
export function composePredictionPatch(entity, patch) {
  const out = { ...entity, x: patch.x, y: patch.y, predicted: true };
  if (Object.prototype.hasOwnProperty.call(patch, "facing")) out.facing = patch.facing;
  if (patch.motion === "move" || patch.motion === "idle") out.state = patch.motion;
  return out;
}
