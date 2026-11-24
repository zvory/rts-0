const GOLDEN_IMAGES = Object.freeze({
  "warrior-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/warrior-runtime-strip.png",
  "wind-melee-b-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/wind-melee-b-runtime-strip.png",
  "war-yari-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/war-yari-runtime-strip.png",
  "magic-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/magic-runtime-strip.png",
  "wind-master-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/wind-master-runtime-strip.png",
  "war-master-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/war-master-runtime-strip.png",
  "magic-master-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/magic-master-runtime-strip.png",
});

// The first row remains the original team-tinted character atlas. The second row
// contains generated, independently colored attack effects in its first cells.
export function withGoldenAttackTrail(strip) {
  const basename = strip.image.split('/').at(-1).split('?')[0];
  const image = GOLDEN_IMAGES[basename];
  if (!image) throw new Error(`Unknown Cultivator attack strip: ${basename}`);
  const revised = /^(warrior|magic|magic-master)-runtime-strip\.png$/.test(basename);
  const revision = revised ? "gold-2" : "gold-1";
  return Object.freeze({
    ...strip,
    image: `${image}?v=${revision}`,
    imageVersion: `${strip.imageVersion.replace(/-gold-\d+$/, "")}-${revision}`,
    attackTrail: Object.freeze({ row: 1, frameCount: basename === "warrior-runtime-strip.png" ? 6 : 4 }),
    source: Object.freeze({ ...strip.source,
      generator: 'scripts/art/cultivator-golden-pipeline.mjs', runtimeStrip: `client${image}` }),
  });
}

// Use the same authoritative shot envelope and weapon admission as the body clip.
export function goldenAttackTrailFrame(strip, context = {}) {
  if (!strip.attackTrail || !(context.recoilProgress > 0)
    || !strip.firingWeaponKinds?.includes(context.recoilWeaponKind)) return null;
  const hold = strip.firingFrameHoldPhase || 1;
  const phase = context.recoilPhase;
  if (!Number.isFinite(phase) || phase < 0 || phase >= hold) return null;
  return Math.min(strip.attackTrail.frameCount - 1,
    Math.floor(phase / hold * strip.attackTrail.frameCount));
}
