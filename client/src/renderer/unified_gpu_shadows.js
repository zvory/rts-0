import { KIND } from "../protocol.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";
import { projectedUnitShadowCandidate } from "./projected_unit_shadow_model_candidates.js";

// Presentation-only box models. Units are analytically projected into a coverage mask while
// terrain keeps its separate height-field ray march.
const PROXIES = Object.freeze({
  [KIND.RIFLEMAN]: [{ length: 10, width: 7, height: 15 }, { length: 6, width: 6, height: 22, forward: 0.8 }],
  [KIND.MACHINE_GUNNER]: [{ length: 13, width: 10, height: 15 }, { length: 7, width: 7, height: 22, forward: 0.8 }],
  [KIND.SCOUT_CAR]: [{ length: 44, width: 25, height: 22 }],
  [KIND.TANK]: [{ length: 55, width: 34, height: 33 }],
});

const CANDIDATE_KINDS = new Set([
  KIND.PANZERFAUST,
  KIND.ANTI_TANK_GUN,
  KIND.MORTAR_TEAM,
  KIND.ARTILLERY,
  KIND.COMMAND_CAR,
]);

const OCCLUDER_TEXTURE_SCALE = 0.5;
// Very low terrain sun remains dramatic, but literal unit projections become longer than the
// sprites are readable. Clamp only their presentation-only projection angle.
export const PROJECTED_UNIT_SHADOW_MIN_ELEVATION_DEGREES = 30;
export const PROJECTED_SHADOW_MAX_MARCH_STEPS = 96;
export const PROJECTED_SHADOW_MARCH_STEP_WORLD = 8;

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
uniform sampler2D uUnitShadowTexture;
uniform vec2 uMapWorldSize;
uniform vec2 uTerrainTextureSize;
uniform vec2 uSunDirection;
uniform float uTileSize;
uniform float uSunSlope;
uniform float uMarchStep;
uniform float uMaxDistance;
uniform float uPenumbra;
uniform float uAlpha;
out vec4 finalColor;

