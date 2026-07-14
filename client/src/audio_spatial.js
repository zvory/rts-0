// Spatial profiles for the shared Web Audio engine. Combat uses a bounded
// world-space envelope so visual zoom does not redefine acoustic distance.

const COMBAT_CATEGORIES = new Set(["combat_self", "combat_other"]);

const MAX_DIST_MULT = 3;
const FAR_DISTANCE_EFFECT_MULT = 4;
const LP_NEAR_HZ = 20000;
const LP_FAR_HZ = 1200;
export const DEFAULT_AUDIO_REF_DIST = 1920;

const COMBAT_NEAR_MULT = 0.4;
const COMBAT_REF_DIST_CAP = 1280;
const COMBAT_MAX_DIST_MULT = 1.2;
const COMBAT_DISTANCE_EFFECT_MULT = 4;
const COMBAT_MAX_DISTANCE_PENALTY = 30;

function clamp01(value) {
  return value < 0 ? 0 : value > 1 ? 1 : value;
}

/**
 * @param {{x:number,y:number,refDist:number}} listener
 * @param {number} x emitter world x
 * @param {number} y emitter world y
 * @param {string} [category] voice category
 * @returns {{gain:number,pan:number,lpHz:number,distance:number,distancePenalty:number}|null}
 */
export function computeSpatialAudio(listener, x, y, category) {
  const refDist = Math.max(1, listener.refDist || DEFAULT_AUDIO_REF_DIST);
  const dx = x - listener.x;
  const dy = y - listener.y;
  const distance = Math.sqrt(dx * dx + dy * dy);

  if (COMBAT_CATEGORIES.has(category)) {
    const combatRefDist = Math.min(refDist, COMBAT_REF_DIST_CAP);
    const near = COMBAT_NEAR_MULT * combatRefDist;
    const localizedBoundary = COMBAT_MAX_DIST_MULT * combatRefDist;
    if (distance > localizedBoundary) return null;
    const effectiveDistance = near
      + Math.max(0, distance - near) * COMBAT_DISTANCE_EFFECT_MULT;
    const farT = clamp01((distance - near) / (localizedBoundary - near));
    return {
      gain: clamp01(near / Math.max(effectiveDistance, near)),
      pan: Math.max(-1, Math.min(1, dx / combatRefDist)),
      lpHz: LP_NEAR_HZ + (LP_FAR_HZ - LP_NEAR_HZ) * farT,
      distance,
      distancePenalty: COMBAT_MAX_DISTANCE_PENALTY * farT,
    };
  }

  const maxDist = MAX_DIST_MULT * refDist;
  if (distance > maxDist) return null;
  const effectiveDistance = refDist
    + Math.max(0, distance - refDist) * FAR_DISTANCE_EFFECT_MULT;
  return {
    gain: clamp01(refDist / Math.max(effectiveDistance, refDist)),
    pan: Math.max(-1, Math.min(1, dx / refDist)),
    lpHz: LP_NEAR_HZ + (LP_FAR_HZ - LP_NEAR_HZ) * clamp01(effectiveDistance / maxDist),
    distance,
    distancePenalty: Math.min(30, (effectiveDistance / refDist) * 10),
  };
}
