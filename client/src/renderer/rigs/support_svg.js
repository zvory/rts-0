import { KIND } from "../../protocol.js";

const OUTLINE = "#1a1712";
const DARK = "#241d17";
const METAL = "#d8d0b0";
const TEAM_GUN_FILL = "#5d7896";

const RECOIL = "recoilKickX:transform.x:1:0;recoilKickY:transform.y:1:0";
const WEAPON_FACE = "weaponVisualFacing:transform.rotation:1:0";
const CARRIAGE_FACE = "carriageVisualFacing:transform.rotation:1:0";
const WEAPON_KICK = "weaponRecoilX:transform.x:1:0;weaponRecoilY:transform.y:1:0";
const ATG_CARRIAGE_KICK = "weaponRecoilX:transform.x:0.12:0;weaponRecoilY:transform.y:0.12:0";
const MORTAR_CARRIAGE_KICK = "weaponRecoilX:transform.x:0.18:0;weaponRecoilY:transform.y:0.18:0";
const ARTILLERY_CARRIAGE_KICK = "weaponRecoilX:transform.x:0.42:0;weaponRecoilY:transform.y:0.42:0";
const ARTILLERY_WEAPON_KICK = "weaponRecoilX:transform.x:1.35:0;weaponRecoilY:transform.y:1.35:0";

const PACKED_ALPHA = "setupVisual:alpha:-1:1";
const DEPLOYED_ALPHA = "setupVisual:alpha:1:0";

export const ANTI_TANK_GUN_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  weapon: Object.freeze([
    ...supportPartIds("part.at", [
      "axle",
      "wheel.left.body", "wheel.left.tread.0", "wheel.left.tread.1", "wheel.left.cross.0", "wheel.left.cross.1", "wheel.left.cross.2", "wheel.left.hub",
      "wheel.right.body", "wheel.right.tread.0", "wheel.right.tread.1", "wheel.right.cross.0", "wheel.right.cross.1", "wheel.right.cross.2", "wheel.right.hub",
      "shield", "shieldStripe", "trail.left", "trail.right", "barrel", "barrelHighlight", "muzzleTick", "breech",
    ], "packed"),
    ...supportPartIds("part.at", [
      "axle",
      "wheel.left.body", "wheel.left.tread.0", "wheel.left.tread.1", "wheel.left.cross.0", "wheel.left.cross.1", "wheel.left.cross.2", "wheel.left.hub",
      "wheel.right.body", "wheel.right.tread.0", "wheel.right.tread.1", "wheel.right.cross.0", "wheel.right.cross.1", "wheel.right.cross.2", "wheel.right.hub",
      "shield", "shieldStripe", "trail.left", "trail.right", "brace.left", "brace.right", "barrel", "barrelHighlight", "muzzleTick", "breech",
    ], "deployed"),
  ]),
});

export const MORTAR_TEAM_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  weapon: Object.freeze([
    ...supportPartIds("part.mortar", [
      "axle",
      "wheel.left.body", "wheel.left.tread.0", "wheel.left.tread.1", "wheel.left.cross.0", "wheel.left.cross.1", "wheel.left.cross.2", "wheel.left.hub",
      "wheel.right.body", "wheel.right.tread.0", "wheel.right.tread.1", "wheel.right.cross.0", "wheel.right.cross.1", "wheel.right.cross.2", "wheel.right.hub",
      "trail", "leg.left.dark", "leg.right.dark", "leg.left", "leg.right", "base", "body", "tube", "tubeHighlight", "muzzle",
    ], "packed"),
    ...supportPartIds("part.mortar", [
      "basePlate",
      "axle",
      "wheel.left.body", "wheel.left.tread.0", "wheel.left.tread.1", "wheel.left.cross.0", "wheel.left.cross.1", "wheel.left.cross.2", "wheel.left.hub",
      "wheel.right.body", "wheel.right.tread.0", "wheel.right.tread.1", "wheel.right.cross.0", "wheel.right.cross.1", "wheel.right.cross.2", "wheel.right.hub",
      "trail", "leg.left.dark", "leg.right.dark", "leg.left", "leg.right", "base", "body", "tube", "tubeHighlight", "muzzle",
    ], "deployed"),
  ]),
});

