import assert from "node:assert/strict";
import { createDaoCorona } from "../../client/src/renderer/dao_corona.js";

let filtersCreated = 0;
const pixi = {
  Graphics: class {
    clear() { return this; }
    rect(...bounds) { this.bounds = bounds; return this; }
    fill() { return this; }
    destroy() { this.destroyed = true; }
  },
  UniformGroup: class {
    constructor(schema) { this.uniforms = Object.fromEntries(Object.entries(schema).map(([key, value]) => [key, value.value])); }
  },
  GlProgram: { from: (program) => program },
  Filter: class {
    constructor(options) { filtersCreated++; Object.assign(this, options); }
    destroy() { this.destroyed = true; }
  },
};

const corona = createDaoCorona(pixi);
corona.update({}, 20, 0);
assert.equal(corona.visible, false);
assert.equal(filtersCreated, 0, "inactive units allocate no GPU filter");
corona.update({ daoReady: true }, 20, 1200);
assert.equal(corona.visible, true);
assert.equal(corona.blendMode, "add");
assert.equal(corona.coronaUniforms.uniforms.uTime, 1.2);
assert.deepEqual(corona.bounds, [-32, -12, 64, 64]);
const filter = corona.coronaFilter;
corona.update({ daoAdvanceTicks: 15 }, 20, 1300);
const initialWidth = corona.bounds[2];
const initialStrength = corona.coronaUniforms.uniforms.uStrength;
corona.update({ daoAdvanceTicks: 1 }, 20, 1700);
assert(corona.bounds[2] > initialWidth, "realm advancement expands the corona");
assert(corona.coronaUniforms.uniforms.uStrength < initialStrength, "realm advancement fades toward its end");
assert.equal(filtersCreated, 1, "animation reuses its filter");
corona.update({}, 20, 1800);
assert.equal(corona.visible, false, "expired bursts leave no ready glow");
corona.destroy();
assert(filter.destroyed && corona.destroyed, "pool teardown releases both filter and graphics");
const inactive = createDaoCorona(pixi);
inactive.destroy();
assert(inactive.destroyed, "uninitialized corona teardown is safe");
console.log("dao_corona_contracts: ok");
