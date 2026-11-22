import assert from "node:assert/strict";
import { getVisualProfile, resolveVisualProfileLaunch } from "../../client/src/visual_profiles.js";
import { reconcileGrassSurvey, destroyGrassSurvey } from "../../client/src/renderer/grass_survey.js";

// Normal matches never allocate a grass prototype or reveal terrain through profile resolution.
assert.equal(resolveVisualProfileLaunch(null).profile, null);
const ordinary = {};
reconcileGrassSurvey(ordinary, 0);
assert.equal(ordinary._grassSurveyLayer, null);
for (let i = 1; i <= 11; i++) {
  const profile = getVisualProfile(`grass-survey-${i}`);
  assert.equal(profile.grassTechnique, i);
  assert.equal(profile.terrainPreviewReveal, true);
}

// A real material must cover grass only, use the capture clock, and release its GPU resources.
const priorPixi = globalThis.PIXI;
const destroyed = [];
let attributes;
try {
  globalThis.PIXI = {
    UniformGroup: class { constructor(fields) { this.uniforms = Object.fromEntries(Object.entries(fields).map(([k,v]) => [k,v.value])); } },
    Geometry: class { constructor(options) { attributes = options.attributes; } destroy() { destroyed.push("geometry"); } },
    Shader: { from: () => ({ destroy() { destroyed.push("shader"); } }) },
    Mesh: class { destroy() { destroyed.push("mesh"); } },
  };
  const layer = { addChild(mesh) { mesh.parent = this; }, removeChild(mesh) { mesh.parent = null; } };
  const renderer = { _map: { width: 2, height: 1, tileSize: 32, terrain: [0, 2] }, layers: { terrain: layer }, world: { tint: 0xffffff }, visualNow: () => 1234 };
  reconcileGrassSurvey(renderer, 3);
  assert.equal(attributes.aPosition.buffer.length, 12, "only one grass tile produces geometry");
  assert.equal(renderer._grassSurveyLayer.uniforms.uniforms.uTime, 1.234);
  const active = renderer._grassSurveyLayer;
  reconcileGrassSurvey(renderer, 3);
  assert.equal(renderer._grassSurveyLayer, active, "ordinary frames reuse geometry");
  reconcileGrassSurvey(renderer, 0);
  assert.deepEqual(destroyed, ["mesh", "geometry", "shader"]);
  destroyGrassSurvey(renderer);
  assert.equal(destroyed.length, 3, "teardown is idempotent");
} finally {
  globalThis.PIXI = priorPixi;
}
console.log("grass survey contracts passed");

// Texture-based density is a map-load cost: animation reuses both textures and frees them on exit.
const priorCanvas = globalThis.OffscreenCanvas;
try {
  const context = { save() {}, restore() {}, beginPath() {}, rect() {}, clip() {},
    fillRect() {}, moveTo() {}, lineTo() {}, closePath() {}, fill() {}, quadraticCurveTo() {}, stroke() {}, putImageData() {},
    createImageData(w,h) { return { data:new Uint8ClampedArray(w*h*4) }; } };
  globalThis.OffscreenCanvas = class { constructor(w,h) { this.width=w; this.height=h; } getContext() { return context; } };
  const { createGrassCarpetMaterial } = await import("../../client/src/renderer/grass_carpet.js");
  for (const technique of [6,7,8]) {
    const sources = [], freed = [];
    const material = createGrassCarpetMaterial({ Texture: { from(canvas) {
      const texture = { source:{}, destroy(deep) { freed.push(deep); } };
      sources.push({ width:canvas.width, height:canvas.height, texture });
      return texture;
    } } },technique);
    assert.equal(sources.length,2);
    assert.equal(sources[0].height, technique===8 ? 2048 : 256);
    assert.equal(material.textureBytes, sources.reduce((total,s) => total+s.width*s.height*4,0));
    assert.equal(sources[0].texture.source.addressMode,"repeat");
    assert.equal(sources[1].texture.source.scaleMode,"linear");
    material.destroy();
    assert.deepEqual(freed,[true,true]);
  }
  const { createGrassGroundMaterial, grassGroundPixels } = await import("../../client/src/renderer/grass_ground.js");
  const fields=[];
  for (const technique of [9,10,11]) {
    const field=grassGroundPixels(technique);
    assert.deepEqual(field.pixels,grassGroundPixels(technique).pixels,"static ground generation is deterministic");
    assert.equal(field.width*field.height*4,field.pixels.length);
    for(let i=3;i<field.pixels.length;i+=4) assert.equal(field.pixels[i],255);
    fields.push(field.pixels);
    let created=0, destroyed=0;
    const material=createGrassGroundMaterial({ Texture:{ from() {
      created++;
      return { source:{},destroy(deep) { assert.equal(deep,true);destroyed++; } };
    } } },technique);
    assert.equal(created,1,"ground variation needs only one owned texture");
    assert.equal(material.textureBytes,256*256*4);
    assert.equal(material.resources.uGround.scaleMode,"linear");
    material.destroy();
    assert.equal(destroyed,1);
  }
  assert.notDeepEqual(fields[0],fields[1]);
  assert.notDeepEqual(fields[1],fields[2]);
} finally { globalThis.OffscreenCanvas = priorCanvas; }
