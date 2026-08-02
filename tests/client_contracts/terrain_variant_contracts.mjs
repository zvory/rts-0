import { assert } from "./assertions.mjs";
import { TERRAIN_VARIANT_PALETTES } from "../../client/src/config.js";
import { TERRAIN } from "../../client/src/protocol.js";
import {
  isImpassableTerrain,
  roadMarkingOrientation,
  terrainColor,
} from "../../client/src/renderer/terrain_palette.js";

const variants = [
  TERRAIN.GRAVEL_A, TERRAIN.GRAVEL_B, TERRAIN.GRAVEL_C,
  TERRAIN.DIRT_A, TERRAIN.DIRT_B, TERRAIN.DIRT_C,
  TERRAIN.MUD_A, TERRAIN.MUD_B, TERRAIN.MUD_C, TERRAIN.FROSTED_GROUND,
];

for (const code of variants) {
  const palette = TERRAIN_VARIANT_PALETTES[code];
  const color = terrainColor(code, 2, 3);
  assert(palette && (color === palette.base || color === palette.alt), `open terrain ${code} uses its production palette`);
  assert(!isImpassableTerrain(code), `open terrain ${code} renders as passable`);
  assert(roadMarkingOrientation(code) === null, `open terrain ${code} has no road marking`);
}
