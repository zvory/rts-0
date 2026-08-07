import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const WIDTH = 126;
const HEIGHT = 126;
const elevation = Array.from({ length: HEIGHT }, () => new Uint8Array(WIDTH));
const terrain = Array.from({ length: HEIGHT }, () => Array(WIDTH).fill("."));

function raise(x, y, level) {
  if (x < 0 || y < 0 || x >= WIDTH || y >= HEIGHT) return;
  elevation[y][x] = Math.max(elevation[y][x], Math.max(0, Math.min(9, Math.round(level))));
}

function dome(cx, cy, radius, peak, exponent = 1) {
  for (let y = Math.floor(cy - radius); y <= Math.ceil(cy + radius); y += 1) {
    for (let x = Math.floor(cx - radius); x <= Math.ceil(cx + radius); x += 1) {
      const distance = Math.hypot(x - cx, y - cy) / radius;
      if (distance > 1) continue;
      raise(x, y, Math.ceil(peak * Math.pow(1 - distance, exponent)));
    }
  }
}

function ridge(x0, x1, centerY, halfWidth, peak) {
  for (let y = centerY - halfWidth; y <= centerY + halfWidth; y += 1) {
    const level = Math.ceil(peak * (1 - Math.abs(y - centerY) / (halfWidth + 1)));
    for (let x = x0; x <= x1; x += 1) raise(x, y, level);
  }
}

function terrace(cx, cy, halfWidth, halfHeight, peak) {
  for (let ring = 0; ring < peak; ring += 1) {
    const left = cx - halfWidth + ring * 2;
    const right = cx + halfWidth - ring * 2;
    const top = cy - halfHeight + ring * 2;
    const bottom = cy + halfHeight - ring * 2;
    for (let y = top; y <= bottom; y += 1) {
      for (let x = left; x <= right; x += 1) raise(x, y, ring + 1);
    }
  }
}

function flatten(cx, cy, radius) {
  for (let y = cy - radius; y <= cy + radius; y += 1) {
    for (let x = cx - radius; x <= cx + radius; x += 1) {
      if (x >= 0 && y >= 0 && x < WIDTH && y < HEIGHT) elevation[y][x] = 0;
    }
  }
}

// Row 1: shallow rounded dome, severe rounded peak, and long parallel ridges.
dome(21, 21, 17, 4, 1.45);
dome(63, 21, 16, 9, 0.72);
ridge(87, 120, 15, 6, 6);
ridge(87, 120, 29, 6, 6);

// Row 2: crater/ring, saddle between two peaks, and stepped mesa.
for (let y = 45; y <= 79; y += 1) {
  for (let x = 4; x <= 38; x += 1) {
    const distance = Math.hypot(x - 21, y - 62);
    if (distance >= 8 && distance <= 17) raise(x, y, 6 - Math.abs(distance - 12.5));
  }
}
dome(54, 62, 15, 7, 1.05);
dome(72, 62, 15, 7, 1.05);
terrace(105, 62, 17, 16, 6);

// Row 3: rolling country, a hard escarpment with a ramped end, and a high canyon.
for (const [x, y, radius, peak] of [
  [10, 98, 10, 3], [24, 92, 13, 5], [35, 105, 11, 4], [15, 116, 9, 4], [31, 119, 8, 3],
]) dome(x, y, radius, peak, 1.2);
for (let y = 87; y <= 120; y += 1) {
  for (let x = 47; x <= 79; x += 1) {
    const ramp = x < 54 ? Math.ceil((x - 46) * 7 / 8) : 7;
    raise(x, y, ramp);
  }
}
for (let y = 86; y <= 122; y += 1) {
  const canyonCenter = 105 + Math.round(Math.sin((y - 86) * 0.22) * 6);
  for (let x = 87; x <= 123; x += 1) {
    const distance = Math.abs(x - canyonCenter);
    if (distance >= 5) raise(x, y, Math.min(8, 2 + Math.floor((distance - 5) / 2)));
  }
}

flatten(12, 12, 6);
flatten(113, 113, 6);

// Gravel separators and flat staging aprons between terrain studies.
for (const separator of [42, 84]) {
  for (let x = 0; x < WIDTH; x += 1) terrain[separator][x] = "0";
  for (let y = 0; y < HEIGHT; y += 1) terrain[y][separator] = "0";
}
for (let x = 0; x < WIDTH; x += 1) {
  terrain[2][x] = "=";
  terrain[123][x] = "=";
}

const map = {
  version: 10,
  name: "Lighting Test",
  description: "A 3x3 atlas of shallow, severe, rounded, ridged, terraced, cliff, crater, saddle, rolling, and canyon elevation forms for live shadow experiments.",
  _design: "NW gentle dome; N severe peak; NE parallel ridges; W crater; center saddle; E terraced mesa; SW rolling hills; S hard escarpment; SE winding canyon. Gravel lines separate studies. Sun is northwesterly at a 12-degree sunset angle.",
  width: WIDTH,
  height: HEIGHT,
  terrain: terrain.map((row) => row.join("")),
  elevation: elevation.map((row) => Array.from(row, String).join("")),
  sun: { azimuthDegrees: 330, elevationDegrees: 12, warmth: 78 },
  startLocations: [{ x: 12, y: 12 }, { x: 113, y: 113 }],
  baseSites: [
    { x: 12, y: 12, steelPatches: 6, oilPatches: 2 },
    { x: 113, y: 113, steelPatches: 6, oilPatches: 2 },
  ],
  doodads: [],
  forestSpans: [],
  concealmentTiles: [],
  noVehicleTiles: [],
  noBuildingTiles: [],
  noEntrenchmentTiles: [],
  damageReductionTiles: [],
  slowMovementTiles: [],
};

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.join(root, "server/assets/maps/lighting-test.json");
await writeFile(output, `${JSON.stringify(map, null, 2)}\n`);
console.log(output);
