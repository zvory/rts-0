const GOLDEN_IMAGES = Object.freeze({
  "warrior-runtime-strip.png": "/assets/rigs/cultivator-team-color-preview/packed/warrior-runtime-strip.png",
  "wind-melee-b-runtime-strip.png": "/assets/rigs/cultivator-team-color-preview/packed/wind-melee-b-runtime-strip.png",
  "war-yari-runtime-strip.png": "/assets/rigs/cultivator-team-color-preview/packed/war-yari-runtime-strip.png",
  "magic-runtime-strip.png": "/assets/rigs/cultivator-team-color-preview/packed/magic-runtime-strip.png",
  "wind-master-runtime-strip.png": "/assets/rigs/cultivator-team-color-preview/packed/wind-master-runtime-strip.png",
  "war-master-runtime-strip.png": "/assets/rigs/cultivator-team-color-preview/packed/war-master-runtime-strip.png",
  "magic-master-runtime-strip.png": "/assets/rigs/cultivator-team-color-preview/packed/magic-master-runtime-strip.png",
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
    attackTrail: Object.freeze({ row: 1, frameCount: basename === "warrior-runtime-strip.png" ? 6 : 4,
      ...(/^magic/.test(basename) ? {
        scale: 2,
        // Target hand anchors plus measured energy-core centers in the source cells.
        sourceCenters: basename.startsWith("magic-master")
          ? [[128.21, 136.96], [122.87, 136.23], [127.98, 137.19], [123.45, 136.8]]
          : [[105.3, 152.35], [127.04, 167.36], [132.5, 175.9], [125.78, 170.18]],
        centers: basename.startsWith("magic-master")
          ? [[128, 164], [110, 183], [128, 183], [128, 167]]
          : [[108, 159], [128, 169], [135, 181], [128, 173]],
      } : {}),
    }),
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
