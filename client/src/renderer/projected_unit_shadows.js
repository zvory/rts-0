import { KIND } from "../protocol.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";

// Coarse presentation-only occluders. Terrain elevation and authored sunlight determine the
// projected footprint; these proxies intentionally stay simpler than the production unit rigs.
const PROXIES = Object.freeze({
  [KIND.RIFLEMAN]: Object.freeze([
    Object.freeze({ length: 10, width: 7, z0: 0, z1: 15 }),
    Object.freeze({ length: 6, width: 6, z0: 15, z1: 22, forward: 0.8 }),
  ]),
  [KIND.MACHINE_GUNNER]: Object.freeze([
    Object.freeze({ length: 13, width: 10, z0: 0, z1: 15 }),
    Object.freeze({ length: 7, width: 7, z0: 15, z1: 22, forward: 0.8 }),
  ]),
  [KIND.SCOUT_CAR]: Object.freeze([
    Object.freeze({ length: 44, width: 25, z0: 0, z1: 22 }),
  ]),
  [KIND.TANK]: Object.freeze([
    Object.freeze({ length: 55, width: 34, z0: 0, z1: 33 }),
  ]),
});

const FLOATS_PER_INSTANCE = 11;
const MAX_INSTANCES = 800;

const VERTEX = `#version 300 es
precision highp float;
in vec2 aPosition;
in vec2 aOrigin;
in vec2 aFacing;
in vec4 aSizeHeight;
in float aForward;
in float aBaseHeight;
in float aMaxHeight;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
uniform vec2 uShadowVector;
uniform float uSoftness;

out vec2 vWorldPosition;
flat out vec2 vOrigin;
flat out vec2 vFacing;
flat out vec4 vSizeHeight;
flat out float vForward;
flat out float vBaseHeight;

void main(void) {
  vec2 right = vec2(-aFacing.y, aFacing.x);
  vec2 baseCenter = aOrigin + aFacing * aForward;
  vec2 farShift = uShadowVector * aMaxHeight;
  vec2 center = baseCenter + farShift * 0.5;
  vec2 boxExtent = abs(aFacing) * aSizeHeight.x + abs(right) * aSizeHeight.y;
  vec2 extent = boxExtent + abs(farShift) * 0.5 + vec2(uSoftness + 2.0);
  vec2 world = center + aPosition * extent;

  vWorldPosition = world;
  vOrigin = aOrigin;
  vFacing = aFacing;
  vSizeHeight = aSizeHeight;
  vForward = aForward;
  vBaseHeight = aBaseHeight;
  mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
  gl_Position = vec4((mvp * vec3(world, 1.0)).xy, 0.0, 1.0);
}
`;

const FRAGMENT = `#version 300 es
precision highp float;
in vec2 vWorldPosition;
flat in vec2 vOrigin;
flat in vec2 vFacing;
flat in vec4 vSizeHeight;
flat in float vForward;
flat in float vBaseHeight;

uniform sampler2D uElevationTexture;
uniform vec2 uMapWorldSize;
uniform vec2 uElevationTextureSize;
uniform vec2 uShadowVector;
uniform float uTileSize;
uniform float uSoftness;
uniform float uErosion;
uniform float uAlpha;

out vec4 finalColor;

float boxDistance(vec2 point, vec2 halfSize) {
  vec2 delta = abs(point) - halfSize;
  return length(max(delta, 0.0)) + min(max(delta.x, delta.y), 0.0);
}

float receiverHeight(vec2 world) {
  vec2 halfTexel = 0.5 / uElevationTextureSize;
  vec2 uv = clamp(world / uMapWorldSize, halfTexel, vec2(1.0) - halfTexel);
  return texture(uElevationTexture, uv).r * 255.0 * uTileSize;
}

void main(void) {
  float receiver = receiverHeight(vWorldPosition);
  float lowHeight = max(0.0, vBaseHeight + vSizeHeight.z - receiver);
  float highHeight = max(0.0, vBaseHeight + vSizeHeight.w - receiver);
  if (highHeight <= 0.001) discard;

  vec2 right = vec2(-vFacing.y, vFacing.x);
  vec2 lowShift = uShadowVector * lowHeight;
  vec2 baseCenter = vOrigin + vFacing * vForward + lowShift;
  vec2 point = vWorldPosition - baseCenter;
  vec2 local = vec2(dot(point, vFacing), dot(point, right));
  vec2 sweepWorld = uShadowVector * (highHeight - lowHeight);
  vec2 sweep = vec2(dot(sweepWorld, vFacing), dot(sweepWorld, right));
  float sweepLengthSq = max(dot(sweep, sweep), 0.0001);
  float alongSweep = clamp(dot(local, sweep) / sweepLengthSq, 0.0, 1.0);
  float distanceToShadow = boxDistance(local - sweep * alongSweep, vSizeHeight.xy);
  float edge = max(fwidth(distanceToShadow), uSoftness);
  float coverage = 1.0 - smoothstep(-edge, edge, distanceToShadow + uErosion);
  float alpha = coverage * uAlpha;
  finalColor = vec4(vec3(0.075, 0.064, 0.052) * alpha, alpha);
}
`;

