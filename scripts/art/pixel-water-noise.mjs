// Bake Jess's WaterMaterial.tres noise settings with the pinned official FastNoiseLite module.
import { pathToFileURL, fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";

if (!process.argv[2]) throw new Error("Usage: node scripts/art/pixel-water-noise.mjs /path/to/FastNoiseLite.mjs");
const { default: FastNoiseLite } = await import(pathToFileURL(path.resolve(process.argv[2])).href);
const { PNG } = createRequire(import.meta.url)("pngjs");
const outputDir = fileURLToPath(new URL("../../client/assets/water/pixel-water/", import.meta.url));
const settings = [
  ["movement", 0, .0021, 3, 2, .5, false],
  ["fade", 0, .02, 2, 2, .5, true],
  ["specular-a", 0, .001, 4, 4, 1, true],
  ["specular-b", 1, .0012, 4, 4, 1, true],
  ["foam", 0, .0191, 5, 2, .5, true],
];
const lerp = (a, b, t) => a + (b - a) * t;
for (const [name, seed, frequency, octaves, lacunarity, gain, normalize] of settings) {
  const noise = new FastNoiseLite(seed);
  noise.SetNoiseType(FastNoiseLite.NoiseType.OpenSimplex2S);
  noise.SetFrequency(frequency);
  noise.SetFractalType(FastNoiseLite.FractalType.FBm);
  noise.SetFractalOctaves(octaves);
  noise.SetFractalLacunarity(lacunarity);
  noise.SetFractalGain(gain);
  const size = 512;
  const values = new Float32Array(size * size);
  let min = 1;
  let max = -1;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const fx = x / size;
      const fy = y / size;
      const value = lerp(
        lerp(noise.GetNoise(x, y), noise.GetNoise(x - size, y), fx),
        lerp(noise.GetNoise(x, y - size), noise.GetNoise(x - size, y - size), fx), fy,
      );
      values[y * size + x] = value;
      min = Math.min(min, value);
      max = Math.max(max, value);
    }
  }
  const png = new PNG({ width: size, height: size });
  for (let i = 0; i < values.length; i++) {
    const value = normalize ? (values[i] - min) / (max - min) : values[i] * .5 + .5;
    png.data[i * 4] = png.data[i * 4 + 1] = png.data[i * 4 + 2] = Math.round(value * 255);
    png.data[i * 4 + 3] = 255;
  }
  fs.writeFileSync(path.join(outputDir, `${name}.png`), PNG.sync.write(png));
  console.log(`${name}: range ${min} to ${max}`);
}
