import { KIND } from "../protocol.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";

// Presentation-only occluders. Terrain and these live volumes feed the same height-field pass.
const PROXIES = Object.freeze({
  [KIND.RIFLEMAN]: [{ length: 10, width: 7, height: 15 }, { length: 6, width: 6, height: 22, forward: 0.8 }],
  [KIND.MACHINE_GUNNER]: [{ length: 13, width: 10, height: 15 }, { length: 7, width: 7, height: 22, forward: 0.8 }],
  [KIND.SCOUT_CAR]: [{ length: 44, width: 25, height: 22 }],
  [KIND.TANK]: [{ length: 55, width: 34, height: 33 }],
});

const OCCLUDER_TEXTURE_SCALE = 0.5;
const MAX_MARCH_STEPS = 96;
const MARCH_STEP_WORLD = 8;

const VERTEX = `#version 300 es
precision highp float;
in vec2 aPosition;
uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
uniform vec2 uMapWorldSize;
out vec2 vMapUv;
void main(void) {
  vMapUv = aPosition / uMapWorldSize;
  mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
  gl_Position = vec4((mvp * vec3(aPosition, 1.0)).xy, 0.0, 1.0);
}
`;

const FRAGMENT = `#version 300 es
precision highp float;
in vec2 vMapUv;
uniform sampler2D uTerrainHeightTexture;
uniform sampler2D uUnitHeightTexture;
uniform vec2 uMapWorldSize;
uniform vec2 uTerrainTextureSize;
uniform vec2 uSunDirection;
uniform float uTileSize;
uniform float uSunSlope;
uniform float uMaxHeight;
uniform float uMarchStep;
uniform float uMaxDistance;
uniform float uPenumbra;
uniform float uAlpha;
out vec4 finalColor;

float terrainHeight(vec2 uv) {
  vec2 halfTexel = 0.5 / uTerrainTextureSize;
  return texture(uTerrainHeightTexture, clamp(uv, halfTexel, vec2(1.0) - halfTexel)).r * 255.0 * uTileSize;
}
float blockerHeight(vec2 uv) {
  return max(terrainHeight(uv), texture(uUnitHeightTexture, uv).r * uMaxHeight);
}
void main(void) {
  if (vMapUv.x < 0.0 || vMapUv.y < 0.0 || vMapUv.x > 1.0 || vMapUv.y > 1.0) discard;
  float receiver = terrainHeight(vMapUv);
  float bestClearance = -10000.0;
  float hitDistance = uMaxDistance;
  for (int stepIndex = 1; stepIndex <= ${MAX_MARCH_STEPS}; stepIndex += 1) {
    float distance = float(stepIndex) * uMarchStep;
    if (distance > uMaxDistance) break;
    vec2 sampleUv = vMapUv + uSunDirection * distance / uMapWorldSize;
    if (sampleUv.x < 0.0 || sampleUv.y < 0.0 || sampleUv.x > 1.0 || sampleUv.y > 1.0) break;
    float clearance = blockerHeight(sampleUv) - receiver - distance * uSunSlope;
    if (clearance > bestClearance) {
      bestClearance = clearance;
      hitDistance = distance;
    }
  }
  float coverage = smoothstep(-uPenumbra, uPenumbra, bestClearance);
  float fade = 1.0 - 0.12 * clamp(hitDistance / max(uMaxDistance, 1.0), 0.0, 1.0);
  float alpha = coverage * fade * uAlpha;
  if (alpha <= 0.001) discard;
  finalColor = vec4(vec3(0.075, 0.064, 0.052) * alpha, alpha);
}
`;

export function hasProjectedUnitShadow(kind) {
  return Array.isArray(PROXIES[kind]);
}