export function hasProjectedUnitShadow(kind) {
  return Array.isArray(PROXIES[kind]);
}

export function projectedUnitShadowHeight(kind) {
  const volumes = PROXIES[kind];
  if (!volumes) return null;
  return Math.max(...volumes.map((volume) => volume.z1));
}

/** One instanced draw whose fragments sample the static height field at each shadow receiver. */
export class ProjectedUnitShadowLayer {
  constructor({ pixi = globalThis.PIXI, layer, recordDiagnostic = null } = {}) {
    this.pixi = pixi;
    this.layer = layer;
    this.recordDiagnostic = recordDiagnostic;
    this.map = null;
    this.enabled = false;
    this.supported = Boolean(
      pixi?.Geometry && pixi?.Mesh && pixi?.Shader && pixi?.Buffer && pixi?.UniformGroup,
    );
    this.instanceData = null;
    this.instanceBuffer = null;
    this.geometry = null;
    this.uniforms = null;
    this.elevationTexture = null;
    this.shader = null;
    this.mesh = null;
    if (!this.supported) return;
    this.instanceData = new Float32Array(MAX_INSTANCES * FLOATS_PER_INSTANCE);
    this.instanceBuffer = new pixi.Buffer({
      data: this.instanceData,
      usage: pixi.BufferUsage.VERTEX,
      shrinkToFit: false,
      label: "projected-unit-shadow-instances",
    });
    const stride = FLOATS_PER_INSTANCE * 4;
    this.geometry = new pixi.Geometry({
      attributes: {
        aPosition: {
          buffer: new Float32Array([-1, -1, 1, -1, 1, 1, -1, 1]),
          format: "float32x2",
        },
        aOrigin: { buffer: this.instanceBuffer, format: "float32x2", stride, offset: 0, instance: true },
        aFacing: { buffer: this.instanceBuffer, format: "float32x2", stride, offset: 8, instance: true },
        aSizeHeight: { buffer: this.instanceBuffer, format: "float32x4", stride, offset: 16, instance: true },
        aForward: { buffer: this.instanceBuffer, format: "float32", stride, offset: 32, instance: true },
        aBaseHeight: { buffer: this.instanceBuffer, format: "float32", stride, offset: 36, instance: true },
        aMaxHeight: { buffer: this.instanceBuffer, format: "float32", stride, offset: 40, instance: true },
      },
      indexBuffer: new Uint16Array([0, 1, 2, 0, 2, 3]),
      instanceCount: 0,
    });
    this.uniforms = new pixi.UniformGroup({
      uMapWorldSize: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      uElevationTextureSize: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      uShadowVector: { value: new Float32Array([0, 1]), type: "vec2<f32>" },
      uTileSize: { value: 32, type: "f32" },
      uSoftness: { value: 2.8, type: "f32" },
      uErosion: { value: 1.15, type: "f32" },
      uAlpha: { value: 0.22, type: "f32" },
    });
    this.elevationTexture = elevationTexture(pixi, { width: 1, height: 1, elevation: [0] });
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
    this.geometry.instanceCount = 0;
    this.mesh.visible = false;
    if (!this.enabled) return;

    const texture = elevationTexture(this.pixi, this.map);
    const shader = this._createShader(texture);
    this.mesh.shader = shader;
    this.shader?.destroy?.();
    this.elevationTexture?.destroy?.(true);
    this.shader = shader;
    this.elevationTexture = texture;

    const azimuth = this.map.sun.azimuthDegrees * Math.PI / 180;
    const elevation = this.map.sun.elevationDegrees * Math.PI / 180;
    const inverseSlope = 1 / Math.max(0.01, Math.tan(elevation));
    this.uniforms.uniforms.uShadowVector[0] = -Math.sin(azimuth) * inverseSlope;
    this.uniforms.uniforms.uShadowVector[1] = Math.cos(azimuth) * inverseSlope;
    this.uniforms.uniforms.uMapWorldSize[0] = this.map.width * this.map.tileSize;
    this.uniforms.uniforms.uMapWorldSize[1] = this.map.height * this.map.tileSize;
    this.uniforms.uniforms.uElevationTextureSize[0] = this.map.width;
    this.uniforms.uniforms.uElevationTextureSize[1] = this.map.height;
    this.uniforms.uniforms.uTileSize = this.map.tileSize;
    this.uniforms.uniforms.uSoftness = this.map.sun.elevationDegrees <= 20 ? 2.8 : 2.1;
  }

