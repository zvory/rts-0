import assert from "node:assert/strict";
import { shorelineField, reconcileShorelineLayer, destroyShorelineLayer } from "../../client/src/renderer/shoreline.js";
import { getVisualProfile, resolveVisualProfileLaunch } from "../../client/src/visual_profiles.js";
import { installFakePixi } from "./pixi_fakes.mjs";

assert.equal(resolveVisualProfileLaunch(null).profile,null);
for(let i=1;i<=3;i++) assert.equal(getVisualProfile(`shoreline-study-${i}`).shorelineTechnique,i);
const map={width:3,height:1,tileSize:32,terrain:[0,2,0]};
const field=shorelineField(map);
const value=(x,channel=0)=>field.pixels[(4*field.width+x)*4+channel];
assert(value(7)<128 && value(8)>128,"distance changes sign at the bank");
assert(value(8)<value(11),"water gets deeper toward the center");
assert.equal(value(4,1),0);
assert.equal(value(12,1),255);
for(const code of [0,2]) {
 const uniform=shorelineField({...map,terrain:[code,code,code]});
 assert.equal(uniform.pixels[0],code===2?255:0,"uniform maps saturate without artificial banks");
}
const restore=installFakePixi();
try {
 const layer=new PIXI.Container();
 const renderer={_map:map,layers:{terrain:layer},world:{tint:0xfff3e2}};
 reconcileShorelineLayer(renderer);
 assert.equal(layer.children.length,0,"ordinary matches allocate no shoreline study resources");
 reconcileShorelineLayer(renderer,1);
 const first=renderer._shorelineLayer;
 reconcileShorelineLayer(renderer,1);
 assert.equal(renderer._shorelineLayer,first,"frames reuse the cached field and geometry");
 first.update(2500);
 assert.equal(first.uniforms.uniforms.uTime,2.5,"animation follows the presentation clock");
 reconcileShorelineLayer(renderer,2);
 assert(first.mesh.destroyed,"switching profiles releases the previous mesh");
 assert.equal(layer.children.length,1);
 const second=renderer._shorelineLayer;
 renderer._map={...map};
 reconcileShorelineLayer(renderer,2);
 assert(second.mesh.destroyed,"map replacement invalidates the cached distance field");
 reconcileShorelineLayer(renderer,0);
 assert.equal(layer.children.length,0,"removing the profile removes its overlay");
 destroyShorelineLayer(renderer);
} finally { restore(); }
console.log("shoreline study contracts passed");
