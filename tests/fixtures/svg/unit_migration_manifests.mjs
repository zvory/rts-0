import path from "node:path";
import { fileURLToPath } from "node:url";
import { KIND } from "../../../client/src/protocol.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const compositionThresholds = Object.freeze({
  minAlphaWeightedMatchingRatio: 0.985,
  maxPerPixelRgbaDistance: 96,
  maxOpaqueMismatchCount: 48,
  maxOpaqueMismatchClusterPx: 12,
  perChannelTolerance: 6,
  opaqueAlphaThreshold: 128,
});

function partThresholds(overrides = {}) {
  return Object.freeze({
    minAlphaWeightedMatchingRatio: 0.996,
    maxPerPixelRgbaDistance: 64,
    maxOpaqueMismatchCount: 8,
    maxOpaqueMismatchClusterPx: 4,
    perChannelTolerance: 4,
    opaqueAlphaThreshold: 128,
    ...overrides,
  });
}

const tankTrackRigParts = Object.freeze([
  "part.track.left",
  "part.track.right",
  ...Array.from({ length: 9 }, (_, i) => `part.tread.left.${i}`),
  ...Array.from({ length: 9 }, (_, i) => `part.tread.right.${i}`),
]);

export const SVG_MIGRATION_MANIFESTS = Object.freeze([
  Object.freeze({
    kind: KIND.WORKER,
    svgPath: path.join(__dirname, "rig-worker.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: Object.freeze({
      shadow: Object.freeze(["part.shadow"]),
      unit: Object.freeze(["part.body", "part.busyIndicator", "part.facingTick"]),
    }),
    compositionThresholds,
    requiredSamples: Object.freeze([
      "worker/facing-0-#0072b2",
      "worker/facing-0-#e69f00",
      "worker/facing-1_571-#0072b2",
      "worker/facing-1_571-#e69f00",
      "worker/facing-3_142-#0072b2",
      "worker/facing-3_142-#e69f00",
      "worker/facing-4_712-#0072b2",
      "worker/facing-4_712-#e69f00",
      "worker/worker-busy-latched-node",
      "worker/worker-busy-build-state",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "worker.shadow",
        rigParts: Object.freeze(["part.shadow"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 12, maxOpaqueMismatchClusterPx: 6 }),
      }),
      Object.freeze({
        legacyPart: "worker.body",
        rigParts: Object.freeze(["part.body"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 16, maxOpaqueMismatchClusterPx: 8 }),
      }),
      Object.freeze({
        legacyPart: "worker.facingTick",
        rigParts: Object.freeze(["part.facingTick"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 8, maxOpaqueMismatchClusterPx: 4 }),
      }),
      Object.freeze({
        legacyPart: "worker.busyIndicator",
        rigParts: Object.freeze(["part.busyIndicator"]),
        busyOnly: true,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 8, maxOpaqueMismatchClusterPx: 4 }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.TANK,
    svgPath: path.join(__dirname, "rig-vehicle.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: Object.freeze({
      shadow: Object.freeze(["part.shadow"]),
      unit: Object.freeze([
        "part.track.left",
        "part.track.right",
        ...Array.from({ length: 9 }, (_, i) => `part.tread.left.${i}`),
        ...Array.from({ length: 9 }, (_, i) => `part.tread.right.${i}`),
        "part.hull",
        "part.hull.shadow",
        "part.hull.nose",
        "part.hull.noseShadow",
        "part.barrel",
        "part.turret",
        "part.noseTick",
        "part.fuelCue.box",
        "part.fuelCue.x1",
        "part.fuelCue.x2",
      ]),
    }),
    compositionThresholds,
    requiredSamples: Object.freeze([
      "tank/facing-0-#0072b2",
      "tank/facing-0-#e69f00",
      "tank/facing-1_571-#0072b2",
      "tank/facing-1_571-#e69f00",
      "tank/facing-3_142-#0072b2",
      "tank/facing-3_142-#e69f00",
      "tank/facing-4_712-#0072b2",
      "tank/facing-4_712-#e69f00",
      "tank/weapon-offset-0_785",
      "tank/weapon-offset-neg_1_571",
      "tank/recoil-0_35",
      "tank/tank-low-oil",
      "tank/tank-oil-starved",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "tank.shadow",
        rigParts: Object.freeze(["part.shadow"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 32, maxOpaqueMismatchClusterPx: 8 }),
      }),
      Object.freeze({
        legacyPart: "tank.tracks",
        rigParts: tankTrackRigParts,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 64, maxOpaqueMismatchClusterPx: 16 }),
      }),
      Object.freeze({
        legacyPart: "tank.hull",
        rigParts: Object.freeze(["part.hull", "part.hull.shadow", "part.hull.nose", "part.hull.noseShadow"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 64, maxOpaqueMismatchClusterPx: 16 }),
      }),
      Object.freeze({
        legacyPart: "tank.barrel",
        rigParts: Object.freeze(["part.barrel"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 16, maxOpaqueMismatchClusterPx: 8 }),
      }),
      Object.freeze({
        legacyPart: "tank.turret",
        rigParts: Object.freeze(["part.turret"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 24, maxOpaqueMismatchClusterPx: 8 }),
      }),
      Object.freeze({
        legacyPart: "tank.noseTick",
        rigParts: Object.freeze(["part.noseTick"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 8, maxOpaqueMismatchClusterPx: 4 }),
      }),
      Object.freeze({
        legacyPart: "tank.fuelCue",
        rigParts: Object.freeze(["part.fuelCue.box", "part.fuelCue.x1", "part.fuelCue.x2"]),
        fuelOnly: true,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 16, maxOpaqueMismatchClusterPx: 8 }),
      }),
    ]),
  }),
]);

export const SVG_MIGRATION_MANIFESTS_BY_KIND = Object.freeze(Object.fromEntries(
  SVG_MIGRATION_MANIFESTS.map((manifest) => [manifest.kind, manifest]),
));
