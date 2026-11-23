import assert from "node:assert/strict";
import { GrassPressureField, INFANTRY_PATH_PASSES } from "../../client/src/renderer/grass_matting.js";
import { KIND } from "../../client/src/protocol.js";
const map={width:128,height:128,tileSize:32};
const unit=(id,x,y,kind=KIND.RIFLEMAN,extra={})=>({id,x,y,kind,hp:100,...extra});
const sample=(f,x,y)=>f.pixels[(Math.floor(y/f.scale)*f.width+Math.floor(x/f.scale))*4];
function passage(field,id,step=4) {
 field.update([unit(id,42,66)]);
 for(let x=42+step;x<=202;x+=step) field.update([unit(id,x,66)]);
}
const field=new GrassPressureField(map);
assert.equal(field.pixels.byteLength,4*1024*1024);
assert.deepEqual([...field.pixels.slice(0,4)],[0,0,0,255]);
passage(field,1);
const first=sample(field,122,66);
assert.ok(first>=10 && first<=15,"one passage stores around 1/20 full wear");
const copy=field.pixels.slice();
assert.equal(field.update([unit(1,202,66)]),false,"standing still never adds wear");
assert.deepEqual(field.pixels,copy);
for(let id=2;id<=INFANTRY_PATH_PASSES;id++) passage(field,id);
assert.ok(sample(field,122,66)>=245,"twenty center-line passages form a clear path");
assert.ok(sample(field,122,82)<sample(field,122,66),"the path has a soft shoulder");
assert.equal(sample(field,122,94),0,"there is no rectangular footprint outside the soft radius");
for(let id=21;id<=50;id++) passage(field,id);
assert.equal(sample(field,122,66),255,"traffic saturates instead of wrapping");
assert.ok(field.pixels.every((v,i)=>i%4!==3 || v===255));

// The same world-space travel produces exactly the same wear at different render cadences.
const slow=new GrassPressureField(map), fast=new GrassPressureField(map);
passage(slow,1,1); passage(fast,1,8);
assert.deepEqual(slow.pixels,fast.pixels,"sampling is distance-based");
const fractional=new GrassPressureField(map);
passage(fractional,1,2.5);
assert.deepEqual(slow.pixels,fractional.pixels,"fractional sample carry preserves the same spatial deposits");
const gap=new GrassPressureField(map);
gap.update([unit(1,42,66)]); gap.update([unit(1,82,66)]);
gap.update([]); gap.update([unit(1,182,66)]);
assert.equal(sample(gap,132,66),0,"fog gaps never connect");
gap.update([unit(1,500,66)]);
assert.equal(sample(gap,300,66),0,"teleports never create wear");
for(const kind of [KIND.TANK,KIND.SCOUT_CAR,KIND.COMMAND_CAR,KIND.MORTAR_TEAM,
 KIND.ANTI_TANK_GUN,KIND.ARTILLERY,KIND.ROCKET_LAUNCHER,KIND.SCOUT_PLANE]) {
 const f=new GrassPressureField(map);
 f.update([unit(2,50,100,kind)]);
 assert.equal(f.update([unit(2,90,100,kind)]),false,`${kind} does not contribute infantry wear`);
 assert.equal(f.poses.size,0);
}
for(const extra of [{shotReveal:true},{aboveFogReveal:true},{visionOnly:true}]) {
 const f=new GrassPressureField(map);
 f.update([unit(1,50,50,KIND.RIFLEMAN,extra)]);
 assert.equal(f.update([unit(1,90,50,KIND.RIFLEMAN,extra)]),false);
 assert.equal(f.poses.size,0);
}
const huge=new GrassPressureField({width:1024,height:1024,tileSize:32});
assert.equal(huge.pixels.byteLength,4*1024*1024,"large maps keep the allocation cap");

const { installFakePixi } = await import("./pixi_fakes.mjs");
const { getVisualProfile } = await import("../../client/src/visual_profiles.js");
const { reconcileGrassLayer, destroyGrassLayer } = await import("../../client/src/renderer/grass.js");
const restore=installFakePixi();
try {
 assert.equal(getVisualProfile("grass-survey-12").grassTechnique,12);
 const renderer={_map:{width:8,height:8,tileSize:32,terrain:Array(64).fill(0)},
   layers:{terrain:new PIXI.Container()},world:{tint:0xffffff}};
 reconcileGrassLayer(renderer,0);
 assert.equal(renderer._grassLayer.matting,undefined);
 reconcileGrassLayer(renderer,12);
 const pressure=renderer._grassLayer.matting;
 assert.deepEqual(pressure.image.data,pressure.field.pixels);
 assert.match(renderer._grassLayer.shader.options.gl.fragment,/vec3 soil=/);
 assert.ok(renderer._grassLayer.shader.options.gl.fragment.includes("*(1.-rootWear*.95)"),
   "worn soil fades old tuft shadows instead of retaining footstep-like dots");
 pressure.update([unit(1,42,66)]);
 pressure.update([unit(1,82,66)]);
 assert.ok(pressure.field.pixels.some((v,i)=>i%4===0 && v>0));
 destroyGrassLayer(renderer);
 assert.equal(pressure.field.poses.size,0);
 assert.equal(renderer.layers.terrain.children.length,0);
} finally { restore(); }
console.log("grass matting contracts passed");
