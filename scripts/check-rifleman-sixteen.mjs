import fs from 'node:fs';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {PNG} from 'pngjs';
import {composeTeamPaintRgba} from '../client/src/renderer/rigs/team_paint.js';
import {RIFLEMAN_EXPERIMENTAL_FRAME_STRIP as strip} from '../client/src/renderer/rigs/rifleman_experimental_png_strip.js';
import {riflemanPoseFrame} from '../client/src/renderer/rigs/rifleman_pose_animation.js';
import {frameStripMovementVisual} from '../client/src/renderer/frame_strip_movement.js';
import {_hpBar,_hpBarSlot,_ringRadius} from '../client/src/renderer/entities.js';
import {installFakePixi} from '../tests/client_contracts/pixi_fakes.mjs';
const root=new URL('../client/assets/rigs/rifleman-experimental-v3/',import.meta.url);
const read=n=>PNG.sync.read(fs.readFileSync(new URL(n+'.png',root)));
const f=read('fixed-atlas'),p=read('paint-atlas'),coverage=read('coverage-atlas');
const blue=composeTeamPaintRgba(f.data,p.data,0x0072b2),orange=composeTeamPaintRgba(f.data,p.data,0xd55e00);
for(let i=0;i<blue.length;i+=4){
 assert.ok(Math.abs(blue[i+3]-coverage.data[i+3])<=1);
 assert.equal(blue[i+3],orange[i+3]);
 if(!p.data[i+3])for(let c=0;c<4;c++)assert.equal(blue[i+c],orange[i+c]);
}
// Cardinal artwork is the approved baseline, preserved pixel-for-pixel.
const hashes=JSON.parse(fs.readFileSync(new URL('approved-cardinal-hashes.json',root)));
for(const layer of ['fixed-atlas','paint-atlas','coverage-atlas']){
 const current=read(layer);
 for(const [before,after]of [[0,0],[1,4],[2,8],[3,12]])for(let pose=0;pose<16;pose++){
  const b=before*16+pose,a=after*16+pose,cell=Buffer.alloc(128*128*4);
  for(let y=0;y<128;y++){
   const ai=((Math.floor(a/16)*128+y)*current.width+a%16*128)*4;
   cell.set(current.data.subarray(ai,ai+512),y*512);
  }
  assert.equal(crypto.createHash('sha256').update(cell).digest('hex'),hashes[layer][b],'approved cardinal pixels changed');
 }
}
const metadata=JSON.parse(fs.readFileSync(new URL('registration.json',root)));
const sizeReference=JSON.parse(fs.readFileSync(new URL('scale-reference.json',root)));
const count=strip.directional.directionCount,step=2*Math.PI/count;
assert.equal(count,16);assert.equal(strip.frameCount,256);
assert.ok(f.width<=2048&&f.height<=2048);
assert.equal(new Set(metadata.registrations.map(r=>r.heading)).size,16);
assert.ok(Math.abs(strip.poseGeometry.scaleFamilies[0].renderedHelmetWidth-sizeReference.southTargetHelmetWidth)<.01,
 'south character size diverged from approved reference head size');
const reachable=new Set();
for(let d=0;d<count;d++)for(let pose=0;pose<16;pose++){
 const frame=d*16+pose;reachable.add(frame);
 assert.ok(strip.poseGeometry.frameBounds[frame]);
}
assert.equal(reachable.size,256);
for(let d=0;d<count;d++)if(d%4!==0){
 const family=strip.poseGeometry.scaleFamilies[d];
 assert.ok(family.renderedHelmetWidth>=20.99&&family.renderedHelmetWidth<=24.01,'helmet scale drift '+d);
 const idle=strip.poseGeometry.frameBounds[d*16];
 assert.ok(idle.height>=81&&idle.height<=91,'standing scale drift '+d);
}
for(let d=0;d<count;d++){
 const facing=Math.PI/2+d*step;
 const e={x:100,y:100,facing,weaponFacing:facing};
 for(const offset of [-2*Math.PI,0,2*Math.PI]){
  const frame=riflemanPoseFrame({}, {...e,facing:facing+offset,weaponFacing:facing+offset},{now:0});
  assert.equal(frame,d*16,'heading wrap');
 }
 const gait=new Set();
 for(let pose=2;pose<=7;pose++){
  const frame=d*16+pose,cell=Buffer.alloc(128*128*4);
  for(let y=0;y<128;y++){
   const src=((Math.floor(frame/16)*128+y)*f.width+frame%16*128)*4;
   cell.set(blue.subarray(src,src+128*4),y*128*4);
  }
  gait.add(crypto.createHash('sha256').update(cell).digest('hex'));
 }
 assert.equal(gait.size,6,'duplicate gait artwork '+d);
}

