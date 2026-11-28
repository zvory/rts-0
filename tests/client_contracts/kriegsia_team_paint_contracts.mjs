import assert from 'node:assert/strict';
import fs from 'node:fs';
import {PNG} from 'pngjs';
import {composeTeamPaintRgba} from '../../client/src/renderer/rigs/team_paint.js';
import {createLiveFrameStrips} from '../../client/src/renderer/rigs/frame_strip_routing.js';
import {createLivePngRigAtlases} from '../../client/src/renderer/rigs/png_routing.js';
import {liveUnitIconMarkupFor, mountLiveUnitIcon} from '../../client/src/renderer/rigs/unit_icon_sources.js';
import {teamPaintComponents} from '../../client/src/renderer/rigs/png_icon_source.js';
const specs=JSON.parse(fs.readFileSync(new URL('../../client/assets/rigs/kriegsia-team-color/specs.json',import.meta.url)));
const read=relative=>PNG.sync.read(fs.readFileSync(new URL('../../'+relative,import.meta.url)));
const strips=createLiveFrameStrips(),atlases=createLivePngRigAtlases();
for(const spec of specs){
 const png=read(`client/assets/rigs/kriegsia-team-color/generated/${spec.name}-atlas.png`);
 const original=read(spec.original),w=png.width,h=png.height/4,row=w*h*4;
 const fixed=png.data.subarray(row*2,row*3),paint=png.data.subarray(row*3,row*4);
 const red=composeTeamPaintRgba(fixed,paint,0xff0000),blue=composeTeamPaintRgba(fixed,paint,0x0000ff);
 assert.equal(png.width,spec.width*spec.count);assert.ok(w<=8192&&png.height<=8192,'GPU-compatible shared sources');
 for(let frame=0;frame<spec.count;frame++){
  let painted=0,stable=0;
  for(let y=0;y<h;y++)for(let x=frame*spec.width;x<(frame+1)*spec.width;x++){
   const i=(y*w+x)*4;assert.equal(paint[i],paint[i+1]);assert.equal(paint[i],paint[i+2]);
   if(paint[i+3]>100)painted++;
   if(!paint[i+3]&&fixed[i+3]>100){stable++;assert.deepEqual(red.subarray(i,i+4),blue.subarray(i,i+4),'fixed materials are identical across owners');}
   const alpha=y<original.height?original.data[(y*original.width+x)*4+3]:fixed[i+3];
   assert.ok(Math.abs(red[i+3]-alpha)<=1,`${spec.name} frame ${frame} silhouette/transparency preserved`);
  }
  assert.ok(painted>20&&stable>20,`${spec.name} frame ${frame}: complete paint and fixed materials`);
 }
 if(['rifleman','panzerfaust'].includes(spec.name)){
  for(let y=0;y<spec.height;y++)for(let x=149;x<spec.width;x++){
   const i=(y*w+4*spec.width+x)*4;
   assert.deepEqual(fixed.subarray(i,i+4),original.data.subarray(i,i+4),'original firing tip/muzzle-flash pixels preserved exactly');
   assert.equal(paint[i+3],0,'muzzle flash never receives team color');
  }
 }
 const kind=spec.name==='panzerfaust'?'panzerfaust':spec.name;
 const descriptor=spec.type==='atlas'?atlases.get(kind):strips.get(spec.name==='panzerfaust'?'rifleman.panzerfaustLoaded':kind);
 assert.ok(descriptor?.teamPaint,`${spec.name} production route uses shared paint`);
 assert.equal(descriptor.teamPaint.renderRows,1,'no empty effect row per cached palette');
 if(spec.type==='strip'){
  assert.equal(descriptor.frameCount,spec.count);
  for(let frame=0;frame<spec.count;frame++){
   const icon=liveUnitIconMarkupFor(kind,{teamColor:'#d55e00',minimapPart:`frame:${frame}`});
   assert.match(icon,/data-unit-icon-component="fixed"/);assert.match(icon,/data-unit-icon-component="team-paint"/);assert.match(icon,/feColorMatrix/);
  }
 }else{
  assert.ok(descriptor.sprites.every(s=>s.tintSlot==='fixed'),'ordinary sprites apply no whole-part team multiply');
  const icon=liveUnitIconMarkupFor(kind,{teamColor:'#d55e00'});
  assert.match(icon,/:fixed/);assert.match(icon,/:team-paint/);assert.match(icon,/feColorMatrix/);
  for(const sprite of descriptor.sprites){
   const icon=liveUnitIconMarkupFor(kind,{teamColor:'#d55e00',minimapPart:'sprite:'+sprite.id});
   assert.match(icon,/:fixed/);assert.match(icon,/:team-paint/);assert.match(icon,/feColorMatrix/);
  }
  // The animated HUD shares the same two layers under each sampled transform.
  const root={innerHTML:'',querySelectorAll:()=>[],replaceChildren(){this.innerHTML='';}};
  const mounted=mountLiveUnitIcon(root,kind,{teamColor:'#d55e00',reducedMotion:true});
  assert.match(root.innerHTML,/data-unit-icon-component="[^"]*:team-paint"/);assert.match(root.innerHTML,/feColorMatrix/);mounted.destroy();if(kind!=='rocket_launcher')assert.equal(root.innerHTML,'');
 }
}
const tank=atlases.get('tank');assert.equal(tank.attachments,undefined,'roof gun is packed into shared fixed source');
const frame=tank.sprites.find(s=>s.id==='sprite.commanderGun').frame;
assert.equal(frame.x,0);assert.equal(frame.y,1373,'roof-gun source location and transforms remain stable');
const gun=read('client/assets/rigs/scout-car-white-pass-01/generated/scout-car-white-atlas.png');
const fixed=read('client/assets/rigs/kriegsia-team-color/generated/tank-fixed.png');
for(let y=0;y<165;y++)for(let x=0;x<538;x++)assert.deepEqual(fixed.data.subarray(((1373+y)*fixed.width+x)*4,((1373+y)*fixed.width+x)*4+4),gun.data.subarray(((339+y)*gun.width+x+59)*4,((339+y)*gun.width+x+59)*4+4));
assert.equal(atlases.get('command_car').teamPaint,undefined,'existing Command Car is untouched');
for(const kind of ['warrior','wind_adept','war_adept','magic_adept'])assert.match(strips.get(kind).image,/cultivator-team-color/,'Cultivator routes unchanged');
const result=teamPaintComponents(tank,[{id:'test',frame:{x:5,y:8,w:10,h:12,originX:2,originY:3,pixelsPerUnit:2},x:7,rotation:.3}]);
assert.equal(result[0].frame.y,8+tank.grid.height*2);assert.equal(result[1].frame.y,8+tank.grid.height*3);assert.equal(result[0].rotation,result[1].rotation);assert.equal(result[0].frame.originX,2);
console.log('kriegsia_team_paint_contracts: ok');
