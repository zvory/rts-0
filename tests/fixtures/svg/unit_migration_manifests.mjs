import path from "node:path";
import { fileURLToPath } from "node:url";
import { KIND } from "../../../client/src/protocol.js";
import {
  ANTI_TANK_GUN_PARTS,
  ARTILLERY_PARTS,
  MORTAR_TEAM_PARTS,
} from "../../../client/src/renderer/rigs/support_svg.js";
import {
  COMMAND_CAR_PARTS,
  EKAT_PARTS,
  SCOUT_CAR_PARTS,
} from "../../../client/src/renderer/rigs/vehicle_svg.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const compositionThresholds = Object.freeze({
  minAlphaWeightedMatchingRatio: 0.985,
  maxPerPixelRgbaDistance: 96,
  maxOpaqueMismatchCount: 48,
  maxOpaqueMismatchClusterPx: 12,
  perChannelTolerance: 6,
  opaqueAlphaThreshold: 128,
});

const scoutCarCompositionThresholds = Object.freeze({
  ...compositionThresholds,
  minAlphaWeightedMatchingRatio: 0.98,
  maxOpaqueMismatchCount: 128,
  maxOpaqueMismatchClusterPx: 40,
});

const commandCarCompositionThresholds = Object.freeze({
  ...compositionThresholds,
  minAlphaWeightedMatchingRatio: 0.97,
  maxOpaqueMismatchCount: 96,
  maxOpaqueMismatchClusterPx: 36,
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

const riflemanBodyRigParts = Object.freeze(["part.body", "part.head", "part.shoulders"]);
const riflemanWeaponRigParts = Object.freeze(["part.rifle.barrel", "part.rifle.hand"]);
const machineGunnerBodyRigParts = riflemanBodyRigParts;
const machineGunnerWeaponRigParts = Object.freeze([
  "part.mg.main",
  "part.mg.stock",
  "part.mg.receiver",
  "part.mg.topPlate",
  "part.mg.shroud",
  "part.mg.slot.0",
  "part.mg.slot.1",
  "part.mg.slot.2",
  "part.mg.slot.3",
  "part.mg.muzzleTick",
  "part.mg.grip",
  "part.mg.bipod",
  "part.mg.muzzleCap",
]);

export const SVG_MIGRATION_MANIFESTS = Object.freeze([
  Object.freeze({
    kind: KIND.COMMAND_CAR,
    svgPath: path.join(__dirname, "rig-command-car.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: COMMAND_CAR_PARTS,
    compositionThresholds: commandCarCompositionThresholds,
    requiredSamples: Object.freeze([
      "command_car/facing-0-#0072b2",
      "command_car/facing-0-#e69f00",
      "command_car/facing-1_571-#0072b2",
      "command_car/facing-1_571-#e69f00",
      "command_car/facing-3_142-#0072b2",
      "command_car/facing-3_142-#e69f00",
      "command_car/facing-4_712-#0072b2",
      "command_car/facing-4_712-#e69f00",
      "command_car/command-breakthrough-on",
      "command_car/command-breakthrough-off",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "command_car.shadow",
        rigParts: COMMAND_CAR_PARTS.shadow,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 64, maxOpaqueMismatchClusterPx: 16 }),
      }),
      Object.freeze({
        legacyPart: "command_car.body",
        rigParts: Object.freeze([
          "part.hull",
          "part.sideGear.top.fill",
          "part.sideGear.bottom.fill",
          "part.cabin",
          "part.darkNose",
          "part.darkSlot.top",
          "part.darkSlot.bottom",
          "part.windshield",
          "part.noseTick",
        ]),
        thresholds: partThresholds({
          minAlphaWeightedMatchingRatio: 0.97,
          maxOpaqueMismatchCount: 64,
          maxOpaqueMismatchClusterPx: 36,
        }),
      }),
      Object.freeze({
        legacyPart: "command_car.badges",
        rigParts: Object.freeze(["part.badge.top", "part.badge.bottom"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 40, maxOpaqueMismatchClusterPx: 12 }),
      }),
      Object.freeze({
        legacyPart: "command_car.breakthroughAura",
        rigParts: Object.freeze(["part.breakthroughAura"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 80, maxOpaqueMismatchClusterPx: 20 }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.EKAT,
    svgPath: path.join(__dirname, "rig-ekat.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: EKAT_PARTS,
    compositionThresholds,
    requiredSamples: Object.freeze([
      "ekat/facing-0-#0072b2",
      "ekat/facing-0-#e69f00",
      "ekat/facing-1_571-#0072b2",
      "ekat/facing-1_571-#e69f00",
      "ekat/facing-3_142-#0072b2",
      "ekat/facing-3_142-#e69f00",
      "ekat/facing-4_712-#0072b2",
      "ekat/facing-4_712-#e69f00",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "ekat.shadow",
        rigParts: EKAT_PARTS.shadow,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 12, maxOpaqueMismatchClusterPx: 6 }),
      }),
      Object.freeze({
        legacyPart: "ekat.body",
        rigParts: Object.freeze(["part.body"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 16, maxOpaqueMismatchClusterPx: 8 }),
      }),
      Object.freeze({
        legacyPart: "ekat.facingTick",
        rigParts: Object.freeze(["part.facingTick"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 8, maxOpaqueMismatchClusterPx: 4 }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.SCOUT_CAR,
    svgPath: path.join(__dirname, "rig-scout-car.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: SCOUT_CAR_PARTS,
    compositionThresholds: scoutCarCompositionThresholds,
    requiredSamples: Object.freeze([
      "scout_car/facing-0-#0072b2",
      "scout_car/facing-0-#e69f00",
      "scout_car/facing-1_571-#0072b2",
      "scout_car/facing-1_571-#e69f00",
      "scout_car/facing-3_142-#0072b2",
      "scout_car/facing-3_142-#e69f00",
      "scout_car/facing-4_712-#0072b2",
      "scout_car/facing-4_712-#e69f00",
      "scout_car/weapon-offset-0_785",
      "scout_car/weapon-offset-neg_1_571",
      "scout_car/recoil-0_35",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "scout_car.shadow",
        rigParts: SCOUT_CAR_PARTS.shadow,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 64, maxOpaqueMismatchClusterPx: 16 }),
      }),
      Object.freeze({
        legacyPart: "scout_car.body",
        rigParts: SCOUT_CAR_PARTS.unit,
        thresholds: partThresholds({
          minAlphaWeightedMatchingRatio: 0.98,
          maxOpaqueMismatchCount: 128,
          maxOpaqueMismatchClusterPx: 40,
        }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.MACHINE_GUNNER,
    svgPath: path.join(__dirname, "rig-machine-gunner.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: Object.freeze({
      shadow: Object.freeze(["part.shadow"]),
      unit: Object.freeze([
        ...machineGunnerBodyRigParts,
        ...machineGunnerWeaponRigParts,
      ]),
    }),
    compositionThresholds,
    requiredSamples: Object.freeze([
      "machine_gunner/facing-0-#0072b2",
      "machine_gunner/facing-0-#e69f00",
      "machine_gunner/facing-1_571-#0072b2",
      "machine_gunner/facing-1_571-#e69f00",
      "machine_gunner/facing-3_142-#0072b2",
      "machine_gunner/facing-3_142-#e69f00",
      "machine_gunner/facing-4_712-#0072b2",
      "machine_gunner/facing-4_712-#e69f00",
      "machine_gunner/weapon-offset-0_785",
      "machine_gunner/weapon-offset-neg_1_571",
      "machine_gunner/recoil-0_35",
      "machine_gunner/setup-setting_up-0.5",
      "machine_gunner/setup-deployed-1",
      "machine_gunner/setup-tearing_down-0.5",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "machine_gunner.shadow",
        rigParts: Object.freeze(["part.shadow"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 12, maxOpaqueMismatchClusterPx: 6 }),
      }),
      Object.freeze({
        legacyPart: "machine_gunner.body",
        rigParts: machineGunnerBodyRigParts,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 24, maxOpaqueMismatchClusterPx: 8 }),
      }),
      Object.freeze({
        legacyPart: "machine_gunner.weapon",
        rigParts: machineGunnerWeaponRigParts,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 96, maxOpaqueMismatchClusterPx: 20 }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.RIFLEMAN,
    svgPath: path.join(__dirname, "rig-rifleman.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: Object.freeze({
      shadow: Object.freeze(["part.shadow"]),
      unit: Object.freeze([
        ...riflemanBodyRigParts,
        ...riflemanWeaponRigParts,
      ]),
    }),
    compositionThresholds,
    requiredSamples: Object.freeze([
      "rifleman/facing-0-#0072b2",
      "rifleman/facing-0-#e69f00",
      "rifleman/facing-1_571-#0072b2",
      "rifleman/facing-1_571-#e69f00",
      "rifleman/facing-3_142-#0072b2",
      "rifleman/facing-3_142-#e69f00",
      "rifleman/facing-4_712-#0072b2",
      "rifleman/facing-4_712-#e69f00",
      "rifleman/recoil-0_35",
      "rifleman/recoil-1",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "rifleman.shadow",
        rigParts: Object.freeze(["part.shadow"]),
        thresholds: partThresholds({ maxOpaqueMismatchCount: 12, maxOpaqueMismatchClusterPx: 6 }),
      }),
      Object.freeze({
        legacyPart: "rifleman.body",
        rigParts: riflemanBodyRigParts,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 24, maxOpaqueMismatchClusterPx: 8 }),
      }),
      Object.freeze({
        legacyPart: "rifleman.weapon",
        rigParts: riflemanWeaponRigParts,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 16, maxOpaqueMismatchClusterPx: 8 }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.ANTI_TANK_GUN,
    svgPath: path.join(__dirname, "rig-anti-tank-gun.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: Object.freeze({
      shadow: ANTI_TANK_GUN_PARTS.shadow,
      unit: ANTI_TANK_GUN_PARTS.weapon,
    }),
    compositionThresholds,
    requiredSamples: Object.freeze([
      "anti_tank_gun/facing-0-#0072b2",
      "anti_tank_gun/facing-0-#e69f00",
      "anti_tank_gun/facing-1_571-#0072b2",
      "anti_tank_gun/facing-1_571-#e69f00",
      "anti_tank_gun/facing-3_142-#0072b2",
      "anti_tank_gun/facing-3_142-#e69f00",
      "anti_tank_gun/facing-4_712-#0072b2",
      "anti_tank_gun/facing-4_712-#e69f00",
      "anti_tank_gun/weapon-offset-0_785",
      "anti_tank_gun/weapon-offset-neg_1_571",
      "anti_tank_gun/recoil-0_35",
      "anti_tank_gun/setup-packed-0",
      "anti_tank_gun/setup-setting_up-0.5",
      "anti_tank_gun/setup-deployed-1",
      "anti_tank_gun/setup-tearing_down-0.5",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "anti_tank_gun.shadow",
        rigParts: ANTI_TANK_GUN_PARTS.shadow,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 96, maxOpaqueMismatchClusterPx: 20 }),
      }),
      Object.freeze({
        legacyPart: "anti_tank_gun.weapon",
        rigParts: ANTI_TANK_GUN_PARTS.weapon,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 260, maxOpaqueMismatchClusterPx: 36 }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.MORTAR_TEAM,
    svgPath: path.join(__dirname, "rig-mortar-team.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: Object.freeze({
      shadow: MORTAR_TEAM_PARTS.shadow,
      unit: MORTAR_TEAM_PARTS.weapon,
    }),
    compositionThresholds,
    requiredSamples: Object.freeze([
      "mortar_team/facing-0-#0072b2",
      "mortar_team/facing-0-#e69f00",
      "mortar_team/facing-1_571-#0072b2",
      "mortar_team/facing-1_571-#e69f00",
      "mortar_team/facing-3_142-#0072b2",
      "mortar_team/facing-3_142-#e69f00",
      "mortar_team/facing-4_712-#0072b2",
      "mortar_team/facing-4_712-#e69f00",
      "mortar_team/weapon-offset-0_785",
      "mortar_team/weapon-offset-neg_1_571",
      "mortar_team/recoil-0_35",
      "mortar_team/setup-packed-0",
      "mortar_team/setup-setting_up-0.5",
      "mortar_team/setup-deployed-1",
      "mortar_team/setup-tearing_down-0.5",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "mortar_team.shadow",
        rigParts: MORTAR_TEAM_PARTS.shadow,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 80, maxOpaqueMismatchClusterPx: 18 }),
      }),
      Object.freeze({
        legacyPart: "mortar_team.weapon",
        rigParts: MORTAR_TEAM_PARTS.weapon,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 240, maxOpaqueMismatchClusterPx: 36 }),
      }),
    ]),
  }),
  Object.freeze({
    kind: KIND.ARTILLERY,
    svgPath: path.join(__dirname, "rig-artillery.svg"),
    approvedIntentionalDrift: Object.freeze([]),
    liveRoutes: Object.freeze({
      shadow: ARTILLERY_PARTS.shadow,
      unit: ARTILLERY_PARTS.weapon,
    }),
    compositionThresholds,
    requiredSamples: Object.freeze([
      "artillery/facing-0-#0072b2",
      "artillery/facing-0-#e69f00",
      "artillery/facing-1_571-#0072b2",
      "artillery/facing-1_571-#e69f00",
      "artillery/facing-3_142-#0072b2",
      "artillery/facing-3_142-#e69f00",
      "artillery/facing-4_712-#0072b2",
      "artillery/facing-4_712-#e69f00",
      "artillery/weapon-offset-0_785",
      "artillery/weapon-offset-neg_1_571",
      "artillery/recoil-0_35",
      "artillery/setup-packed-0",
      "artillery/setup-setting_up-0.5",
      "artillery/setup-deployed-1",
      "artillery/setup-tearing_down-0.5",
    ]),
    partMappings: Object.freeze([
      Object.freeze({
        legacyPart: "artillery.shadow",
        rigParts: ARTILLERY_PARTS.shadow,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 96, maxOpaqueMismatchClusterPx: 20 }),
      }),
      Object.freeze({
        legacyPart: "artillery.weapon",
        rigParts: ARTILLERY_PARTS.weapon,
        thresholds: partThresholds({ maxOpaqueMismatchCount: 360, maxOpaqueMismatchClusterPx: 48 }),
      }),
    ]),
  }),
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