export const ARTILLERY_PARTS = Object.freeze({
  shadow: Object.freeze(["part.shadow"]),
  weapon: Object.freeze([
    ...supportPartIds("part.art", [
      "axle",
      "wheel.left.body", "wheel.left.tread.0", "wheel.left.tread.1", "wheel.left.cross.0", "wheel.left.cross.1", "wheel.left.cross.2", "wheel.left.hub",
      "wheel.right.body", "wheel.right.tread.0", "wheel.right.tread.1", "wheel.right.cross.0", "wheel.right.cross.1", "wheel.right.cross.2", "wheel.right.hub",
      "trail.left", "trail.right", "cradleLink", "cradle", "breechBase", "barrel", "barrelHighlight", "breech", "breechShield",
    ], "packed"),
    ...supportPartIds("part.art", [
      "axle",
      "wheel.left.body", "wheel.left.tread.0", "wheel.left.tread.1", "wheel.left.cross.0", "wheel.left.cross.1", "wheel.left.cross.2", "wheel.left.hub",
      "wheel.right.body", "wheel.right.tread.0", "wheel.right.tread.1", "wheel.right.cross.0", "wheel.right.cross.1", "wheel.right.cross.2", "wheel.right.hub",
      "trail.left", "trail.right", "foot.left", "foot.right", "cradleLink", "cradle", "breechBase", "barrel", "barrelHighlight", "breech", "breechShield",
    ], "deployed"),
    "part.art.flashCone",
    "part.art.flashCore",
    "part.art.flashGlow",
  ]),
});

export const ANTI_TANK_GUN_RIG_SVG = supportRig({
  kind: KIND.ANTI_TANK_GUN,
  id: "anti-tank-gun.authored",
  viewBox: "-58 -46 116 92",
  shadow: vehicleShadow("part.shadow", 25, 16, 5.6, `facing:transform.rotation:1:0;${RECOIL}`),
  parts: [
    ...antiTankGunParts("packed", 0, PACKED_ALPHA),
    ...antiTankGunParts("deployed", 1, DEPLOYED_ALPHA),
  ],
  selection: { x: -25, y: -17, width: 50, height: 34 },
  hp: { x: -16, y: -33, width: 32, height: 5 },
  hpAnchorY: -28,
  anchors: { muzzle: { x: 38, y: 0 }, wheel: { x: -3.2, y: 8.4 } },
});

export const MORTAR_TEAM_RIG_SVG = supportRig({
  kind: KIND.MORTAR_TEAM,
  id: "mortar-team.authored",
  viewBox: "-42 -36 84 72",
  shadow: ellipse("part.shadow", 0, 6.3, 18, 10.8, { fill: "#000000", opacity: 0.28, animation: RECOIL }),
  parts: [
    ...mortarTeamParts("packed", 0, PACKED_ALPHA),
    ...mortarTeamParts("deployed", 1, DEPLOYED_ALPHA),
  ],
  selection: { x: -21, y: -21, width: 42, height: 42 },
  hp: { x: -13, y: -31, width: 26, height: 5 },
  hpAnchorY: -26,
  anchors: { muzzle: { x: 13.32, y: 0 }, bipod: { x: 3.96, y: 0 } },
});

export const ARTILLERY_RIG_SVG = supportRig({
  kind: KIND.ARTILLERY,
  id: "artillery.authored",
  viewBox: "-72 -56 144 112",
  shadow: ellipse("part.shadow", 0, 8.925983, 25.502809, 15.301685, { fill: "#000000", opacity: 0.28, animation: RECOIL }),
  parts: [
    ...artilleryParts("packed", 0, PACKED_ALPHA),
    ...artilleryParts("deployed", 1, DEPLOYED_ALPHA),
    artilleryFlashParts(),
  ],
  selection: { x: -30, y: -22, width: 60, height: 44 },
  hp: { x: -16, y: -38, width: 32, height: 5 },
  hpAnchorY: -33,
  anchors: { muzzle: { x: 45.864, y: 0 }, wheel: { x: -4.536, y: 11.808 } },
});

