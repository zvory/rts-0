import { createWorkerSafeCanvas } from "./raster_primitives.js";
import { grassGroundPixels } from "./grass_ground.js";

const SIZE = 256;
const FRAMES = 8;
const CLUMPS_PER_AXIS = 24;
const WIND_SIZE = 128;

function fragment(terrainLighting) { return `#version 300 es
precision highp float;
in vec2 vWorld;
out vec4 finalColor;
uniform float uTime;
uniform vec3 uTint;
uniform sampler2D uGrassPoses;
uniform sampler2D uWind;
${terrainLighting ? "uniform sampler2D uTerrainLight; uniform vec2 uMapWorldSize;" : ""}
vec3 pose(vec2 uv,float index){
 return texture(uGrassPoses,vec2(fract(uv.x),(fract(uv.y)+mod(index,8.))/8.)).rgb;
}
void main(){
 vec2 p=floor(vWorld/1.25)*1.25;
 vec2 field=texture(uWind,p/650.+vec2(-uTime*.025,uTime*.008)).rg;
 // The atlas has fixed roots and eight bending poses. Blend current poses every frame.
 float phase=(dot(p,vec2(.022,.013))-uTime*1.7+field.r*3.)*(8./6.28318530718);
 float index=floor(phase);
 vec3 color=mix(pose(p/256.,index),pose(p/256.,index+1.),fract(phase));
 color*=mix(.80,1.09,smoothstep(.28,.73,field.g));
 // Broad traveling light, without the procedural hash/noise/exp front loop.
 float front=sin(p.x*.003927-uTime*.31416+(field.r-.5)*.8);
 color*=1.+smoothstep(.72,.99,front)*.25;
 ${terrainLighting ? "color*=texture(uTerrainLight,p/uMapWorldSize).rgb*(255./128.);" : ""}
 const float opacity=.90;
 finalColor=vec4(color*uTint*opacity,opacity);
}`; }

function randomSource(seed) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

function clumps() {
  const random = randomSource(731);
  const spacing = SIZE / CLUMPS_PER_AXIS;
  const result = [];
  for (let y = 0; y < CLUMPS_PER_AXIS; y++) for (let x = 0; x < CLUMPS_PER_AXIS; x++) {
    const r = random(), r2 = random();
    result.push({ x: (x + .2 + r * .6) * spacing,
      y: (y + .2 + r2 * .6) * spacing, r, height: 6 + r * 7 });
  }
  return result;
}

function wrappedClumps(roots) {
  const draws = [];
  for (let order = 0; order < roots.length; order++) {
    const root = roots[order];
    for (const dx of [-SIZE, 0, SIZE]) for (const dy of [-SIZE, 0, SIZE]) {
      const x = root.x + dx, y = root.y + dy;
      if (x < -8 || x > SIZE + 8 || y < -3 || y - root.height > SIZE) continue;
      draws.push({ ...root, x, y, order });
    }
  }
  // Wrapped roots below the tile can cast blades into its bottom edge. Their actual
  // destination height, not the unwrapped root's height, determines painter order.
  return draws.sort((a, b) => a.y - b.y || a.x - b.x || a.order - b.order);
}

function blade(ctx, x, y, height, bend, width) {
  ctx.beginPath();
  ctx.moveTo(x - width, y);
  ctx.lineTo(x + bend * .25 - width * .6, y - height * .5);
  ctx.lineTo(x + bend, y - height);
  ctx.lineTo(x + bend * .25 + width * .6, y - height * .5);
  ctx.lineTo(x + width, y);
  ctx.closePath();
  ctx.fill();
}

function paintPose(ctx, frame, roots) {
  const offset = frame * SIZE;
  ctx.save();
  ctx.beginPath();
  ctx.rect(0, offset, SIZE, SIZE);
  ctx.clip();
  for (const root of roots) {
    const phase = frame / FRAMES * Math.PI * 2;
    const bend = Math.sin(phase) * 3 + Math.sin(phase * 2 + root.r * Math.PI * 2) * .65;
    const tip = root.r * .6 + .15;
    const color = [87 + (145 - 87) * tip, 105 + (150 - 105) * tip, 48 + (77 - 48) * tip];
    const x = root.x, y = root.y + offset;
    ctx.fillStyle = "rgba(14,25,10,.16)";
    ctx.beginPath();
    ctx.moveTo(x - 3, y + 1);
    ctx.lineTo(x, y - .5);
    ctx.lineTo(x + 4, y - .5);
    ctx.lineTo(x + 7, y + 1);
    ctx.lineTo(x + 4, y + 2.5);
    ctx.lineTo(x, y + 2.5);
    ctx.closePath();
    ctx.fill();
    ctx.fillStyle = `rgba(${Math.round(color[0])},${Math.round(color[1])},${Math.round(color[2])},.90)`;
    blade(ctx, x, y, root.height, bend, .95);
    blade(ctx, x - 2, y, root.height * .72, bend - 2, .85);
    blade(ctx, x + 2, y, root.height * .85, bend + 2, .85);
  }
  ctx.restore();
}

function windCanvas() {
  const canvas = createWorkerSafeCanvas(WIND_SIZE, WIND_SIZE);
  const ctx = canvas.getContext("2d");
  const image = ctx.createImageData(WIND_SIZE, WIND_SIZE);
  for (let y = 0; y < WIND_SIZE; y++) for (let x = 0; x < WIND_SIZE; x++) {
    const a = x / WIND_SIZE * Math.PI * 2, b = y / WIND_SIZE * Math.PI * 2;
    const i = (y * WIND_SIZE + x) * 4;
    image.data[i] = 128 + 45 * Math.sin(a + b) + 28 * Math.cos(a * 2 - b * 3) + 17 * Math.sin(a * 4 + b * 2);
    image.data[i + 1] = 128 + 52 * Math.sin(a - b * 2) + 31 * Math.cos(a * 3 + b) + 18 * Math.sin(a * 2 + b * 4);
    image.data[i + 2] = 128;
    image.data[i + 3] = 255;
  }
  ctx.putImageData(image, 0, 0);
  return canvas;
}

/** Default grass: bake static art once and blend current-frame wind poses. */
export function createGrassBakedCarpetMaterial(pixi, terrainLighting = false) {
  const canvas = createWorkerSafeCanvas(SIZE, SIZE * FRAMES);
  const ctx = canvas.getContext("2d");
  const field = grassGroundPixels(9);
  const image = ctx.createImageData(SIZE, SIZE);
  image.data.set(field.pixels);
  const roots = wrappedClumps(clumps());
  for (let frame = 0; frame < FRAMES; frame++) {
    ctx.putImageData(image, 0, frame * SIZE);
    paintPose(ctx, frame, roots);
  }
  const grass = pixi.Texture.from(canvas);
  grass.source.scaleMode = "nearest";
  grass.source.addressMode = "repeat";
  const wind = pixi.Texture.from(windCanvas());
  wind.source.scaleMode = "linear";
  wind.source.addressMode = "repeat";
  return {
    fragment: fragment(terrainLighting),
    resources: { uGrassPoses: grass.source, uWind: wind.source },
    textureBytes: SIZE * SIZE * FRAMES * 4 + WIND_SIZE * WIND_SIZE * 4,
    destroy() { grass.destroy(true); wind.destroy(true); },
  };
}