  update(entities) {
    if (!this.supported) return 0;
    if (!this.enabled || !this.map) {
      this.geometry.instanceCount = 0;
      this.mesh.visible = false;
      return 0;
    }
    let count = 0;
    for (const entity of entities || []) {
      const volumes = PROXIES[entity?.kind];
      if (!volumes) continue;
      const facing = finite(entity.facing, 0);
      const baseLevel = bilinearElevation(this.map, finite(entity.x, 0), finite(entity.y, 0));
      const baseHeight = baseLevel * this.map.tileSize;
      for (const volume of volumes) {
        if (count >= MAX_INSTANCES) break;
        const offset = count * FLOATS_PER_INSTANCE;
        this.instanceData[offset] = finite(entity.x, 0);
        this.instanceData[offset + 1] = finite(entity.y, 0);
        this.instanceData[offset + 2] = Math.cos(facing);
        this.instanceData[offset + 3] = Math.sin(facing);
        this.instanceData[offset + 4] = volume.length * 0.5;
        this.instanceData[offset + 5] = volume.width * 0.5;
        this.instanceData[offset + 6] = volume.z0;
        this.instanceData[offset + 7] = volume.z1;
        this.instanceData[offset + 8] = finite(volume.forward, 0);
        this.instanceData[offset + 9] = baseHeight;
        this.instanceData[offset + 10] = baseHeight - this.map.minElevation * this.map.tileSize + volume.z1;
        count += 1;
      }
    }
    this.instanceBuffer.setDataWithSize(this.instanceData, count * FLOATS_PER_INSTANCE, true);
    this.geometry.instanceCount = count;
    this.mesh.visible = count > 0;
    this.recordDiagnostic?.("renderer.projectedUnitShadows.instances", count);
    this.recordDiagnostic?.("renderer.projectedUnitShadows.drawCalls", count > 0 ? 1 : 0);
    return count;
  }

  _createShader(texture = this.elevationTexture) {
    return this.pixi.Shader.from({
      gl: { vertex: VERTEX, fragment: FRAGMENT, name: "projected-unit-shadows" },
      resources: {
        shadowUniforms: this.uniforms,
        uElevationTexture: texture.source,
      },
    });
  }

  destroy() {
    this.mesh?.parent?.removeChild?.(this.mesh);
    this.mesh?.destroy?.();
    this.geometry?.destroy?.(true);
    this.shader?.destroy?.();
    this.elevationTexture?.destroy?.(true);
    this.mesh = null;
    this.geometry = null;
    this.shader = null;
    this.elevationTexture = null;
    this.instanceBuffer = null;
    this.instanceData = null;
    this.map = null;
    this.supported = false;
  }
}

function elevationTexture(pixi, map) {
  const canvas = createWorkerSafeCanvas();
  canvas.width = map.width;
  canvas.height = map.height;
  const ctx = canvas.getContext("2d", { alpha: false });
  const image = ctx.createImageData(map.width, map.height);
  for (let index = 0; index < map.width * map.height; index += 1) {
    const value = Math.max(0, Math.min(255, Number(map.elevation?.[index]) || 0));
    const offset = index * 4;
    image.data[offset] = value;
    image.data[offset + 1] = value;
    image.data[offset + 2] = value;
    image.data[offset + 3] = 255;
  }
  ctx.putImageData(image, 0, 0);
  const texture = pixi.Texture.from(canvas);
  if (texture.source?.style) texture.source.style.scaleMode = "linear";
  return texture;
}

function normalizedMap(map) {
  const width = Math.max(1, Math.trunc(Number(map?.width)) || 1);
  const height = Math.max(1, Math.trunc(Number(map?.height)) || 1);
  const tileSize = Math.max(1, Number(map?.tileSize) || 32);
  const elevation = Array.from(map?.elevation || new Uint8Array(width * height));
  let minElevation = Infinity;
  let maxElevation = -Infinity;
  for (const value of elevation) {
    const level = Number(value) || 0;
    minElevation = Math.min(minElevation, level);
    maxElevation = Math.max(maxElevation, level);
  }
  return {
    width,
    height,
    tileSize,
    elevation,
    minElevation: Number.isFinite(minElevation) ? minElevation : 0,
    maxElevation: Number.isFinite(maxElevation) ? maxElevation : 0,
    sun: map?.sun ? { ...map.sun } : null,
  };
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
  return Number(map.elevation[ty * map.width + tx]) || 0;
}

function finite(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}