function antiTankGunParts(suffix, deploy, setupAlpha) {
  const r = 20;
  const wheelY = r * 0.42;
  const axleX = -r * 0.16;
  const trailRear = lerp(-r * 0.45, -r * 1.55, deploy);
  const trailSpread = lerp(r * 0.18, r * 0.72, deploy);
  const baseAnim = `${WEAPON_FACE};${RECOIL};${ATG_CARRIAGE_KICK};${setupAlpha}`;
  const barrelAnim = `${WEAPON_FACE};${RECOIL};${WEAPON_KICK};${setupAlpha}`;
  return [
    line(`part.at.axle.${suffix}`, axleX, -wheelY, axleX, wheelY, { stroke: OUTLINE, strokeWidth: 4, strokeOpacity: 0.9, animation: baseAnim }),
    ...gunTireParts(`part.at.wheel.left`, suffix, axleX, -wheelY, r * 0.68, r * 0.34, baseAnim),
    ...gunTireParts(`part.at.wheel.right`, suffix, axleX, wheelY, r * 0.68, r * 0.34, baseAnim),
    rect(`part.at.shield.${suffix}`, -r * 0.23, -r * 0.59, r * 0.46, r * 1.18, {
      fill: TEAM_GUN_FILL,
      fillOpacity: 0.96,
      stroke: "#17130f",
      strokeWidth: 1.4,
      strokeOpacity: 0.9,
      tint: "team",
      animation: `${baseAnim};weaponVisualDoubleCos:transform.x:${fmt(r * 0.12)}:0;weaponVisualDoubleSin:transform.y:${fmt(r * 0.12)}:0`,
    }),
    rect(`part.at.shieldStripe.${suffix}`, -r * 0.06, -r * 0.5, r * 0.12, r, {
      fill: OUTLINE,
      fillOpacity: 0.28,
      stroke: "#17130f",
      strokeWidth: 1.4,
      strokeOpacity: 0.9,
      animation: `${baseAnim};weaponVisualDoubleCos:transform.x:${fmt(r * 0.22)}:0;weaponVisualDoubleSin:transform.y:${fmt(r * 0.22)}:0`,
    }),
    line(`part.at.trail.left.${suffix}`, -r * 0.14, 0, trailRear, -trailSpread, { stroke: METAL, strokeWidth: 4, strokeOpacity: 0.9, animation: baseAnim }),
    line(`part.at.trail.right.${suffix}`, -r * 0.14, 0, trailRear, trailSpread, { stroke: METAL, strokeWidth: 4, strokeOpacity: 0.9, animation: baseAnim }),
    ...(deploy > 0 ? [
      line(`part.at.brace.left.${suffix}`, -r * 0.14, 0, lerp(-r * 0.2, -r * 0.95, deploy), -trailSpread * 0.72, { stroke: "#2a2119", strokeWidth: 3, strokeOpacity: 0.96, animation: baseAnim }),
      line(`part.at.brace.right.${suffix}`, -r * 0.14, 0, lerp(-r * 0.2, -r * 0.95, deploy), trailSpread * 0.72, { stroke: "#2a2119", strokeWidth: 3, strokeOpacity: 0.96, animation: baseAnim }),
    ] : []),
    line(`part.at.barrel.${suffix}`, -r * 0.28, 0, r * 1.9, 0, { stroke: DARK, strokeWidth: r * 0.22, strokeOpacity: 0.98, animation: barrelAnim }),
    line(`part.at.barrelHighlight.${suffix}`, -r * 0.28, -r * 0.07, r * 1.9, -r * 0.07, { stroke: METAL, strokeWidth: r * 0.07, strokeOpacity: 0.58, animation: barrelAnim }),
    line(`part.at.muzzleTick.${suffix}`, r * 1.58, 0, r * 1.9, 0, { stroke: METAL, strokeWidth: r * 0.1, strokeOpacity: 0.75, animation: barrelAnim }),
    rect(`part.at.breech.${suffix}`, -r * 0.38 - r * 0.26, -r * 0.21, r * 0.52, r * 0.42, {
      fill: "#3d3528",
      fillOpacity: 0.98,
      stroke: METAL,
      strokeWidth: r * 0.1,
      strokeOpacity: 0.75,
      animation: barrelAnim,
    }),
  ];
}