float terrainHeight(vec2 uv) {
  vec2 halfTexel = 0.5 / uTerrainTextureSize;
  return texture(uTerrainHeightTexture, clamp(uv, halfTexel, vec2(1.0) - halfTexel)).r * 255.0 * uTileSize;
}
void main(void) {
  if (vMapUv.x < 0.0 || vMapUv.y < 0.0 || vMapUv.x > 1.0 || vMapUv.y > 1.0) discard;
  float receiver = terrainHeight(vMapUv);
  float bestClearance = -10000.0;
  float hitDistance = uMaxDistance;
  for (int stepIndex = 1; stepIndex <= ${PROJECTED_SHADOW_MAX_MARCH_STEPS}; stepIndex += 1) {
    float distance = float(stepIndex) * uMarchStep;
    if (distance > uMaxDistance) break;
    vec2 sampleUv = vMapUv + uSunDirection * distance / uMapWorldSize;
    if (sampleUv.x < 0.0 || sampleUv.y < 0.0 || sampleUv.x > 1.0 || sampleUv.y > 1.0) break;
    float rayHeight = receiver + distance * uSunSlope;
    float clearance = terrainHeight(sampleUv) - rayHeight;
    if (clearance > bestClearance) {
      bestClearance = clearance;
      hitDistance = distance;
    }
  }
  float terrainCoverage = smoothstep(-uPenumbra, uPenumbra, bestClearance);
  float unitCoverage = texture(uUnitShadowTexture, vMapUv).r;
  float fade = 1.0 - 0.12 * clamp(hitDistance / max(uMaxDistance, 1.0), 0.0, 1.0);
  float coverage = max(terrainCoverage * fade, unitCoverage);
  float alpha = coverage * uAlpha;
  if (alpha <= 0.001) discard;
  finalColor = vec4(vec3(0.075, 0.064, 0.052) * alpha, alpha);
}
`;

export function hasProjectedUnitShadow(kind) {
  return Array.isArray(PROXIES[kind]) || CANDIDATE_KINDS.has(kind);
}

export function supportsUnifiedGpuShadowPass(map) {
  if (!map?.sun) return false;
  if (Number.isFinite(map.minElevation) && Number.isFinite(map.maxElevation)) {
    return map.maxElevation > map.minElevation;
  }
  let minElevation = Infinity;
  let maxElevation = -Infinity;
  for (const value of map.elevation || []) {
    const elevation = Number(value) || 0;
    minElevation = Math.min(minElevation, elevation);
    maxElevation = Math.max(maxElevation, elevation);
  }
  return maxElevation > minElevation;
}

/** GPU terrain ray march composited with an analytic projected-unit coverage mask. */
export class UnifiedGpuShadowLayer {
  constructor({ pixi = globalThis.PIXI, renderer, layer, recordDiagnostic = null } = {}) {
    this.pixi = pixi;
    this.renderer = renderer;
    this.layer = layer;
    this.recordDiagnostic = recordDiagnostic;
    this.map = null;
    this.enabled = false;
    this.unitShadowsEnabled = false;
    this.unitProjectionSlope = 1;
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
      uMarchStep: { value: PROJECTED_SHADOW_MARCH_STEP_WORLD, type: "f32" },
      uMaxDistance: { value: PROJECTED_SHADOW_MARCH_STEP_WORLD, type: "f32" },
      uPenumbra: { value: 1.8, type: "f32" },
      uAlpha: { value: 0.25, type: "f32" },
    });
    this.terrainHeightTexture = elevationTexture(pixi, { width: 1, height: 1, elevation: [0] });
    this.unitShadowTexture = pixi.RenderTexture.create({ width: 1, height: 1, resolution: 1 });
    this.unitShadowGraphics = new pixi.Graphics();
    this.shader = this._createShader();
    this.mesh = new pixi.Mesh({ geometry: this.geometry, shader: this.shader });
    this.mesh.eventMode = "none";
    this.mesh.visible = false;
    layer?.addChild?.(this.mesh);
  }

  setMap(map) {
    if (!this.supported) return;
    this.map = normalizedMap(map);
    this.enabled = supportsUnifiedGpuShadowPass(this.map);
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
    // This is ordinary coverage rather than encoded height data, so linear filtering is safe and
    // gives the half-resolution mask a stable one-pixel antialiased edge.
    unitTexture.source.scaleMode = "linear";
    const shader = this._createShader(terrainTexture, unitTexture);
    this.mesh.shader = shader;
    this.shader?.destroy?.();
    this.terrainHeightTexture?.destroy?.(true);
    this.unitShadowTexture?.destroy?.(true);
    this.shader = shader;
    this.terrainHeightTexture = terrainTexture;
    this.unitShadowTexture = unitTexture;
    const positionBuffer = this.geometry.getAttribute("aPosition").buffer;
    positionBuffer.data.set([0, 0, worldWidth, 0, worldWidth, worldHeight, 0, worldHeight]);
    positionBuffer.update();
    const azimuth = this.map.sun.azimuthDegrees * Math.PI / 180;
    const elevation = this.map.sun.elevationDegrees * Math.PI / 180;
    const slope = Math.max(0.05, Math.tan(elevation));
    const unitElevation = Math.max(
      elevation,
      PROJECTED_UNIT_SHADOW_MIN_ELEVATION_DEGREES * Math.PI / 180,
    );
    this.unitProjectionSlope = Math.tan(unitElevation);
    const maxVertical = (this.map.maxElevation - this.map.minElevation) * this.map.tileSize;
    this.uniforms.uniforms.uMapWorldSize[0] = worldWidth;
    this.uniforms.uniforms.uMapWorldSize[1] = worldHeight;
    this.uniforms.uniforms.uTerrainTextureSize[0] = this.map.width;
    this.uniforms.uniforms.uTerrainTextureSize[1] = this.map.height;
    this.uniforms.uniforms.uSunDirection[0] = Math.sin(azimuth);
    this.uniforms.uniforms.uSunDirection[1] = -Math.cos(azimuth);
    this.uniforms.uniforms.uTileSize = this.map.tileSize;
    this.uniforms.uniforms.uSunSlope = slope;
    this.uniforms.uniforms.uMaxDistance = Math.min(
      PROJECTED_SHADOW_MAX_MARCH_STEPS * PROJECTED_SHADOW_MARCH_STEP_WORLD,
      maxVertical / slope + this.map.tileSize * 2);
    this.uniforms.uniforms.uPenumbra = this.map.sun.elevationDegrees <= 20 ? 1.8 : 1.2;
    this.mesh.visible = true;
  }

  setUnitShadowsEnabled(enabled) {
    const next = enabled === true;
    if (next === this.unitShadowsEnabled) return;
    this.unitShadowsEnabled = next;
    this.projectedEntityIds.clear();
    if (next || !this.supported || !this.enabled || !this.map) return;
    this.unitShadowGraphics.clear();
    this.renderer.render({
      container: this.unitShadowGraphics,
      target: this.unitShadowTexture,
      clear: true,
      clearColor: [0, 0, 0, 0],
    });
  }

  update(entities) {
    this.projectedEntityIds.clear();
    if (!this.supported || !this.enabled || !this.map || !this.unitShadowsEnabled) return 0;
    // The render-texture update and native-shadow suppression are one transaction. If the
    // offscreen render fails, _drawSafely catches the error; keep this mesh hidden and leave the
    // id set empty so the unit rigs fall back to their native shadows instead of combining them
    // with stale GPU occluders from the previous frame.
    this.mesh.visible = false;
    const shapes = [];
    for (const entity of entities || []) {
      const volumes = proxyVolumesFor(entity);
      if (!volumes || entity?.visionOnly) continue;
      const x = finite(entity.x, 0);
      const y = finite(entity.y, 0);
      const facing = finite(entity.facing, 0);
      for (const volume of volumes) shapes.push({ entityId: entity.id, x, y, facing, ...volume });
    }
    const graphics = this.unitShadowGraphics;
    graphics.clear();
    for (const shape of shapes) {
      graphics.poly(projectedProxyPolygon(
        shape,
        this.uniforms.uniforms.uSunDirection,
        this.unitProjectionSlope,
        OCCLUDER_TEXTURE_SCALE,
      ));
      graphics.fill({ color: 0xffffff, alpha: 1 });
    }
    this.renderer.render({ container: graphics, target: this.unitShadowTexture, clear: true, clearColor: [0, 0, 0, 0] });
    for (const shape of shapes) this.projectedEntityIds.add(shape.entityId);
    this.mesh.visible = true;
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.projectedBoxes", shapes.length);
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.drawCalls", 2);
    return shapes.length;
  }

  hasShadowFor(entityId) { return this.projectedEntityIds.has(entityId); }

  _createShader(terrain = this.terrainHeightTexture, units = this.unitShadowTexture) {
    return this.pixi.Shader.from({
      gl: { vertex: VERTEX, fragment: FRAGMENT, name: "unified-gpu-height-field-shadows" },
      resources: { shadowUniforms: this.uniforms, uTerrainHeightTexture: terrain.source, uUnitShadowTexture: units.source },
    });
  }

  destroy() {
    this.mesh?.parent?.removeChild?.(this.mesh);
    this.mesh?.destroy?.();
    this.geometry?.destroy?.(true);
    this.shader?.destroy?.();
    this.terrainHeightTexture?.destroy?.(true);
    this.unitShadowTexture?.destroy?.(true);
    this.unitShadowGraphics?.destroy?.();
    this.projectedEntityIds.clear();
    this.supported = false;
  }
}

export function projectedProxyPolygon(shape, sunDirection, sunSlope, scale = 1) {
  const center = shape.center || [0, 0, 0];
  const size = shape.size || [1, 1, 1];
  const facing = finite(shape.facing, 0);
  const unitCos = Math.cos(facing);
  const unitSin = Math.sin(facing);
  const centerX = finite(shape.x, 0) + center[0] * unitCos - center[1] * unitSin;
  const centerY = finite(shape.y, 0) + center[0] * unitSin + center[1] * unitCos;
  const partAngle = facing + finite(shape.yaw, 0);
  const yawCos = Math.cos(partAngle);
  const yawSin = Math.sin(partAngle);
  const pitch = finite(shape.pitch, 0);
  const pitchCos = Math.cos(pitch);
  const pitchSin = Math.sin(pitch);
  const inverseSlope = 1 / Math.max(0.05, finite(sunSlope, 1));
  const sunX = finite(sunDirection?.[0], 0);
  const sunY = finite(sunDirection?.[1], -1);
  const corners = [];
  for (const sx of [-0.5, 0.5]) {
    for (const sy of [-0.5, 0.5]) {
      for (const sz of [-0.5, 0.5]) {
        const localX = size[0] * sx;
        const localY = size[1] * sy;
        const localZ = size[2] * sz;
        // Positive pitch raises the forward (+x) end of the box.
        const pitchedX = localX * pitchCos - localZ * pitchSin;
        const pitchedZ = localX * pitchSin + localZ * pitchCos;
        const worldX = centerX + pitchedX * yawCos - localY * yawSin;
        const worldY = centerY + pitchedX * yawSin + localY * yawCos;
        const relativeHeight = Math.max(0, center[2] + pitchedZ);
        const projectionDistance = relativeHeight * inverseSlope;
        corners.push({
          x: (worldX - sunX * projectionDistance) * scale,
          y: (worldY - sunY * projectionDistance) * scale,
        });
      }
    }
  }
  return convexHull(corners).flatMap((point) => [point.x, point.y]);
}

function proxyVolumesFor(entity) {
  const fixed = PROXIES[entity?.kind];
  if (fixed) {
    return fixed.map((volume) => ({
      center: [finite(volume.forward, 0), finite(volume.right, 0), volume.height * 0.5],
      size: [volume.length, volume.width, volume.height],
      yaw: 0,
      pitch: 0,
    }));
  }
  const model = projectedUnitShadowCandidate(entity?.kind, entity?.id);
  return model?.parts || null;
}

function convexHull(points) {
  const sorted = [...points].sort((a, b) => a.x - b.x || a.y - b.y);
  if (sorted.length <= 2) return sorted;
  const lower = [];
  for (const point of sorted) {
    while (lower.length >= 2 && cross(lower.at(-2), lower.at(-1), point) <= 0) lower.pop();
    lower.push(point);
  }
  const upper = [];
  for (let index = sorted.length - 1; index >= 0; index -= 1) {
    const point = sorted[index];
    while (upper.length >= 2 && cross(upper.at(-2), upper.at(-1), point) <= 0) upper.pop();
    upper.push(point);
  }
  lower.pop();
  upper.pop();
  return lower.concat(upper);
}

function cross(origin, a, b) {
  return (a.x - origin.x) * (b.y - origin.y) - (a.y - origin.y) * (b.x - origin.x);
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

function finite(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}
