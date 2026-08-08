// Deliberately coarse presentation-only geometry for projected unit shadows.
//
// Coordinates use +x as unit-forward, +y as unit-right, and +z as height in
// world pixels. `yaw` rotates around z; `pitch` raises the forward end. Each
// part is one rectangular prism and every candidate stays under ten parts.

const box = (id, center, size, { yaw = 0, pitch = 0 } = {}) => Object.freeze({
  id,
  center: Object.freeze(center),
  size: Object.freeze(size),
  yaw,
  pitch,
});

const candidate = (id, label, rationale, parts) => Object.freeze({
  id,
  label,
  rationale,
  parts: Object.freeze(parts),
});

const unit = (kind, label, spriteEnvelope, candidates) => Object.freeze({
  kind,
  label,
  spriteEnvelope: Object.freeze(spriteEnvelope),
  candidates: Object.freeze(candidates),
});

export const PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES = Object.freeze([
  unit("panzerfaust", "Panzerfaust", { length: 41, width: 23 }, [
    candidate("panzerfaust-a", "A · Shoulder tube", "Long offset launcher follows the sprite's dominant forward axis.", [
      box("body", [0, 0, 8], [12, 10, 16]),
      box("head", [3, 0, 19], [6, 6, 6]),
      box("launcher", [7, -6, 16], [38, 3.5, 3.5], { yaw: -0.08 }),
      box("warhead", [25, -7.5, 16], [5, 6, 6], { yaw: -0.08 }),
      box("pack", [-5, 1, 11], [14, 18, 12]),
    ]),
    candidate("panzerfaust-b", "B · Low carry", "A lower tube and wider kit make a denser infantry shadow.", [
      box("body", [0, 0, 8], [12, 10, 16]),
      box("head", [3, 0, 19], [6, 6, 6]),
      box("launcher", [6, 6, 11], [38, 3, 3], { yaw: 0.06 }),
      box("warhead", [24, 7, 11], [6, 5, 5], { yaw: 0.06 }),
      box("rear-pack", [-5, -1, 11], [13, 17, 12]),
      box("forearm", [5, 3, 10], [10, 3, 4], { yaw: 0.2 }),
    ]),
  ]),
  unit("anti_tank_gun", "Anti-Tank Gun", { length: 35, width: 23 }, [
    candidate("anti-tank-gun-a", "A · Split trails", "Wheels, breech, long barrel, and two rear trails retain the sprite silhouette.", [
      box("axle", [0, 0, 5], [5, 22, 4]),
      box("wheel-left", [0, -10, 7], [8, 4, 14]),
      box("wheel-right", [0, 10, 7], [8, 4, 14]),
      box("carriage", [1, 0, 9], [13, 9, 7]),
      box("breech", [5, 0, 13], [8, 7, 7]),
      box("barrel", [11, 0, 15], [20, 3, 3], { pitch: 0.08 }),
      box("trail-left", [-6, -4.5, 3], [16, 3, 4], { yaw: -0.22 }),
      box("trail-right", [-6, 4.5, 3], [16, 3, 4], { yaw: 0.22 }),
    ]),
    candidate("anti-tank-gun-b", "B · Solid carriage", "A broad low carriage and single trail produce a heavier, cleaner shadow.", [
      box("axle", [0, 0, 5], [5, 23, 4]),
      box("wheel-left", [0, -10.5, 7], [8, 4, 14]),
      box("wheel-right", [0, 10.5, 7], [8, 4, 14]),
      box("carriage", [-1, 0, 9], [15, 10, 7]),
      box("breech", [5, 0, 13], [8, 7, 8]),
      box("barrel", [11, 0, 15], [21, 3.5, 3.5], { pitch: 0.08 }),
      box("single-trail", [-6, 0, 3], [16, 7, 4]),
    ]),
  ]),
  unit("mortar_team", "Mortar Team", { length: 34, width: 19 }, [
    candidate("mortar-team-a", "A · Compact tripod", "A short, steeply raised tube gives the packed team a clear mortar shadow.", [
      box("baseplate", [-5, 0, 2], [9, 12, 3]),
      box("tube", [2, 0, 13], [21, 4, 4], { pitch: 0.92 }),
      box("muzzle", [8.5, 0, 21.5], [5, 7, 7], { pitch: 0.92 }),
      box("leg-left", [-1, -5, 4], [15, 3, 3], { yaw: -0.38 }),
      box("leg-right", [-1, 5, 4], [15, 3, 3], { yaw: 0.38 }),
      box("ammo", [-8, 0, 5], [8, 8, 9]),
    ]),
    candidate("mortar-team-b", "B · Wheeled mount", "Small wheels and a steeper tube echo the current cart-like top view.", [
      box("axle", [-4, 0, 5], [5, 17, 4]),
      box("wheel-left", [-4, -7.5, 6], [7, 4, 12]),
      box("wheel-right", [-4, 7.5, 6], [7, 4, 12]),
      box("bed", [-5, 0, 8], [12, 9, 6]),
      box("tube", [3, 0, 15], [20, 4.5, 4.5], { pitch: 1.02 }),
      box("muzzle", [8, 0, 23], [5, 7, 7], { pitch: 1.02 }),
      box("trail", [-11, 0, 3], [12, 5, 4]),
    ]),
  ]),
  unit("artillery", "Artillery", { length: 47, width: 21 }, [
    candidate("artillery-a", "A · Long split trail", "A long upward barrel and paired trails preserve the A-19 sprite's open silhouette.", [
      box("axle", [0, 0, 6], [6, 20, 5]),
      box("wheel-left", [0, -9, 9], [10, 5, 18]),
      box("wheel-right", [0, 9, 9], [10, 5, 18]),
      box("carriage", [2, 0, 11], [15, 10, 9]),
      box("breech", [7, 0, 16], [9, 8, 9]),
      box("barrel", [16, 0, 22], [24, 4, 4], { pitch: 0.4 }),
      box("trail-left", [-8, -4, 4], [20, 4, 5], { yaw: -0.18 }),
      box("trail-right", [-8, 4, 4], [20, 4, 5], { yaw: 0.18 }),
    ]),
    candidate("artillery-b", "B · Box trail", "A single boxed trail makes a chunkier silhouette under the same raised barrel.", [
      box("axle", [0, 0, 6], [6, 21, 5]),
      box("wheel-left", [0, -9.5, 9], [10, 5, 18]),
      box("wheel-right", [0, 9.5, 9], [10, 5, 18]),
      box("carriage", [1, 0, 11], [17, 11, 9]),
      box("breech", [7, 0, 16], [9, 8, 9]),
      box("barrel", [16, 0, 23], [25, 4.5, 4.5], { pitch: 0.44 }),
      box("box-trail", [-9, 0, 4], [18, 9, 5]),
    ]),
  ]),
  unit("command_car", "Command Car", { length: 36, width: 19 }, [
    candidate("command-car-a", "A · One box", "The literal one-prism option matches the compact rectangular sprite.", [
      box("body", [0, 0, 12], [36, 19, 24]),
    ]),
    candidate("command-car-b", "B · Cab boxes", "A low hull, cab, and radio box add height variation without changing the top envelope.", [
      box("hull", [0, 0, 8], [36, 19, 16]),
      box("cab", [5, 0, 19], [18, 16, 9]),
      box("radio-pack", [-12, 0, 17], [9, 15, 8]),
    ]),
  ]),
]);

export const PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET = 10;

export function projectedUnitShadowCandidate(kind, entityId = 0) {
  const unitModel = PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES.find((entry) => entry.kind === kind);
  if (!unitModel) return null;
  return unitModel.candidates[Math.abs(Number(entityId) || 0) % unitModel.candidates.length];
}