function mortarTeamParts(suffix, deploy, setupAlpha) {
  const r = 18;
  const wheelX = lerp(-r * 0.42, -r * 0.28, deploy);
  const wheelY = r * 0.52;
  const baseAnim = `${WEAPON_FACE};${RECOIL};${MORTAR_CARRIAGE_KICK};${setupAlpha}`;
  const tubeAnim = `${WEAPON_FACE};${RECOIL};${WEAPON_KICK};${setupAlpha}`;
  const footSpread = lerp(r * 0.12, r * 0.46, deploy);
  const footForward = lerp(r * 0.52, r * 0.82, deploy);
  const muzzleDist = lerp(r * 1.02, r * 0.74, deploy);
  const bipodRootX = r * 0.22;
  return [
    ...(deploy > 0 ? mortarBasePlateParts(suffix) : []),
    line(`part.mortar.axle.${suffix}`, wheelX, -wheelY, wheelX, wheelY, { stroke: OUTLINE, strokeWidth: 2, strokeOpacity: 0.9, animation: baseAnim }),
    ...gunTireParts(`part.mortar.wheel.left`, suffix, wheelX, -wheelY, r * 0.54, r * 0.18, baseAnim),
    ...gunTireParts(`part.mortar.wheel.right`, suffix, wheelX, wheelY, r * 0.54, r * 0.18, baseAnim),
    line(`part.mortar.trail.${suffix}`, lerp(-r * 1.2, -r * 0.72, deploy), 0, -r * 0.16, 0, { stroke: TEAM_GUN_FILL, strokeWidth: 2, strokeOpacity: 0.9, tint: "team-stroke", animation: baseAnim }),
    line(`part.mortar.leg.left.dark.${suffix}`, bipodRootX, 0, footForward, -footSpread, { stroke: "#15120f", strokeWidth: 3.2, strokeOpacity: 0.72, animation: baseAnim }),
    line(`part.mortar.leg.right.dark.${suffix}`, bipodRootX, 0, footForward, footSpread, { stroke: "#15120f", strokeWidth: 3.2, strokeOpacity: 0.72, animation: baseAnim }),
    line(`part.mortar.leg.left.${suffix}`, bipodRootX, 0, footForward, -footSpread, { stroke: TEAM_GUN_FILL, strokeWidth: 2, strokeOpacity: 0.92, tint: "team-stroke", animation: baseAnim }),
    line(`part.mortar.leg.right.${suffix}`, bipodRootX, 0, footForward, footSpread, { stroke: TEAM_GUN_FILL, strokeWidth: 2, strokeOpacity: 0.92, tint: "team-stroke", animation: baseAnim }),
    rect(`part.mortar.body.${suffix}`, -r * 0.08 - r * 0.29, -r * 0.21, r * 0.58, r * 0.42, {
      fill: TEAM_GUN_FILL,
      fillOpacity: 0.95,
      tint: "team-fill-stroke",
      stroke: TEAM_GUN_FILL,
      strokeWidth: 2,
      strokeOpacity: deploy > 0 ? 0.92 : 0.9,
      animation: baseAnim,
    }),
    rect(`part.mortar.base.${suffix}`, -r * 0.16 - r * 0.17, -r * 0.25, r * 0.34, r * 0.5, {
      fill: TEAM_GUN_FILL,
      fillOpacity: 0.92,
      tint: "team-fill-stroke",
      stroke: TEAM_GUN_FILL,
      strokeWidth: 2,
      strokeOpacity: deploy > 0 ? 0.92 : 0.9,
      animation: baseAnim,
    }),
    line(`part.mortar.tube.${suffix}`, -r * 0.14, 0, muzzleDist, 0, { stroke: "#263f22", strokeWidth: r * 0.22, strokeOpacity: 0.98, animation: tubeAnim }),
    line(`part.mortar.tubeHighlight.${suffix}`, -r * 0.14, -r * 0.08, muzzleDist, -r * 0.08, { stroke: "#58734c", strokeWidth: r * 0.08, strokeOpacity: 0.66, animation: tubeAnim }),
    rect(`part.mortar.muzzle.${suffix}`, muzzleDist - r * 0.08, -r * 0.14, r * 0.16, r * 0.28, {
      fill: "#1c2c19",
      fillOpacity: 0.98,
      stroke: "#58734c",
      strokeWidth: r * 0.08,
      strokeOpacity: 0.66,
      animation: tubeAnim,
    }),
  ];
}

