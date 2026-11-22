import assert from "node:assert/strict";
import { getVisualProfile, resolveVisualProfileLaunch } from "../../client/src/visual_profiles.js";
import { reconcileGrassSurvey, destroyGrassSurvey } from "../../client/src/renderer/grass_survey.js";

// Normal matches never allocate a grass prototype or reveal terrain through profile resolution.
assert.equal(resolveVisualProfileLaunch(null).profile, null);
const ordinary = {};
reconcileGrassSurvey(ordinary, 0);
assert.equal(ordinary._grassSurveyLayer, null);
for (let i = 1; i <= 5; i++) {
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
