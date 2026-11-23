import assert from "node:assert/strict";
import { createGrassBakedCarpetMaterial } from "../../client/src/renderer/grass_baked_carpet.js";

import { GRASS_LIGHTING_GLSL } from "../../client/src/renderer/grass_lighting.js";
import { GrassLayer } from "../../client/src/renderer/grass.js";

const previousCanvas = globalThis.OffscreenCanvas;
class RecordingCanvas {
  constructor(width, height) {
    this.width = width;
    this.height = height;
    this.poses = [];
    this.images = [];
    let pose, points;
    this.context = {
      save() {}, restore() {}, clip() {}, closePath() {},
      rect: (_x, y) => { pose = []; this.poses[y / 256] = pose; },
      beginPath: () => { points = []; },
      moveTo: (x, y) => points.push([x, y]),
      lineTo: (x, y) => points.push([x, y]),
      fill: () => pose.push({ color: this.context.fillStyle, points }),
      createImageData: (w, h) => ({ data: new Uint8ClampedArray(w * h * 4) }),
      putImageData: (image, x, y) => this.images.push({ x, y, pixels: image.data.slice() }),
    };
  }
  getContext(type) { assert.equal(type, "2d"); return this.context; }
}

try {
  globalThis.OffscreenCanvas = RecordingCanvas;
  function make(lighting) {
    const textures = [], freed = [];
    const material = createGrassBakedCarpetMaterial({ Texture: { from(canvas) {
      const texture = { source: {}, destroy(deep) { freed.push({ canvas, deep }); } };
      textures.push({ canvas, texture });
      return texture;
    } } }, lighting);
    return { material, textures, freed };
  }

  const first = make(true), repeat = make(false);
  const [atlas, wind] = first.textures.map(({ canvas }) => canvas);
  assert.deepEqual([atlas.width, atlas.height], [256, 2048]);
  assert.deepEqual([wind.width, wind.height], [128, 128]);
  assert.equal(first.material.textureBytes, (atlas.width * atlas.height + wind.width * wind.height) * 4);
  assert.equal(first.material.resources.uGrassPoses, first.textures[0].texture.source);
  assert.equal(first.material.resources.uWind, first.textures[1].texture.source);
  assert.equal(first.textures[0].texture.source.scaleMode, "nearest");
  assert.equal(first.textures[1].texture.source.scaleMode, "linear");
  assert(first.textures.every(({ texture }) => texture.source.addressMode === "repeat"));
  assert(first.material.fragment.includes("uniform sampler2D uTerrainLight;"));
  assert(!repeat.material.fragment.includes("uTerrainLight"), "flat maps do not need a missing lighting sampler");

  // The atlas must use the same world-space atmosphere as the procedural reference.
  // The tileable wind lookup only controls blades; using its shade channel caused repetition.
  let proceduralFragment;
  const reference = new GrassLayer({
    Geometry: class { destroy() {} },
    UniformGroup: class {},
    Shader: { from({ gl }) { proceduralFragment = gl.fragment; return { destroy() {} }; } },
    Mesh: class { destroy() {} },
  }, { addChild() {} }, { width: 1, height: 1, tileSize: 32, terrain: [0] }, 5, 0xfff3e2);
  assert(proceduralFragment.includes(GRASS_LIGHTING_GLSL));
  assert(first.material.fragment.includes(GRASS_LIGHTING_GLSL));
  assert(first.material.fragment.includes("color=grassAtmosphere(p,color,1.);"));
  assert(!first.material.fragment.includes("field.g"), "repeating wind must not shade clouds");
  reference.destroy();

  assert.equal(atlas.poses.length, 8);
  assert.deepEqual(atlas.images.map(({ y }) => y), Array.from({ length: 8 }, (_, i) => i * 256));
  for (const { pixels } of atlas.images) {
    assert.deepEqual(pixels, atlas.images[0].pixels, "every pose starts with the same ground");
    for (let i = 3; i < pixels.length; i += 4) assert.equal(pixels[i], 255);
  }
  const roots = atlas.poses.map((draws, frame) => {
    assert.equal(draws.length % 4, 0, "each wrapped root draws one shadow and three blades");
    return draws.filter((_, i) => i % 4 === 0).map(({ points }) => [points[0][0] + 3, points[0][1] - 1 - frame * 256]);
  });
  assert(roots[0].some(([, y]) => y < 0), "top-edge shadow copies are retained");
  assert(roots[0].some(([, y]) => y > 256), "bottom-edge blades can extend into the tile");
  for (const pose of roots) {
    assert.equal(pose.length, roots[0].length);
    // Adding/subtracting the atlas-row offset can round JavaScript doubles slightly.
    for (let i = 0; i < pose.length; i++) for (let axis = 0; axis < 2; axis++) {
      assert(Math.abs(pose[i][axis] - roots[0][i][axis]) < 1e-10, "roots remain anchored across all bending poses");
    }
    for (let i = 1; i < pose.length; i++) assert(pose[i][1] >= pose[i - 1][1], "wrapped copies use destination painter order");
  }
  const silhouettes = atlas.poses.map((draws, frame) => JSON.stringify(draws.filter((_, i) => i % 4 !== 0)
    .map(({ points }) => points.map(([x, y]) => [x.toFixed(8), (y - frame * 256).toFixed(8)]))));
  // The two neutral crossings of a wind cycle may share a silhouette.
  assert(new Set(silhouettes).size >= 4, "the atlas contains multiple distinguishable bending poses");
  assert.deepEqual(atlas.poses, repeat.textures[0].canvas.poses, "artwork generation is deterministic");
  assert.deepEqual(wind.images, repeat.textures[1].canvas.images, "wind fields are deterministic");
  first.material.destroy();
  repeat.material.destroy();
  assert.deepEqual(first.freed, first.textures.map(({ canvas }) => ({ canvas, deep: true })));
  assert.equal(repeat.freed.length, 2, "both owned textures are released");
} finally {
  globalThis.OffscreenCanvas = previousCanvas;
}

console.log("grass atlas poses, seam ordering, lighting variants and texture lifecycle passed");
