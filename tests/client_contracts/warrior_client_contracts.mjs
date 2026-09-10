import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import { assert } from "./assertions.mjs";
import { VisualEffectBuffers } from "../../client/src/state_visual_effects.js";
import { EVENT, WEAPON_KIND } from "../../client/src/protocol.js";

const effects = new VisualEffectBuffers();
effects.applySnapshotEvents([{
  e: EVENT.ATTACK,
  from: 10,
  to: 11,
  weaponKind: WEAPON_KIND.WARRIOR_SWORD,
}], 1000, () => null);

assert(effects.muzzleFlashes.length === 0, "Warrior sword attacks create no tracer or muzzle flash");
assert(
  effects.weaponRecoilKind(10) === WEAPON_KIND.WARRIOR_SWORD,
  "Warrior sword attacks still drive the authored swipe frame",
);

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const strip = PNG.sync.read(fs.readFileSync(path.join(
  repoRoot,
  "client/assets/rigs/warrior-placeholder-pass-01/generated/warrior-runtime-strip.png",
)));
assert(strip.width === 960 && strip.height === 160, "Warrior runtime strip preserves six 160px cells");

for (let frame = 0; frame < 6; frame += 1) {
  for (let y = 0; y < 160; y += 1) {
    for (let x = 0; x < 160; x += 1) {
      if (x >= 5 && x < 155 && y >= 5 && y < 155) continue;
      assert(alphaAt(strip, frame * 160 + x, y) === 0, `Warrior frame ${frame} keeps a transparent sampling gutter`);
    }
  }
  assert(!hasEnclosedTransparency(strip, frame), `Warrior frame ${frame} has no keyed holes inside its silhouette`);
}

for (let index = 0; index < strip.width * strip.height; index += 1) {
  const offset = index * 4;
  if (strip.data[offset + 3] === 0) continue;
  const magentaDominance = Math.min(strip.data[offset], strip.data[offset + 2]) - strip.data[offset + 1];
  assert(magentaDominance < 20, "Warrior visible pixels contain no chroma-key magenta spill");
}

function alphaAt(image, x, y) {
  return image.data[(y * image.width + x) * 4 + 3];
}

function hasEnclosedTransparency(image, frame) {
  const size = 160;
  const exterior = new Uint8Array(size * size);
  const queue = [];
  const enqueue = (x, y) => {
    const local = y * size + x;
    if (exterior[local] || alphaAt(image, frame * size + x, y) >= 16) return;
    exterior[local] = 1;
    queue.push(local);
  };
  for (let x = 0; x < size; x += 1) {
    enqueue(x, 0);
    enqueue(x, size - 1);
  }
  for (let y = 1; y < size - 1; y += 1) {
    enqueue(0, y);
    enqueue(size - 1, y);
  }
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const local = queue[cursor];
    const x = local % size;
    const y = Math.floor(local / size);
    if (x > 0) enqueue(x - 1, y);
    if (x + 1 < size) enqueue(x + 1, y);
    if (y > 0) enqueue(x, y - 1);
    if (y + 1 < size) enqueue(x, y + 1);
  }
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      if (alphaAt(image, frame * size + x, y) < 16 && !exterior[y * size + x]) return true;
    }
  }
  return false;
}