/** GPU ray march shared by terrain and live unit proxy volumes. */
export class UnifiedGpuShadowLayer {
  constructor({ pixi = globalThis.PIXI, renderer, layer, recordDiagnostic = null } = {}) {
    this.pixi = pixi;
    this.renderer = renderer;
    this.layer = layer;
    this.recordDiagnostic = recordDiagnostic;
    this.map = null;
    this.enabled = false;
    this.supported = Boolean(pixi?.Geometry && pixi?.Mesh && pixi?.Shader && pixi?.UniformGroup
      && pixi?.Graphics && pixi?.RenderTexture && renderer?.render);
    this.projectedEntityIds = new Set();
    if (!this.supported) return;
    this.geometry = new pixi.Geometry({
      attributes: { aPosition: { buffer: new Float32Array(8), format: "float32x2" } },
      indexBuffer: new Uint16Array([0, 1, 2, 0, 2, 3]),
    });
    this.uniforms = new pixi.UniformGroup({
      uMapWorldSize: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      uTerrainTextureSize: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      uSunDirection: { value: new Float32Array([0, -1]), type: "vec2<f32>" },
      uTileSize: { value: 32, type: "f32" },
      uSunSlope: { value: 1, type: "f32" },
      uMaxHeight: { value: 1, type: "f32" },
      uMarchStep: { value: MARCH_STEP_WORLD, type: "f32" },
      uMaxDistance: { value: MARCH_STEP_WORLD, type: "f32" },
      uPenumbra: { value: 1.8, type: "f32" },
      uAlpha: { value: 0.25, type: "f32" },
    });
    this.terrainHeightTexture = elevationTexture(pixi, { width: 1, height: 1, elevation: [0] });
    this.unitHeightTexture = pixi.RenderTexture.create({ width: 1, height: 1, resolution: 1 });
    this.unitHeightGraphics = new pixi.Graphics();
    this.shader = this._createShader();
    this.mesh = new pixi.Mesh({ geometry: this.geometry, shader: this.shader });
    this.mesh.eventMode = "none";
    this.mesh.visible = false;
    layer?.addChild?.(this.mesh);
  }

  setMap(map) {
    if (!this.supported) return;
    this.map = normalizedMap(map);
    this.enabled = Boolean(this.map?.sun && this.map.maxElevation > this.map.minElevation);
    this.mesh.visible = false;
    this.projectedEntityIds.clear();
    if (!this.enabled) return;
    const worldWidth = this.map.width * this.map.tileSize;
    const worldHeight = this.map.height * this.map.tileSize;
    const terrainTexture = elevationTexture(this.pixi, this.map);
    const unitTexture = this.pixi.RenderTexture.create({
      width: Math.max(1, Math.ceil(worldWidth * OCCLUDER_TEXTURE_SCALE)),
      height: Math.max(1, Math.ceil(worldHeight * OCCLUDER_TEXTURE_SCALE)),
      resolution: 1,
    });
    unitTexture.source.scaleMode = "linear";
    const shader = this._createShader(terrainTexture, unitTexture);
    this.mesh.shader = shader;
    this.shader?.destroy?.();
    this.terrainHeightTexture?.destroy?.(true);
    this.unitHeightTexture?.destroy?.(true);
    this.shader = shader;
    this.terrainHeightTexture = terrainTexture;
    this.unitHeightTexture = unitTexture;
    const positionBuffer = this.geometry.getAttribute("aPosition").buffer;
    positionBuffer.data.set([0, 0, worldWidth, 0, worldWidth, worldHeight, 0, worldHeight]);
    positionBuffer.update();
    const azimuth = this.map.sun.azimuthDegrees * Math.PI / 180;
    const elevation = this.map.sun.elevationDegrees * Math.PI / 180;
    const slope = Math.max(0.05, Math.tan(elevation));
    const maxVertical = (this.map.maxElevation - this.map.minElevation) * this.map.tileSize + 40;
    this.uniforms.uniforms.uMapWorldSize[0] = worldWidth;
    this.uniforms.uniforms.uMapWorldSize[1] = worldHeight;
    this.uniforms.uniforms.uTerrainTextureSize[0] = this.map.width;
    this.uniforms.uniforms.uTerrainTextureSize[1] = this.map.height;
    this.uniforms.uniforms.uSunDirection[0] = Math.sin(azimuth);
    this.uniforms.uniforms.uSunDirection[1] = -Math.cos(azimuth);
    this.uniforms.uniforms.uTileSize = this.map.tileSize;
    this.uniforms.uniforms.uSunSlope = slope;
    this.uniforms.uniforms.uMaxHeight = this.map.maxElevation * this.map.tileSize + 40;
    this.uniforms.uniforms.uMaxDistance = Math.min(MAX_MARCH_STEPS * MARCH_STEP_WORLD,
      maxVertical / slope + this.map.tileSize * 2);
    this.uniforms.uniforms.uPenumbra = this.map.sun.elevationDegrees <= 20 ? 1.8 : 1.2;
    this.mesh.visible = true;
  }

