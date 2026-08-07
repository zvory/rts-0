// Review-only candidate geometry for the next projected-unit-shadow pass.
//
// Coordinates use the production shadow convention: +x is unit-forward, +y is
// unit-right, and +z is presentation-only height in world pixels. Every part is
// a rectangular prism so the eventual GPU proxy can stay deliberately coarse.

const box = (id, center, size, yaw = 0) => Object.freeze({
  id,
  center: Object.freeze(center),
  size: Object.freeze(size),
  yaw,
});

const candidate = (id, label, rationale, parts) => Object.freeze({
  id,
  label,
  rationale,
  parts: Object.freeze(parts),
});

const scaleFootprint = (parts, scaleX, scaleY = scaleX) => parts.map((part) => box(
  part.id,
  [part.center[0] * scaleX, part.center[1] * scaleY, part.center[2]],
  [part.size[0] * scaleX, part.size[1] * scaleY, part.size[2]],
  part.yaw,
));

const unit = (kind, label, spriteEnvelope, candidates) => Object.freeze({
  kind,
  label,
  spriteEnvelope: Object.freeze(spriteEnvelope),
  candidates: Object.freeze(candidates),
});

export const PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES = Object.freeze([
  unit("worker", "Engineer", { length: 29, width: 18 }, [
    candidate("worker-a", "A · Tool-forward", "Long tool preserves the current diagonal working silhouette.", [
      box("body", [0, 0, 8], [11, 8, 16]),
      box("head", [3, 0, 19], [6, 6, 6]),
      box("pack", [-5, 0, 11], [6, 9, 10]),
      box("tool-shaft", [7, 4, 8], [19, 2.5, 2.5], -0.35),
      box("tool-head", [15, 1, 8], [4, 8, 3], -0.35),
    ]),
    candidate("worker-b", "B · Pack-heavy", "Broader rear mass and shorter carried tool read more compactly.", [
      box("body", [0, 0, 8], [12, 9, 16]),
      box("head", [3, 0, 19], [6, 6, 6]),
      box("pack", [-6, 0, 12], [8, 11, 12]),
      box("arm", [4, 5, 11], [10, 3, 4], 0.18),
      box("tool-shaft", [8, 6, 9], [16, 2.5, 2.5], 0.18),
      box("tool-head", [15, 7, 9], [4, 7, 3], 0.18),
    ]),
  ]),
  unit("panzerfaust", "Panzerfaust", { length: 41, width: 23 }, [
    candidate("panzerfaust-a", "A · Shoulder tube", "A long offset launcher follows the production sprite's dominant axis.", [
      box("body", [0, 0, 8], [12, 10, 16]),
      box("head", [3, 0, 19], [6, 6, 6]),
      box("launcher", [7, -6, 16], [38, 3.5, 3.5], -0.08),
      box("warhead", [25, -7, 16], [5, 5, 5], -0.08),
      box("pack", [-5, 1, 11], [14, 18, 12]),
    ]),
    candidate("panzerfaust-b", "B · Low carry", "Lower launcher and a wider torso make the shadow less top-heavy.", [
      box("body", [0, 0, 8], [12, 10, 16]),
      box("head", [3, 0, 19], [6, 6, 6]),
      box("launcher", [6, 6, 11], [38, 3, 3], 0.06),
      box("warhead", [24, 7, 11], [6, 5, 5], 0.06),
      box("rear-pack", [-5, -1, 11], [13, 17, 12]),
    ]),
  ]),
  unit("anti_tank_gun", "Anti-Tank Gun", { length: 35, width: 23 }, [
    candidate("anti-tank-gun-a", "A · Split trails", "Eight boxes retain wheels, breech, long barrel, and split rear trails.", scaleFootprint([
      box("axle", [0, 0, 7], [8, 36, 5]),
      box("wheel-left", [0, -18, 8], [10, 6, 16]),
      box("wheel-right", [0, 18, 8], [10, 6, 16]),
      box("carriage", [2, 0, 11], [16, 13, 8]),
      box("breech", [8, 0, 15], [11, 9, 8]),
      box("barrel", [25, 0, 17], [37, 4, 4]),
      box("trail-left", [-18, -9, 4], [32, 5, 5], -0.25),
      box("trail-right", [-18, 9, 4], [32, 5, 5], 0.25),
    ], 0.45, 0.55)),
    candidate("anti-tank-gun-b", "B · Solid carriage", "A broader low carriage yields a denser, less spidery shadow.", scaleFootprint([
      box("axle", [0, 0, 7], [9, 38, 5]),
      box("wheel-left", [0, -18, 8], [11, 6, 16]),
      box("wheel-right", [0, 18, 8], [11, 6, 16]),
      box("carriage", [-2, 0, 10], [24, 16, 8]),
      box("breech", [9, 0, 15], [12, 10, 9]),
      box("barrel", [27, 0, 17], [40, 4.5, 4.5]),
      box("single-trail", [-20, 0, 4], [34, 10, 5]),
    ], 0.42, 0.55)),
  ]),
  unit("mortar_team", "Mortar Team", { length: 34, width: 19 }, [
    candidate("mortar-team-a", "A · Packed cart", "A narrow tube over a wheeled cart mirrors the packed gameplay sprite.", scaleFootprint([
      box("axle", [-2, 0, 6], [7, 29, 4]),
      box("wheel-left", [-2, -14, 7], [9, 5, 14]),
      box("wheel-right", [-2, 14, 7], [9, 5, 14]),
      box("bed", [-3, 0, 10], [23, 11, 7]),
      box("tube", [13, 0, 16], [31, 4.5, 4.5]),
      box("muzzle", [27, 0, 16], [4, 7, 7]),
      box("trail", [-17, 0, 5], [18, 6, 5]),
    ], 0.61, 0.58)),
    candidate("mortar-team-b", "B · High tube", "A shorter raised tube makes the mortar identity clearer in side projection.", scaleFootprint([
      box("axle", [-3, 0, 6], [7, 30, 4]),
      box("wheel-left", [-3, -14, 7], [9, 5, 14]),
      box("wheel-right", [-3, 14, 7], [9, 5, 14]),
      box("base", [-4, 0, 9], [18, 14, 7]),
      box("tube", [10, 0, 20], [30, 5, 5]),
      box("support", [1, 0, 14], [6, 9, 13]),
      box("trail", [-17, 0, 4], [19, 8, 5]),
    ], 0.65, 0.58)),
  ]),
  unit("artillery", "Artillery", { length: 47, width: 21 }, [
    candidate("artillery-a", "A · Long split trail", "The longest footprint in the set follows the A-19 sprite's barrel and trails.", scaleFootprint([
      box("axle", [0, 0, 8], [10, 40, 6]),
      box("wheel-left", [0, -20, 10], [13, 7, 20]),
      box("wheel-right", [0, 20, 10], [13, 7, 20]),
      box("carriage", [1, 0, 13], [24, 17, 10]),
      box("breech", [12, 0, 19], [15, 12, 11]),
      box("barrel", [34, 0, 22], [47, 5, 5]),
      box("trail-left", [-24, -8, 5], [39, 6, 6], -0.18),
      box("trail-right", [-24, 8, 5], [39, 6, 6], 0.18),
    ], 0.46, 0.45)),
    candidate("artillery-b", "B · Box trail", "A single rear beam reduces gaps while preserving the same sprite envelope.", scaleFootprint([
      box("axle", [0, 0, 8], [10, 41, 6]),
      box("wheel-left", [0, -20, 10], [13, 7, 20]),
      box("wheel-right", [0, 20, 10], [13, 7, 20]),
      box("carriage", [-1, 0, 13], [27, 19, 10]),
      box("breech", [13, 0, 19], [16, 13, 11]),
      box("barrel", [35, 0, 22], [48, 5.5, 5.5]),
      box("box-trail", [-25, 0, 5], [41, 13, 6]),
    ], 0.45, 0.45)),
  ]),
  unit("command_car", "Command Car", { length: 36, width: 19 }, [
    candidate("command-car-a", "A · One box", "The literal one-prism option matches the compact rectangular sprite.", [
      box("body", [0, 0, 12], [36, 19, 24]),
    ]),
    candidate("command-car-b", "B · Cab box", "A low hull plus cab adds height variation without changing the top envelope.", [
      box("hull", [0, 0, 8], [36, 19, 16]),
      box("cab", [5, 0, 20], [18, 16, 10]),
      box("radio-pack", [-12, 0, 18], [9, 15, 8]),
    ]),
  ]),
  unit("ekat", "Ekat", { length: 36, width: 19 }, [
    candidate("ekat-a", "A · Orb staff", "The long staff and terminal orb preserve the live sprite's lateral reach.", scaleFootprint([
      box("body", [-5, 0, 10], [14, 9, 20]),
      box("head", [-1, 0, 23], [7, 7, 7]),
      box("cape", [-11, 0, 11], [14, 17, 13]),
      box("arm", [2, 5, 14], [15, 3, 4], 0.12),
      box("staff", [10, 5, 12], [29, 2.5, 2.5], 0.12),
      box("orb", [23, 7, 12], [7, 7, 7]),
    ], 0.8, 0.8)),
    candidate("ekat-b", "B · Broad cloak", "The wider low cloak gives her shadow more hero-scale body mass.", scaleFootprint([
      box("body", [-4, 0, 10], [15, 10, 20]),
      box("head", [0, 0, 23], [7, 7, 7]),
      box("cloak-left", [-11, -6, 9], [17, 8, 15], -0.2),
      box("cloak-right", [-11, 6, 9], [17, 8, 15], 0.2),
      box("staff", [9, 0, 13], [30, 3, 3]),
      box("orb", [23, 0, 13], [7, 8, 8]),
    ], 0.8, 0.8)),
  ]),
  unit("golem", "Golem", { length: 17, width: 16 }, [
    candidate("golem-a", "A · Six-block brute", "A compact torso with broad arms matches the small square sprite envelope.", [
      box("torso", [0, 0, 13], [13, 11, 26]),
      box("head", [3, 0, 30], [8, 8, 9]),
      box("arm-left", [0, -5.5, 14], [12, 5, 12]),
      box("arm-right", [0, 5.5, 14], [12, 5, 12]),
      box("leg-left", [-4, -3, 5], [8, 5, 10]),
      box("leg-right", [-4, 3, 5], [8, 5, 10]),
    ]),
    candidate("golem-b", "B · Monolith", "Three large boxes create a heavier, simpler shadow with fewer gaps.", [
      box("lower", [-2, 0, 8], [14, 13, 16]),
      box("upper", [1, 0, 21], [16, 16, 15]),
      box("head", [4, 0, 34], [8, 8, 11]),
      box("fist-left", [4, -6.5, 13], [7, 4, 10]),
      box("fist-right", [4, 6.5, 13], [7, 4, 10]),
    ]),
  ]),
]);

export const PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET = 10;
