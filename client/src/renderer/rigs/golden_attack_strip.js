const GOLDEN_IMAGES = Object.freeze({
  "warrior-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/warrior-runtime-strip.png",
  "wind-melee-b-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/wind-melee-b-runtime-strip.png",
  "war-yari-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/war-yari-runtime-strip.png",
  "magic-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/magic-runtime-strip.png",
  "wind-master-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/wind-master-runtime-strip.png",
  "war-master-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/war-master-runtime-strip.png",
  "magic-master-runtime-strip.png": "/assets/rigs/cultivator-golden/generated/magic-master-runtime-strip.png",
});

// Original body and untinted gold effects occupy rows 0/1. Fixed-color and
// shaded team-paint layers occupy rows 2/3; both are shared by every player color.
export function withGoldenAttackTrail(strip) {
  const basename = strip.image.split('/').at(-1).split('?')[0];
  const originalImage = GOLDEN_IMAGES[basename];
  const image = originalImage?.replace("cultivator-golden/generated", "cultivator-team-color/generated");
  if (!image) throw new Error(`Unknown Cultivator attack strip: ${basename}`);
  const revised = /^(warrior|magic|magic-master)-runtime-strip\.png$/.test(basename);
  const revision = revised ? "gold-2" : "gold-1";
  return Object.freeze({
    ...strip,
    image: `${image}?v=${revision}-team-1`,
    sheetRows: 4,
    teamPaint: Object.freeze({ baseRow: 2, paintRow: 3 }),
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
      generator: 'scripts/art/cultivator-team-color-pipeline.mjs', runtimeStrip: `client${image}` }),
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