  update(entities) {
    this.projectedEntityIds.clear();
    if (!this.supported || !this.enabled || !this.map) return 0;
    const shapes = [];
    for (const entity of entities || []) {
      const volumes = PROXIES[entity?.kind];
      if (!volumes || entity?.visionOnly) continue;
      const x = finite(entity.x, 0);
      const y = finite(entity.y, 0);
      const facing = finite(entity.facing, 0);
      const baseHeight = bilinearElevation(this.map, x, y) * this.map.tileSize;
      for (const volume of volumes) shapes.push({ entityId: entity.id, x, y, facing, baseHeight, ...volume });
    }
    shapes.sort((a, b) => (a.baseHeight + a.height) - (b.baseHeight + b.height));
    const graphics = this.unitHeightGraphics;
    graphics.clear();
    for (const shape of shapes) {
      const encoded = Math.max(1, Math.min(255, Math.round(
        (shape.baseHeight + shape.height) / this.uniforms.uniforms.uMaxHeight * 255,
      )));
      graphics.poly(proxyPolygon(shape, OCCLUDER_TEXTURE_SCALE));
      graphics.fill({ color: (encoded << 16) | (encoded << 8) | encoded, alpha: 1 });
      this.projectedEntityIds.add(shape.entityId);
    }
    this.renderer.render({ container: graphics, target: this.unitHeightTexture, clear: true, clearColor: [0, 0, 0, 0] });
    this.mesh.visible = true;
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.occluders", shapes.length);
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.drawCalls", 2);
    return shapes.length;
  }

  hasShadowFor(entityId) { return this.projectedEntityIds.has(entityId); }

  _createShader(terrain = this.terrainHeightTexture, units = this.unitHeightTexture) {
    return this.pixi.Shader.from({
      gl: { vertex: VERTEX, fragment: FRAGMENT, name: "unified-gpu-height-field-shadows" },
      resources: { shadowUniforms: this.uniforms, uTerrainHeightTexture: terrain.source, uUnitHeightTexture: units.source },
    });
  }

  destroy() {
    this.mesh?.parent?.removeChild?.(this.mesh);
    this.mesh?.destroy?.();
    this.geometry?.destroy?.(true);
    this.shader?.destroy?.();
    this.terrainHeightTexture?.destroy?.(true);
    this.unitHeightTexture?.destroy?.(true);
    this.unitHeightGraphics?.destroy?.();
    this.projectedEntityIds.clear();
    this.supported = false;
  }
}

function proxyPolygon(shape, scale) {
  const fx = Math.cos(shape.facing);
  const fy = Math.sin(shape.facing);
  const rx = -fy;
  const ry = fx;
  const cx = shape.x + fx * finite(shape.forward, 0);
  const cy = shape.y + fy * finite(shape.forward, 0);
  const hl = shape.length * 0.5;
  const hw = shape.width * 0.5;
  return [
    (cx - fx * hl - rx * hw) * scale, (cy - fy * hl - ry * hw) * scale,
    (cx + fx * hl - rx * hw) * scale, (cy + fy * hl - ry * hw) * scale,
    (cx + fx * hl + rx * hw) * scale, (cy + fy * hl + ry * hw) * scale,
    (cx - fx * hl + rx * hw) * scale, (cy - fy * hl + ry * hw) * scale,
  ];
}

function elevationTexture(pixi, map) {
  const canvas = createWorkerSafeCanvas();
  canvas.width = map.width;
  canvas.height = map.height;
  const ctx = canvas.getContext("2d", { alpha: false });
  const image = ctx.createImageData(map.width, map.height);
  for (let index = 0; index < map.width * map.height; index += 1) {
    const value = Math.max(0, Math.min(255, Number(map.elevation?.[index]) || 0));
    image.data.fill(value, index * 4, index * 4 + 3);
    image.data[index * 4 + 3] = 255;
  }
  ctx.putImageData(image, 0, 0);
  const texture = pixi.Texture.from(canvas);
  texture.source.scaleMode = "linear";
  return texture;
}

function normalizedMap(map) {
  const width = Math.max(1, Math.trunc(Number(map?.width)) || 1);
  const height = Math.max(1, Math.trunc(Number(map?.height)) || 1);
  const tileSize = Math.max(1, Number(map?.tileSize) || 32);
  const elevation = Array.from(map?.elevation || new Uint8Array(width * height));
  const values = elevation.map((value) => Number(value) || 0);
  return { width, height, tileSize, elevation, minElevation: Math.min(...values), maxElevation: Math.max(...values), sun: map?.sun ? { ...map.sun } : null };
}

function bilinearElevation(map, worldX, worldY) {
  const x = worldX / map.tileSize - 0.5;
  const y = worldY / map.tileSize - 0.5;
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const fx = x - x0;
  const fy = y - y0;
  const top = elevationAt(map, x0, y0) * (1 - fx) + elevationAt(map, x0 + 1, y0) * fx;
  const bottom = elevationAt(map, x0, y0 + 1) * (1 - fx) + elevationAt(map, x0 + 1, y0 + 1) * fx;
  return top * (1 - fy) + bottom * fy;
}
function elevationAt(map, x, y) {
  const tx = Math.max(0, Math.min(map.width - 1, x));
  const ty = Math.max(0, Math.min(map.height - 1, y));
  return Number(map.elevation?.[ty * map.width + tx]) || 0;
}
function finite(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}
