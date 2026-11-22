import { TERRAIN } from "../protocol.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";
import { waterSunTint } from "./water.js";
import { SHORELINE_TILES_FRAGMENT } from "./shoreline_tiles.js";
import { SHORELINE_SPLAT_FRAGMENT } from "./shoreline_splat.js";
import { SHORELINE_SWASH_FRAGMENT } from "./shoreline_swash.js";

export const DEFAULT_SHORELINE_TECHNIQUE = 3;
const SAMPLES = 8;
const RANGE = 128;
const FRAGMENTS = [null, SHORELINE_TILES_FRAGMENT, SHORELINE_SPLAT_FRAGMENT, SHORELINE_SWASH_FRAGMENT];
const VERTEX = `#version 300 es
precision highp float;
in vec2 aPosition;
uniform mat3 uProjectionMatrix, uWorldTransformMatrix, uTransformMatrix;
out vec2 vWorld;
void main() {
 vWorld=aPosition;
 gl_Position=vec4((uProjectionMatrix*uWorldTransformMatrix*uTransformMatrix*vec3(aPosition,1.)).xy,0.,1.);
}`;
const HEADER = `#version 300 es
precision highp float;
in vec2 vWorld;
out vec4 finalColor;
uniform float uTime, uTileSize;
uniform vec2 uMapSize;
uniform vec3 uTint;
uniform sampler2D uShore;
float shoreHash(vec2 p) { return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
float shoreNoise(vec2 p) {
 vec2 i=floor(p), f=fract(p); f=f*f*(3.-2.*f);
 return mix(mix(shoreHash(i),shoreHash(i+vec2(1,0)),f.x),
 mix(shoreHash(i+vec2(0,1)),shoreHash(i+vec2(1)),f.x),f.y);
}
float shoreDistance(vec2 p) { return texture(uShore,p/uMapSize).r*256.-128.; }
float shoreWater(vec2 p) {
 if(any(lessThan(p,vec2(0))) || any(greaterThanEqual(p,uMapSize))) return 0.;
 return step(.5,texture(uShore,(floor(p/uTileSize)+.5)*uTileSize/uMapSize).g);
}
`;

// Two chamfer sweeps per material: fixed map-load work, never per-frame geometry.
function distanceTo(mask, width, height, target) {
 const d=Float32Array.from(mask, value => value===target ? 0 : 1e6);
 for(let y=0;y<height;y++) for(let x=0;x<width;x++) {
  const i=y*width+x;
  if(x) d[i]=Math.min(d[i],d[i-1]+1);
  if(y) d[i]=Math.min(d[i],d[i-width]+1);
  if(x&&y) d[i]=Math.min(d[i],d[i-width-1]+Math.SQRT2);
  if(x+1<width&&y) d[i]=Math.min(d[i],d[i-width+1]+Math.SQRT2);
 }
 for(let y=height-1;y>=0;y--) for(let x=width-1;x>=0;x--) {
  const i=y*width+x;
  if(x+1<width) d[i]=Math.min(d[i],d[i+1]+1);
  if(y+1<height) d[i]=Math.min(d[i],d[i+width]+1);
  if(x&&y+1<height) d[i]=Math.min(d[i],d[i+width-1]+Math.SQRT2);
  if(x+1<width&&y+1<height) d[i]=Math.min(d[i],d[i+width+1]+Math.SQRT2);
 }
 return d;
}

export function shorelineField(map) {
 const width=map.width*SAMPLES, height=map.height*SAMPLES;
 const mask=new Uint8Array(width*height);
 for(let y=0;y<height;y++) for(let x=0;x<width;x++)
  mask[y*width+x]=Number(map.terrain[Math.floor(y/SAMPLES)*map.width+Math.floor(x/SAMPLES)]===TERRAIN.WATER);
 const toLand=distanceTo(mask,width,height,0), toWater=distanceTo(mask,width,height,1);
 const pixels=new Uint8ClampedArray(width*height*4);
 for(let i=0;i<mask.length;i++) {
  const signed=(mask[i] ? toLand[i]-.5 : .5-toWater[i])*map.tileSize/SAMPLES;
  pixels[i*4]=Math.round((Math.max(-RANGE,Math.min(RANGE,signed))+RANGE)/256*255);
  pixels[i*4+1]=mask[i]*255;
  pixels[i*4+3]=255;
 }
 return {width,height,pixels};
}

export class ShorelineLayer {
 constructor(pixi,layer,map,tint,technique) {
  // Uniform maps have no bank: skip the field, GPU texture, and draw call entirely.
  const water=map.terrain.some(code=>code===TERRAIN.WATER);
  const land=map.terrain.some(code=>code!==TERRAIN.WATER);
  if(!water||(!land&&technique!==1)) return;
  const field=shorelineField(map);
  const canvas=createWorkerSafeCanvas(field.width,field.height);
  const ctx=canvas.getContext("2d"), image=ctx.createImageData(field.width,field.height);
  image.data.set(field.pixels); ctx.putImageData(image,0,0);
  this.texture=pixi.Texture.from(canvas); this.texture.source.scaleMode="linear";
  const w=map.width*map.tileSize,h=map.height*map.tileSize;
  this.uniforms=new pixi.UniformGroup({
   uTime:{value:0,type:"f32"}, uTileSize:{value:map.tileSize,type:"f32"},
   uMapSize:{value:new Float32Array([w,h]),type:"vec2<f32>"},
   uTint:{value:waterSunTint(tint),type:"vec3<f32>"},
  });
  this.geometry=new pixi.Geometry({attributes:{aPosition:{buffer:new Float32Array([0,0,w,0,w,h,0,0,w,h,0,h]),format:"float32x2"}}});
  this.shader=pixi.Shader.from({gl:{vertex:VERTEX,fragment:HEADER+FRAGMENTS[technique],name:`shoreline-study-${technique}`},
   resources:{shorelineUniforms:this.uniforms,uShore:this.texture.source}});
  this.mesh=new pixi.Mesh({geometry:this.geometry,shader:this.shader}); layer.addChild(this.mesh);
 }
 update(nowMs) {
  if(!this.mesh) return;
  this.uniforms.uniforms.uTime=nowMs/1000;
  // Async water asset completion must never cover the shoreline, including in the editor.
  const layer=this.mesh.parent;
  if(layer&&layer.children[layer.children.length-1]!==this.mesh) layer.setChildIndex(this.mesh,layer.children.length-1);
 }
 destroy() {
  this.mesh?.parent?.removeChild(this.mesh); this.mesh?.destroy();
  this.geometry?.destroy(true); this.shader?.destroy(); this.texture?.destroy(true);
 }
}

export function destroyShorelineLayer(renderer) {
 renderer._shorelineLayer?.destroy(); renderer._shorelineLayer=null;
 renderer._shorelineMap=null; renderer._shorelineTechnique=0;
}

// Animated swash is the shared match/editor default; Lab profiles may compare alternatives.
export function reconcileShorelineLayer(renderer,requested=0) {
 const technique=Number.isInteger(requested)&&requested>=1&&requested<=3 ? requested : DEFAULT_SHORELINE_TECHNIQUE;
 if(!renderer._map) { if(renderer._shorelineLayer) destroyShorelineLayer(renderer); return; }
 if(renderer._shorelineTechnique!==technique||renderer._shorelineMap!==renderer._map) {
  destroyShorelineLayer(renderer);
  renderer._shorelineLayer=new ShorelineLayer(PIXI,renderer.layers.terrain,renderer._map,renderer.world.tint,technique);
  renderer._shorelineMap=renderer._map; renderer._shorelineTechnique=technique;
 }
}
