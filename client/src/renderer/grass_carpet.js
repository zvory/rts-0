import { createWorkerSafeCanvas } from "./raster_primitives.js";

const SIZE = 256;
const FRAMES = 8;
const FRAGMENT = `#version 300 es
precision highp float;
in vec2 vWorld;
out vec4 finalColor;
uniform float uTime;
uniform float uStyle;
uniform vec3 uTint;
uniform sampler2D uGrass;
uniform sampler2D uWind;
vec3 grass(vec2 uv, float frame){
 if(uStyle==8.) return texture(uGrass,vec2(fract(uv.x),(fract(uv.y)+frame)/8.)).rgb;
 return texture(uGrass,fract(uv)).rgb;
}
void main(){
 vec2 p=floor(vWorld/1.25)*1.25;
 // One repeatable texture encodes independent smooth wind and cloud fields.
 vec2 field=texture(uWind,p/650.+vec2(-uTime*.025,uTime*.008)).rg;
 float wave=sin(dot(p,vec2(.022,.013))-uTime*1.7+field.r*3.);
 vec2 uv=p/128.;
 vec3 color;
 if(uStyle==8.){
   float phase=uTime*1.5+field.r*2.;
   float index=floor(phase);
   color=mix(grass(uv,mod(index,8.)),grass(uv,mod(index+1.,8.)),fract(phase));
 }else{
   vec2 bend=vec2(wave,field.r-.5)*.018;
   color=grass(uv+bend,0.);
   if(uStyle==7.){
     // A second, rotated weave breaks the repeating pattern and adds a taller moving layer.
     vec3 upper=grass(uv.yx*1.13+vec2(.37,.21)+bend*1.8,0.);
     color=mix(color,upper,.42);
   }
 }
 float shade=smoothstep(.28,.73,field.g);
 color*=mix(.80,1.09,shade);
 color+=vec3(.03,.04,.012)*smoothstep(.2,.95,wave);
 const float opacity=.90;
 finalColor=vec4(color*uTint*opacity,opacity);
}`;

function randomSource(seed) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

function paintCarpet(ctx, frame, tall) {
  const random = randomSource(731);
  const offset = frame * SIZE;
  ctx.save();
  ctx.beginPath();
  ctx.rect(0, offset, SIZE, SIZE);
  ctx.clip();
  ctx.fillStyle = "#566333";
  ctx.fillRect(0, offset, SIZE, SIZE);
  // Overlapping strokes carpet every cell. Density is paid once, never per rendered fragment.
  const blades = [];
  for (let y = 0; y < SIZE; y += 3) for (let x = 0; x < SIZE; x += 3) {
    blades.push({ x: x + random()*4, y: y + random()*4,
      h: (tall ? 9 : 6) + random()*9, lean: random()*4-2,
      color: random(), phase: random()*Math.PI*2 });
  }
  blades.sort((a,b) => a.y-b.y);
  for (const blade of blades) {
    const wind = frame >= 0 ? Math.sin(frame/FRAMES*Math.PI*2 + blade.phase*.18)*3 : 0;
    const r = Math.round(73+blade.color*54);
    const g = Math.round(91+blade.color*51);
    const b = Math.round(39+blade.color*30);
    ctx.strokeStyle = `rgb(${r},${g},${b})`;
    ctx.lineWidth = 1.3 + blade.color*.8;
    // Wrap source strokes so the texture has no tile-edge gaps.
    for (const dx of [-SIZE,0,SIZE]) for (const dy of [-SIZE,0,SIZE]) {
      const x = blade.x+dx, y = blade.y+dy+offset;
      if (x < -8 || x > SIZE+8 || y < offset-2 || y-blade.h > offset+SIZE) continue;
      ctx.beginPath();
      ctx.moveTo(x,y);
      ctx.quadraticCurveTo(x+blade.lean*.3,y-blade.h*.5,x+blade.lean+wind,y-blade.h);
      ctx.stroke();
    }
  }
  ctx.restore();
}

function windCanvas() {
  const canvas = createWorkerSafeCanvas(128,128);
  const ctx = canvas.getContext("2d");
  const image = ctx.createImageData(128,128);
  for (let y=0;y<128;y++) for (let x=0;x<128;x++) {
    const a=x/128*Math.PI*2, b=y/128*Math.PI*2, i=(y*128+x)*4;
    image.data[i]=128+45*Math.sin(a+b)+28*Math.cos(a*2-b*3)+17*Math.sin(a*4+b*2);
    image.data[i+1]=128+52*Math.sin(a-b*2)+31*Math.cos(a*3+b)+18*Math.sin(a*2+b*4);
    image.data[i+2]=128;
    image.data[i+3]=255;
  }
  ctx.putImageData(image,0,0);
  return canvas;
}

/** Dense source art is baked once; the runtime shader uses only 2–3 texture samples. */
export function createGrassCarpetMaterial(pixi, technique) {
  const frames = technique === 8 ? FRAMES : 1;
  const canvas = createWorkerSafeCanvas(SIZE, SIZE*frames);
  const ctx = canvas.getContext("2d");
  for (let frame=0;frame<frames;frame++) paintCarpet(ctx,frame,technique===7);
  const grass = pixi.Texture.from(canvas);
  const wind = pixi.Texture.from(windCanvas());
  grass.source.scaleMode = "nearest";
  grass.source.addressMode = "repeat";
  wind.source.scaleMode = "linear";
  wind.source.addressMode = "repeat";
  return {
    fragment: FRAGMENT,
    resources: { uGrass: grass.source, uWind: wind.source },
    textureBytes: SIZE*SIZE*frames*4+128*128*4,
    destroy() { grass.destroy(true); wind.destroy(true); },
  };
}