for(const r of metadata.registrations){
 if(!r.running){assert.ok(Math.abs(r.footContact.x*r.scale+r.ox-64)<1e-6);assert.ok(Math.abs(r.footContact.y*r.scale+r.oy-112)<1e-6);}
 assert.equal(r.clipped,0);
 assert.equal(r.originalRgbFallback,0);
 const bounds=strip.poseGeometry.frameBounds[r.frame];
 assert.ok(bounds.x>0&&bounds.y>0&&bounds.x+bounds.width<128&&bounds.y+bounds.height<128,`clipping ${r.frame}`);
}
const restore=installFakePixi();
try {
 // Reproduce a live renderer running faster than its 30Hz position samples.
 // Every repeated position must retain the run pose and body bearing.
 for(const hz of [60,144,240])for(const facing of Array.from({length:count},(_,d)=>Math.PI/2+d*step)){
  const motion=new Map(),instance={};
  const expected=((Math.round((facing-Math.PI/2)/step)%count)+count)%count;
  const endTick=60;
  for(let i=0;i<hz*2;i++){
   const now=i*1000/hz,tick=Math.floor(now*30/1000);
   const position=t=>({x:100+Math.cos(facing)*t*1.6,y:100+Math.sin(facing)*t*1.6});
   const e={id:1,kind:'rifleman',state:'move',facing,weaponFacing:facing+Math.PI/2,...position(tick)};
   const state={tick,_curById:new Map([[1,position(tick)]]),_prevById:new Map([[1,position(Math.max(0,tick-1))]])};
   const movement=frameStripMovementVisual(e,state,motion,now);
   const frame=riflemanPoseFrame(instance,e,{now,frameStripMoving:movement.moving});
   if(now>100){assert.equal(Math.floor(frame/16),expected,`heading flicker at ${hz}Hz`);assert.ok(frame%16>=2&&frame%16<=7,`idle/run flicker at ${hz}Hz`);}
  }
  for(let i=0;i<hz/2;i++){
   const now=2000+i*1000/hz,pos={x:100+Math.cos(facing)*endTick*1.6,y:100+Math.sin(facing)*endTick*1.6};
   const e={id:1,kind:'rifleman',state:'idle',facing,...pos};
   const state={tick:endTick+i,_curById:new Map([[1,pos]]),_prevById:new Map([[1,pos]])};
   const movement=frameStripMovementVisual(e,state,motion,now);
   const frame=riflemanPoseFrame(instance,e,{now,frameStripMoving:movement.moving});
   if(now>2200)assert.equal(frame%16,0,`run animation persisted after stop at ${hz}Hz`);
  }
 }
 for(const [direction,facing]of Array.from({length:count},(_,d)=>[d,Math.PI/2+d*step])){
  const e={id:1,kind:'rifleman',x:100,y:200,facing,hp:45,maxHp:45,owner:1};
  const instance={};assert.equal(riflemanPoseFrame(instance,e,{now:0}),direction*16);
  const moved={...e,x:e.x+Math.cos(facing)*2,y:e.y+Math.sin(facing)*2};
  const run=riflemanPoseFrame(instance,moved,{now:90});assert.equal(Math.floor(run/16),direction);assert.ok(run%16>=2&&run%16<=7);
  for(const scale of [.5,1,2]){
   const bar=_hpBarSlot.call({_pools:{hpBars:new Map()},_seen:{hpBars:new Set()},layers:{hpBars:new PIXI.Container()}},1);
   const renderer={_experimentalRiflemanFrameStrip:()=>({strip}),_unitRenderContexts:new Map([[1,{visualScale:scale}]])};
   _hpBar.call(renderer,bar,e,null,0x0072b2);
   for(let pose=0;pose<16;pose++){
    const top=e.y+(strip.poseGeometry.frameBounds[direction*16+pose].y-112)*.45*scale;
    assert.ok(bar.y+5<=top-2,`HP overlaps sprite direction ${direction} pose ${pose} scale ${scale}`);
   }
   renderer._liveRigPools={liveUnitRigs:new Map([[1,{visualTopY:80}]])};_hpBar.call(renderer,bar,e,null,0x0072b2);assert.equal(bar.y,72);
   renderer._liveRigPools.liveShotRevealRigs=new Map([[1,{visualTopY:120}]]);
   for(const reveal of [{shotReveal:true},{aboveFogReveal:true}]){
    _hpBar.call(renderer,bar,{...e,...reveal},null,0x0072b2);
    assert.equal(bar.y,112,'firing-reveal HP must follow its current sprite, not a retained visible rig');
   }
   _hpBar.call(renderer,bar,e,null,0x0072b2);
   assert.equal(bar.y,72,'visible HP must keep following the visible sprite');
  }
  assert.equal(_ringRadius.call({_experimentalRiflemanFrameStrip:()=>({strip})},e).cy,0);
 }
} finally {restore();}
console.log('Sixteen-heading review checks passed: 256 poses, foot anchors, HP clearance, team masks, clipping, south size, and 60/144/240Hz movement/heading stability.');