function mortarBasePlateParts(suffix) {
  const plateSize = 16;
  const growAnim = `${WEAPON_FACE};setupVisual:geometry.scaleX:1:-1;setupVisual:geometry.scaleY:1:-1;${DEPLOYED_ALPHA}`;
  return [
    rect(`part.mortar.basePlate.${suffix}`, -plateSize / 2, -plateSize / 2, plateSize, plateSize, {
      fill: "none",
      animation: growAnim,
    }),
  ];
}

function artilleryParts(suffix, deploy, setupAlpha) {
  const halfLen = 25.2;
  const halfWidth = 14.4;
  const trailSpread = halfWidth * (0.18 + deploy * 0.95);
  const trailRear = -halfLen * (0.58 + deploy * 0.62);
  const baseAnim = `${CARRIAGE_FACE};${RECOIL};${ARTILLERY_CARRIAGE_KICK};${setupAlpha}`;
  const barrelAnim = `weaponFacing:transform.rotation:1:0;${RECOIL};${ARTILLERY_WEAPON_KICK};${setupAlpha}`;
  const axleX = -halfLen * 0.18;
  const axleY = halfWidth * 0.82;
  const tireLength = halfWidth * 0.95;
  const tireWidth = halfWidth * 0.42;
  return [
    line(`part.art.axle.${suffix}`, axleX, -axleY, axleX, axleY, { stroke: "#17120e", strokeWidth: 3, strokeOpacity: 0.9, animation: baseAnim }),
    ...gunTireParts("part.art.wheel.left", suffix, axleX, -axleY, tireLength, tireWidth, baseAnim),
    ...gunTireParts("part.art.wheel.right", suffix, axleX, axleY, tireLength, tireWidth, baseAnim),
    line(`part.art.trail.left.${suffix}`, -halfLen * 0.12, 0, trailRear, -trailSpread, { stroke: "#2a2119", strokeWidth: 3.5, strokeOpacity: 0.9, animation: baseAnim }),
    line(`part.art.trail.right.${suffix}`, -halfLen * 0.12, 0, trailRear, trailSpread, { stroke: "#2a2119", strokeWidth: 3.5, strokeOpacity: 0.9, animation: baseAnim }),
    ...(deploy > 0 ? [
      rect(`part.art.foot.left.${suffix}`, trailRear - halfWidth * 0.36, -trailSpread - halfWidth * 0.11, halfWidth * 0.72, halfWidth * 0.22, { fill: "none", stroke: "#15120f", strokeWidth: 2.5, strokeOpacity: 0.82, animation: baseAnim }),
      rect(`part.art.foot.right.${suffix}`, trailRear - halfWidth * 0.36, trailSpread - halfWidth * 0.11, halfWidth * 0.72, halfWidth * 0.22, { fill: "none", stroke: "#15120f", strokeWidth: 2.5, strokeOpacity: 0.82, animation: baseAnim }),
    ] : []),
    line(`part.art.cradleLink.${suffix}`, halfLen * 0.02, 0, -halfLen * 0.12, 0, { stroke: "#5d4a34", strokeWidth: 2, strokeOpacity: 0.78, animation: baseAnim }),
    rect(`part.art.cradle.${suffix}`, halfLen * 0.02 - halfLen * 0.26, -halfWidth * 0.29, halfLen * 0.52, halfWidth * 0.58, {
      fill: TEAM_GUN_FILL,
      fillOpacity: 0.9,
      tint: "team",
      stroke: "#17130f",
      strokeWidth: 1.4,
      strokeOpacity: 0.9,
      animation: baseAnim,
    }),
    rect(`part.art.breechBase.${suffix}`, halfLen * 0.02 - halfLen * 0.1 - halfLen * 0.12, -halfWidth * 0.18, halfLen * 0.24, halfWidth * 0.36, {
      fill: "#2a2119",
      fillOpacity: 0.9,
      stroke: "#17130f",
      strokeWidth: 1.4,
      strokeOpacity: 0.9,
      animation: baseAnim,
    }),
    line(`part.art.barrel.${suffix}`, halfLen * 0.04, 0, halfLen * 1.82, 0, { stroke: DARK, strokeWidth: 8, strokeOpacity: 0.98, animation: barrelAnim }),
    line(`part.art.barrelHighlight.${suffix}`, halfLen * 0.04, -3, halfLen * 1.82, -3, { stroke: METAL, strokeWidth: 2.2, strokeOpacity: 0.62, animation: barrelAnim }),
    rect(`part.art.breech.${suffix}`, halfLen * 0.04 - halfLen * 0.19, -halfWidth * 0.31, halfLen * 0.38, halfWidth * 0.62, {
      fill: "#3d3528",
      fillOpacity: 0.98,
      stroke: METAL,
      strokeWidth: 2.2,
      strokeOpacity: 0.62,
      animation: barrelAnim,
    }),
    rect(`part.art.breechShield.${suffix}`, halfLen * 0.04 - halfLen * 0.18 - halfLen * 0.1, -halfWidth * 0.25, halfLen * 0.2, halfWidth * 0.5, {
      fill: TEAM_GUN_FILL,
      fillOpacity: 0.88,
      tint: "team",
      stroke: METAL,
      strokeWidth: 2.2,
      strokeOpacity: 0.62,
      animation: barrelAnim,
    }),
  ];
}

