import { createGrassGroundMaterial } from "./grass_ground.js";
import { createGrassCarpetMaterial } from "./grass_carpet.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";
import { GrassMattingMask } from "./grass_matting.js";
import { TERRAIN } from "../protocol.js";

const VERTEX = `#version 300 es
precision highp float;
in vec2 aPosition;
uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
out vec2 vWorld;
void main(){
 vWorld=aPosition;
 gl_Position=vec4((uProjectionMatrix*uWorldTransformMatrix*uTransformMatrix*vec3(aPosition,1.)).xy,0.,1.);
}`;
export const DEFAULT_GRASS_TECHNIQUE = 9;

function grassFragment(groundTexture = false, terrainLighting = false, matting = false) { return `#version 300 es
precision highp float;
in vec2 vWorld;
out vec4 finalColor;
uniform float uTime;
uniform float uStyle;
uniform vec3 uTint;
${groundTexture ? "uniform sampler2D uGround;" : ""}
${matting ? "uniform sampler2D uPressure; uniform vec2 uPressureWorldSize;" : ""}
${terrainLighting ? "uniform sampler2D uTerrainLight; uniform vec2 uMapWorldSize;" : ""}
float hash(vec2 p){ return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
float noise(vec2 p){
 vec2 i=floor(p), f=fract(p); f=f*f*(3.-2.*f);
 return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+vec2(1)),f.x),f.y);
}
// Wide, curved fronts travel across the field about twenty seconds apart.
float windLight(vec2 p){
 float x=p.x-uTime*80.;
 float cell=floor(x/1600.);
 float light=0.;
 for(int i=-1;i<=1;i++){
   float id=cell+float(i);
   float center=(id+.5)*1600.+(hash(vec2(id,7.))-.5)*350.;
   float curve=(noise(vec2(p.y/550.,id+91.))-.5)*350.;
   float width=190.+hash(vec2(id,11.))*120.;
   float d=(x-center+curve)/width;
   light+=exp(-d*d*1.5)*(.24+hash(vec2(id,17.))*.12);
 }
 return light;
}
float blade(vec2 p, vec2 root, float h, float bend, float width){
 float t=clamp((root.y-p.y)/h,0.,1.);
 float center=root.x+bend*t*t;
 return (1.-smoothstep(width*(1.-t)+.1,width*(1.-t)+.65,abs(p.x-center)))
   *step(root.y-h,p.y)*step(p.y,root.y);
}
float pressedBlade(vec2 p, vec2 root, float h, float bend, float width, vec3 pressure){
 vec2 direction=pressure.gb*2.-1.;
 direction=length(direction)>.01?normalize(direction):vec2(1.,0.);
 vec2 tip=mix(vec2(bend,-h),direction*h*.95+vec2(0.,-h*.12),pressure.r);
 float t=clamp(dot(p-root,tip)/max(dot(tip,tip),.01),0.,1.);
 return 1.-smoothstep(width*(1.-t)+.1,width*(1.-t)+.65,length(p-root-tip*t));
}
void main(){
 // Quantization deliberately follows the chunky water artwork rather than subpixel sparkles.
 vec2 p=floor(vWorld/1.25)*1.25;
 float variation=noise(p/85.)*.65+noise(p/23.)*.35;
 float grain=hash(floor(p/2.));
 float gust=sin(dot(p,vec2(.022,.013))-uTime*1.7+noise(p/180.)*3.);
 vec3 dark=vec3(.225,.276,.135), light=vec3(.43,.47,.25);
 if(uStyle==4.){ dark=vec3(.30,.29,.16); light=vec3(.55,.51,.29); }
 vec3 color=${groundTexture ? "texture(uGround,fract(p/512.)).rgb" : "mix(dark,light,variation*.8+grain*.2)"};
 if(uStyle==1.){
   float fleck=step(.70,grain)*step(.38,hash(floor(p/vec2(2.,5.))));
   color+=vec3(.065,.08,.025)*fleck;
   color+=vec3(.045,.055,.018)*gust;
 }else{
   float spacing=uStyle==2.?14.:(uStyle==3.?12.:(uStyle==4.?15.:11.));
   vec2 cell=floor(p/spacing);
   for(int y=-1;y<=1;y++)for(int x=-1;x<=1;x++){
     vec2 id=cell+vec2(float(x),float(y));
     float r=hash(id), r2=hash(id+19.);
     vec2 root=(id+vec2(.2+r*.6,.2+r2*.6))*spacing;
     float island=noise(root/65.);
     if(uStyle==2. && r<.48)continue;
     if(uStyle==3. && island<.40)continue;
     if(uStyle==4. && r<.27)continue;
     float wave=sin(dot(root,vec2(.022,.013))-uTime*1.7+noise(root/180.)*3.);
     float bend=wave*(uStyle==4.?4.5:3.)+sin(uTime*2.1+r*6.28)*.65;
     float h=uStyle==2.?5.+r*6.:(uStyle==3.?7.+r*7.:(uStyle==4.?9.+r*9.:6.+r*7.));
     // Low dark roots, then multiple tapered blades with different tips.
     float shadow=(1.-smoothstep(1.,5.,length((p-root-vec2(2,1))*vec2(.7,1.5))))*.25;
     color*=1.-shadow;
     ${matting ? `vec3 pressure=texture(uPressure,root/uPressureWorldSize).rgb;
     bend*=1.-pressure.r;
     float mask=pressedBlade(p,root,h,bend,.8,pressure);
     mask=max(mask,pressedBlade(p,root+vec2(-2,0),h*.72,bend-2.,.7,pressure));
     mask=max(mask,pressedBlade(p,root+vec2(2,0),h*.85,bend+2.,.7,pressure));` : `float mask=blade(p,root,h,bend,.8);
     mask=max(mask,blade(p,root+vec2(-2,0),h*.72,bend-2.,.7));
     mask=max(mask,blade(p,root+vec2(2,0),h*.85,bend+2.,.7));`}
     vec3 tip=mix(vec3(.34,.41,.19),vec3(.57,.59,.30),r*.6+(uStyle==5. || uStyle>=9. ? 0. : wave*.15)+.15);
     if(uStyle==4.)tip=mix(vec3(.46,.44,.22),vec3(.68,.61,.36),r);
     color=mix(color,tip,mask*.9);
     if(uStyle==4. && r>.58){
       vec2 head=root+vec2(bend,-h);
       float seed=(1.-smoothstep(1.,2.1,length((p-head)*vec2(1.,.48))));
       color=mix(color,vec3(.70,.63,.39),seed*.85);
     }
   }
   if(uStyle==5. || uStyle>=9.){
     float cloud=smoothstep(.36,.68,noise(p/250.+vec2(-uTime*.028,uTime*.009)));
     color*=mix(.80,1.09,cloud);
     color*=1.+windLight(p);
   }
 }
 ${terrainLighting ? "color*=texture(uTerrainLight,p/uMapWorldSize).rgb*(255./128.);" : ""}
 // Pixi's normal blend mode expects premultiplied color, including for custom meshes.
 const float opacity=.90;
 finalColor=vec4(color*uTint*opacity,opacity);
}`; }

