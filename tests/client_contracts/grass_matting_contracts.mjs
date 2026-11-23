import assert from "node:assert/strict";
import { GrassPressureField } from "../../client/src/renderer/grass_matting.js";
import { KIND } from "../../client/src/protocol.js";
const map={width:128,height:128,tileSize:32};
const unit=(id,x,y,kind=KIND.RIFLEMAN,extra={})=>({id,x,y,kind,hp:100,...extra});
const field=new GrassPressureField(map);
assert.equal(field.pixels.byteLength,4*1024*1024);
assert.equal(field.update([unit(1,50,50)]),false);
assert.equal(field.update([unit(1,90,50)]),true);
const sample=(f,x,y)=>f.pixels[(Math.floor(y/f.scale)*f.width+Math.floor(x/f.scale))*4];
assert.ok(sample(field,70,50)>0,"infantry mats the center line");
const copy=field.pixels.slice();
assert.equal(field.update([unit(1,90,50)]),false,"stationary units cause no upload");
assert.deepEqual(field.pixels,copy);
field.update([]);
field.update([unit(1,150,50)]);
assert.equal(sample(field,120,50),0,"fog gaps never connect");
field.update([unit(1,500,50)]);
assert.equal(sample(field,300,50),0,"teleports never draw long stripes");
const tank=new GrassPressureField(map);
tank.update([unit(2,50,100,KIND.TANK)]);
tank.update([unit(2,90,100,KIND.TANK)]);
assert.ok(sample(tank,70,82)>0 && sample(tank,70,118)>0);
assert.equal(sample(tank,70,100),0,"paired vehicle lines leave the center upright");
for(const extra of [{shotReveal:true},{aboveFogReveal:true},{visionOnly:true}]) {
 const f=new GrassPressureField(map);
 f.update([unit(1,50,50,KIND.RIFLEMAN,extra)]);
 assert.equal(f.update([unit(1,90,50,KIND.RIFLEMAN,extra)]),false);
 assert.equal(f.poses.size,0);
}
const air=new GrassPressureField(map);
air.update([unit(1,50,50,KIND.SCOUT_PLANE)]);
assert.equal(air.update([unit(1,90,50,KIND.SCOUT_PLANE)]),false);
const huge=new GrassPressureField({width:1024,height:1024,tileSize:32});
assert.equal(huge.pixels.byteLength,4*1024*1024,"large maps keep the allocation cap");
console.log("grass matting contracts passed");

// The opt-in shader owns/releases its mask; ordinary grass allocates no mask.
const { installFakePixi } = await import("./pixi_fakes.mjs");
const { getVisualProfile } = await import("../../client/src/visual_profiles.js");
const { reconcileGrassLayer, destroyGrassLayer } = await import("../../client/src/renderer/grass.js");
const restore=installFakePixi();
try {
 assert.equal(getVisualProfile("grass-survey-12").grassTechnique,12);
 const renderer={_map:{width:2,height:2,tileSize:32,terrain:[0,0,0,0]},
   layers:{terrain:new PIXI.Container()},world:{tint:0xffffff}};
 reconcileGrassLayer(renderer,0);
 assert.equal(renderer._grassLayer.matting,undefined);
 reconcileGrassLayer(renderer,12);
 const pressure=renderer._grassLayer.matting;
 pressure.update([unit(1,10,20)]);
 pressure.update([unit(1,30,20)]);
 assert.ok(pressure.field.pixels.some(v=>v>0));
 destroyGrassLayer(renderer);
 assert.equal(pressure.field.poses.size,0);
 assert.equal(renderer.layers.terrain.children.length,0);
} finally { restore(); }