function artilleryFlashParts() {
  const flashX = 25.2 * 2.16;
  const flashCenter = `weaponFacingCos:transform.x:${fmt(flashX)}:0;weaponFacingSin:transform.y:${fmt(flashX)}:0`;
  const coneScale = "recoilPx:geometry.scaleX:0.1:0;recoilPx:geometry.scaleY:0.084444444444:0";
  const flashAnim = `weaponFacing:transform.rotation:1:0;${RECOIL};${ARTILLERY_WEAPON_KICK};${flashCenter}`;
  return [
    polygon("part.art.flashCone", [[-2.7, 0], [9, -4.5], [9, 4.5]], {
      fill: "#ffd84a",
      fillOpacity: 0.78,
      stroke: METAL,
      strokeWidth: 2.2,
      strokeOpacity: 0.62,
      animation: `${flashAnim};recoilPx:alpha:0.1:0;${coneScale}`,
    }),
    circle("part.art.flashCore", 0, 0, 3.5, {
      fill: "#fff2d0",
      fillOpacity: 0.9,
      stroke: METAL,
      strokeWidth: 2.2,
      strokeOpacity: 0.62,
      animation: `${flashAnim};recoilPx:alpha:0.1:0;recoilPx:geometry.scaleX:0.08:0;recoilPx:geometry.scaleY:0.08:0`,
    }),
    circle("part.art.flashGlow", 0, 0, 6, {
      fill: "#fff06a",
      fillOpacity: 0.58,
      stroke: METAL,
      strokeWidth: 2.2,
      strokeOpacity: 0.62,
      animation: `${flashAnim};recoilPx:alpha:0.1:0;recoilPx:geometry.scaleX:0.066666666667:0;recoilPx:geometry.scaleY:0.066666666667:0`,
    }),
  ];
}

function gunTireParts(prefix, suffix, cx, cy, length, width, animation) {
  const treadOffset = width * 0.32;
  const treadInset = length * 0.26;
  const sideTreadLength = length - treadInset * 2;
  return [
    polygon(`${prefix}.body.${suffix}`, orientedCapsulePolygon(cx, cy, length, width), {
      fill: "#26221b",
      fillOpacity: 0.98,
      stroke: "#17130f",
      strokeWidth: 2.4,
      strokeOpacity: 0.98,
      animation,
    }),
    line(`${prefix}.tread.0.${suffix}`, cx - sideTreadLength / 2, cy - treadOffset, cx + sideTreadLength / 2, cy - treadOffset, { stroke: METAL, strokeWidth: 1.5, strokeOpacity: 0.5, animation }),
    line(`${prefix}.tread.1.${suffix}`, cx - sideTreadLength / 2, cy + treadOffset, cx + sideTreadLength / 2, cy + treadOffset, { stroke: METAL, strokeWidth: 1.5, strokeOpacity: 0.5, animation }),
    ...[-1, 0, 1].map((index) => {
      const x = cx + index * length * 0.2;
      return line(`${prefix}.cross.${index + 1}.${suffix}`, x, cy - width * 0.42, x, cy + width * 0.42, { stroke: "#4a4031", strokeWidth: 1.2, strokeOpacity: 0.9, animation });
    }),
    circle(`${prefix}.hub.${suffix}`, cx, cy, width * 0.32, {
      fill: METAL,
      fillOpacity: 0.76,
      stroke: "#17130f",
      strokeWidth: 1.4,
      strokeOpacity: 0.9,
      animation,
    }),
  ];
}

