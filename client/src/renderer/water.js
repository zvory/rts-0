import { TERRAIN } from "../protocol.js";
import { loadWorkerSafeTexture, createWorkerSafeCanvas } from "./raster_primitives.js";

const SHORE_RANGE = 128;
const SAMPLES_PER_TILE = 8;

// These colors were approved under Classic's warmth=25 lighting. Calibrate the
// material there, while allowing other authored sun settings to warm/cool it.
export function waterSunTint(tint) {
  return new Float32Array([
    ((tint >> 16) & 255) / 255, ((tint >> 8) & 255) / 243, (tint & 255) / 226,
  ]);
}
const VERTEX = `#version 300 es
precision highp float;
in vec2 aPosition;
uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
out vec2 vWorld;
void main() {
  vWorld = aPosition;
  mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
  gl_Position = vec4((mvp * vec3(aPosition, 1.0)).xy, 0.0, 1.0);
}`;
/** A fixed, coarse signed-distance-style field; only water-side distances are sampled. */
export function shoreDistances(map) {
  const width = map.width * SAMPLES_PER_TILE;
  const height = map.height * SAMPLES_PER_TILE;
  const distance = new Float32Array(width * height);
  const diagonal = Math.SQRT2;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const code = map.terrain[Math.floor(y / SAMPLES_PER_TILE) * map.width + Math.floor(x / SAMPLES_PER_TILE)];
      distance[y * width + x] = code === TERRAIN.WATER ? 1e5 : 0;
    }
  }
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = y * width + x;
      if (x > 0) distance[i] = Math.min(distance[i], distance[i - 1] + 1);
      if (y > 0) distance[i] = Math.min(distance[i], distance[i - width] + 1);
      if (x > 0 && y > 0) distance[i] = Math.min(distance[i], distance[i - width - 1] + diagonal);
      if (x + 1 < width && y > 0) distance[i] = Math.min(distance[i], distance[i - width + 1] + diagonal);
    }
  }
  for (let y = height - 1; y >= 0; y--) {
    for (let x = width - 1; x >= 0; x--) {
      const i = y * width + x;
      if (x + 1 < width) distance[i] = Math.min(distance[i], distance[i + 1] + 1);
      if (y + 1 < height) distance[i] = Math.min(distance[i], distance[i + width] + 1);
      if (x > 0 && y + 1 < height) distance[i] = Math.min(distance[i], distance[i + width - 1] + diagonal);
      if (x + 1 < width && y + 1 < height) distance[i] = Math.min(distance[i], distance[i + width + 1] + diagonal);
    }
  }
  const pixels = new Uint8ClampedArray(width * height * 4);
  for (let i = 0; i < distance.length; i++) {
    const value = Math.round(Math.min(SHORE_RANGE, Math.max(0, distance[i] - 0.5) * map.tileSize / SAMPLES_PER_TILE) / SHORE_RANGE * 255);
    pixels[i * 4] = pixels[i * 4 + 1] = pixels[i * 4 + 2] = value;
    pixels[i * 4 + 3] = 255;
  }
  return { width, height, pixels };
}

/** Direct MIT-licensed jess::codes pixel-water port with shore-derived depth inputs. */
export class WaterLayer {
  constructor(pixi, layer, map, tint = 0xffffff) {
    this.destroyed = false;
    this.textures = [];
    this.ready = Promise.resolve(true);
    const positions = [];
    const ts = map.tileSize;
    for (let y = 0; y < map.height; y++) {
      for (let x = 0; x < map.width; x++) {
        if (map.terrain[y * map.width + x] !== TERRAIN.WATER) continue;
        const left = x * ts, top = y * ts, right = left + ts, bottom = top + ts;
        positions.push(left, top, right, top, right, bottom, left, top, right, bottom, left, bottom);
      }
    }
    if (!positions.length) return;
    const field = shoreDistances(map);
    const canvas = createWorkerSafeCanvas(field.width, field.height);
    const ctx = canvas.getContext("2d");
    const image = ctx.createImageData(field.width, field.height);
    image.data.set(field.pixels);
    ctx.putImageData(image, 0, 0);
    this.texture = pixi.Texture.from(canvas);
    this.texture.source.scaleMode = "nearest";
    this.uniforms = new pixi.UniformGroup({
      uTime: { value: 0, type: "f32" },
      uMapSize: { value: new Float32Array([map.width * ts, map.height * ts]), type: "vec2<f32>" },
      // Custom mesh shaders do not inherit the sprite pipeline's world tint.
      uTint: { value: waterSunTint(tint), type: "vec3<f32>" },
    });
    this.geometry = new pixi.Geometry({
      attributes: { aPosition: { buffer: new Float32Array(positions), format: "float32x2" } },
    });
    const base = new URL("../../assets/water/pixel-water/", import.meta.url);
    const files = {
      CausticTexture: "caustics.png", CausticHighlightTexture: "highlights.png",
      MovementNoise: "movement.png", RandomFadeNoise: "fade.png",
      SpecularNoiseTextureMoving1: "specular-a.png", SpecularNoiseTextureMoving2: "specular-b.png",
      FoamNoiseTexture: "foam.png",
    };
    this.ready = Promise.all([
      fetch(new URL("water.frag", base)).then((response) => {
        if (!response.ok) throw new Error("Pixel-water shader source unavailable");
        return response.text();
      }),
      Promise.all(Object.entries(files).map(async ([key, file]) => {
        const texture = await loadWorkerSafeTexture(pixi, new URL(file, base));
        texture.source.addressMode = "repeat";
        if (key !== "CausticTexture" && key !== "CausticHighlightTexture" && key !== "FoamNoiseTexture") {
          texture.source.scaleMode = "linear";
        }
        if (this.destroyed) texture.destroy(true);
        else this.textures.push(texture);
        return [key, texture.source];
      })),
    ]).then(([fragment, entries]) => {
      if (this.destroyed) return false;
      this.shader = pixi.Shader.from({
        gl: { vertex: VERTEX, fragment, name: "jess-pixel-water-port" },
        resources: { waterUniforms: this.uniforms, uShore: this.texture.source, ...Object.fromEntries(entries) },
      });
      this.mesh = new pixi.Mesh({ geometry: this.geometry, shader: this.shader });
      layer.addChild(this.mesh);
      return true;
    });
  }

  update(nowMs) {
    if (this.uniforms) this.uniforms.uniforms.uTime = nowMs / 1000;
  }

  destroy() {
    this.destroyed = true;
    this.mesh?.parent?.removeChild(this.mesh);
    this.mesh?.destroy();
    this.geometry?.destroy(true);
    this.shader?.destroy();
    this.texture?.destroy(true);
    for (const texture of this.textures || []) texture.destroy(true);
  }
}