/** Animated grass terrain; Lab profiles may override the selected material. */
export class GrassLayer {
  constructor(pixi, layer, map, technique, tint, lighting = null) {
    const positions = [];
    const ts = map.tileSize;
    for (let y = 0; y < map.height; y++) for (let x = 0; x < map.width; x++) {
      if (map.terrain[y * map.width + x] !== TERRAIN.GRASS) continue;
      const l = x * ts, t = y * ts, r = l + ts, b = t + ts;
      positions.push(l,t,r,t,r,b,l,t,r,b,l,b);
    }
    if (!positions.length) return;
    const uniforms = {
      uTime: { value: 0, type: "f32" },
      uStyle: { value: technique, type: "f32" },
      uTint: { value: new Float32Array([((tint >> 16) & 255)/255, ((tint >> 8) & 255)/243, (tint & 255)/226]), type: "vec3<f32>" },
    };
    this.geometry = new pixi.Geometry({ attributes: { aPosition: { buffer: new Float32Array(positions), format: "float32x2" } } });
    this.material = technique >= 9
      ? createGrassGroundMaterial(pixi, technique === 12 ? 9 : technique)
      : technique >= 6 ? createGrassCarpetMaterial(pixi, technique) : null;
    if (lighting && !this.material?.fragment) {
      const canvas = createWorkerSafeCanvas(lighting.width, lighting.height);
      const ctx = canvas.getContext("2d");
      const image = ctx.createImageData(lighting.width, lighting.height);
      image.data.set(lighting.pixels);
      ctx.putImageData(image, 0, 0);
      this.lightingTexture = pixi.Texture.from(canvas);
      this.lightingTexture.source.scaleMode = "linear";
      uniforms.uMapWorldSize = { value: new Float32Array([map.width * ts, map.height * ts]), type: "vec2<f32>" };
    }
    if (technique === 12) {
      this.matting = new GrassMattingMask(pixi, map);
      uniforms.uPressureWorldSize = { value: new Float32Array([
        this.matting.field.width * this.matting.field.scale,
        this.matting.field.height * this.matting.field.scale]), type: "vec2<f32>" };
    }
    this.uniforms = new pixi.UniformGroup(uniforms);
    this.shader = pixi.Shader.from({
      gl: { vertex: VERTEX, fragment: this.material?.fragment || grassFragment(technique >= 9, !!this.lightingTexture, technique === 12), name: "grass-terrain" },
      resources: { grassUniforms: this.uniforms, ...(this.matting ? { uPressure: this.matting.texture.source } : {}), ...this.material?.resources, ...(this.lightingTexture ? { uTerrainLight: this.lightingTexture.source } : {}) },
    });
    this.mesh = new pixi.Mesh({ geometry: this.geometry, shader: this.shader });
    layer.addChild(this.mesh);
  }
  update(nowMs) { if (this.uniforms) this.uniforms.uniforms.uTime = nowMs / 1000; }
  destroy() {
    this.mesh?.parent?.removeChild(this.mesh);
    this.mesh?.destroy();
    this.geometry?.destroy(true);
    this.shader?.destroy();
    this.material?.destroy();
    this.matting?.destroy();
    this.lightingTexture?.destroy(true);
  }
}

export function destroyGrassLayer(renderer, { releaseLighting = false } = {}) {
  renderer._grassLayer?.destroy();
  renderer._grassLayer = null;
  renderer._grassTechnique = 0;
  if (releaseLighting) renderer._grassLightingField = null;
}

export function reconcileGrassLayer(renderer, requestedTechnique = 0) {
  const technique = Number.isInteger(requestedTechnique) && requestedTechnique >= 1 && requestedTechnique <= 12
    ? requestedTechnique : DEFAULT_GRASS_TECHNIQUE;
  if (!renderer._map) { destroyGrassLayer(renderer); return; }
  if (renderer._grassTechnique !== technique || !renderer._grassLayer) {
    destroyGrassLayer(renderer);
    renderer._grassTechnique = technique;
    renderer._grassLayer = new GrassLayer(PIXI, renderer.layers.terrain, renderer._map, technique,
      renderer.world.tint, renderer._grassLightingField);
  }
}

export function updateTerrainMaterials(renderer, nowMs) {
  renderer._shorelineLayer?.update(nowMs);
  renderer._waterLayer?.update(nowMs);
  renderer._grassLayer?.update(nowMs);
}