function supportRig({ kind, id, viewBox, shadow, parts, selection, hp, hpAnchorY, anchors }) {
  return svg({ kind, id, viewBox }, [
    shadow,
    ...parts.flat(),
    ...commonAnchors({ hpY: hpAnchorY, selection, hp, extra: anchors }),
  ]);
}

function commonAnchors({ hpY, selection, hp, extra = {} }) {
  return [
    circle("anchor.origin", 0, 0, 1, { fill: "#ffffff" }),
    circle("anchor.selection", 0, 0, 1, { fill: "#ffffff" }),
    circle("anchor.hp", 0, hpY, 1, { fill: "#ffffff" }),
    ...Object.entries(extra).map(([name, point]) => circle(`anchor.${name}`, point.x, point.y, 1, { fill: "#ffffff" })),
    rect("bounds.selection", selection.x, selection.y, selection.width, selection.height, { fill: "none" }),
    rect("bounds.hp", hp.x, hp.y, hp.width, hp.height, { fill: "none" }),
  ];
}

function supportPartIds(prefix, names, suffix) {
  return names.map((name) => `${prefix}.${name}.${suffix}`);
}

function vehicleShadow(id, rx, ry, drop, animation) {
  const points = [];
  for (let i = 0; i < 24; i += 1) {
    const angle = (Math.PI * 2 * i) / 24;
    points.push([Math.cos(angle) * rx, Math.sin(angle) * ry]);
  }
  return polygon(id, points, { fill: "#000000", opacity: 0.28, transform: `translate(0 ${fmt(drop)})`, animation });
}

function orientedCapsulePolygon(cx, cy, length, width) {
  const radius = width / 2;
  const halfStraight = Math.max(0, length / 2 - radius);
  const points = [];
  const steps = 8;
  for (let i = 0; i <= steps; i += 1) {
    const t = -Math.PI / 2 + (Math.PI * i) / steps;
    points.push([cx + halfStraight + Math.cos(t) * radius, cy + Math.sin(t) * radius]);
  }
  for (let i = 0; i <= steps; i += 1) {
    const t = Math.PI / 2 + (Math.PI * i) / steps;
    points.push([cx - halfStraight + Math.cos(t) * radius, cy + Math.sin(t) * radius]);
  }
  return points;
}

function svg({ kind, id, viewBox }, children) {
  return `<svg viewBox="${viewBox}" data-rts-rig-kind="${kind}" data-rts-rig-version="1" data-rts-origin="center" id="${id}">
${children.flat().join("\n")}
</svg>`;
}

function rect(id, x, y, width, height, options = {}) {
  return element("rect", id, { x, y, width, height }, options);
}

function circle(id, cx, cy, r, options = {}) {
  return element("circle", id, { cx, cy, r }, options);
}

function ellipse(id, cx, cy, rx, ry, options = {}) {
  return element("ellipse", id, { cx, cy, rx, ry }, options);
}

function line(id, x1, y1, x2, y2, options = {}) {
  return element("line", id, { x1, y1, x2, y2 }, options);
}

function polygon(id, points, options = {}) {
  return element("polygon", id, { points: points.map(([x, y]) => `${fmt(x)},${fmt(y)}`).join(" ") }, options);
}

function element(name, id, geometry, options) {
  const attrs = [
    ["id", id],
    ["transform", options.transform],
    ["data-rts-pivot", options.pivot],
    ...Object.entries(geometry),
    ["fill", options.fill],
    ["fill-opacity", options.fillOpacity],
    ["stroke", options.stroke],
    ["stroke-width", options.strokeWidth],
    ["stroke-opacity", options.strokeOpacity],
    ["opacity", options.opacity],
    ["data-rts-tint", options.tint],
    ["data-rts-animation", options.animation],
  ].filter(([, value]) => value !== undefined && value !== null);
  return `  <${name} ${attrs.map(([key, value]) => `${key}="${typeof value === "number" ? fmt(value) : value}"`).join(" ")} />`;
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

function fmt(value) {
  if (typeof value !== "number") return value;
  const rounded = Number(value.toFixed(12));
  return Object.is(rounded, -0) ? "0" : String(rounded);
}
