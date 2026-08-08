import { KIND } from "../protocol.js";
import { createWorkerSafeCanvas } from "./raster_primitives.js";
import { projectedUnitShadowCandidate } from "./projected_unit_shadow_model_candidates.js";

// Presentation-only box models. Static terrain uses a cached directional horizon transform;
// units are projected into a camera-bounded coverage mask every presentation frame.
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
const MASK_FILTER_GUTTER_TEXELS = 2;
const INSTANCE_FLOATS = 11;
const INSTANCE_STRIDE_BYTES = INSTANCE_FLOATS * Float32Array.BYTES_PER_ELEMENT;
const INITIAL_INSTANCE_CAPACITY = 1024;
export const STATIC_SHADOW_SAMPLES_PER_TILE = 4;

const VERTEX = `#version 300 es
precision highp float;
in vec2 aPosition;
uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
uniform vec2 uMapWorldSize;
out vec2 vMapUv;
out vec2 vWorldPosition;
void main(void) {
  vMapUv = aPosition / uMapWorldSize;
  vWorldPosition = aPosition;
  mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
  gl_Position = vec4((mvp * vec3(aPosition, 1.0)).xy, 0.0, 1.0);
}
`;

const FRAGMENT = `#version 300 es
precision highp float;
in vec2 vMapUv;
in vec2 vWorldPosition;
uniform sampler2D uStaticShadowTexture;
uniform sampler2D uUnitShadowTexture;
uniform vec2 uMapWorldSize;
uniform float uAlpha;
uniform vec2 uUnitMaskOrigin;
uniform vec2 uUnitMaskWorldSize;
out vec4 finalColor;

void main(void) {
  if (vMapUv.x < 0.0 || vMapUv.y < 0.0 || vMapUv.x > 1.0 || vMapUv.y > 1.0) discard;
  float terrainCoverage = texture(uStaticShadowTexture, vMapUv).r;
  vec2 unitUv = (vWorldPosition - uUnitMaskOrigin) / uUnitMaskWorldSize;
  float unitCoverage = unitUv.x >= 0.0 && unitUv.y >= 0.0 && unitUv.x <= 1.0 && unitUv.y <= 1.0
    ? texture(uUnitShadowTexture, unitUv).r
    : 0.0;
  float coverage = max(terrainCoverage, unitCoverage);
  float alpha = coverage * uAlpha;
  if (alpha <= 0.001) discard;
  finalColor = vec4(vec3(0.075, 0.064, 0.052) * alpha, alpha);
}
`;

const UNIT_VERTEX = `#version 300 es
precision highp float;
in vec3 aCorner;
in vec3 aInstanceOrigin;
in vec3 aPartCenter;
in vec3 aPartSize;
in vec2 aPartRotation;
uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
uniform vec2 uMaskOrigin;
uniform vec2 uSunDirection;
uniform float uInverseSunSlope;
uniform float uMaskScale;
uniform sampler2D uTerrainHeightTexture;
uniform vec2 uMapWorldSize;
uniform float uTileSize;
float terrainHeight(vec2 world) {
  vec2 uv = clamp(world / uMapWorldSize, vec2(0.0), vec2(1.0));
  return texture(uTerrainHeightTexture, uv).r * 255.0 * uTileSize;
}
void main(void) {
  float facingCos = cos(aInstanceOrigin.z);
  float facingSin = sin(aInstanceOrigin.z);
  vec2 partCenter = aInstanceOrigin.xy + mat2(facingCos, facingSin, -facingSin, facingCos) * aPartCenter.xy;
  float yaw = aInstanceOrigin.z + aPartRotation.x;
  float yawCos = cos(yaw);
  float yawSin = sin(yaw);
  float pitchCos = cos(aPartRotation.y);
  float pitchSin = sin(aPartRotation.y);
  vec3 local = aCorner * aPartSize;
  float pitchedX = local.x * pitchCos - local.z * pitchSin;
  float pitchedZ = local.x * pitchSin + local.z * pitchCos;
  vec2 world = partCenter + mat2(yawCos, yawSin, -yawSin, yawCos) * vec2(pitchedX, local.y);
  float worldHeight = terrainHeight(aInstanceOrigin.xy) + max(0.0, aPartCenter.z + pitchedZ);
  vec2 projectedWorld = world;
  // Solve the directional ray against the heightfield receiver. Three fixed refinements keep the
  // dynamic cost proportional to box vertices, not viewport pixels, and align silhouettes on
  // slopes without moving gameplay coordinates.
  for (int refinement = 0; refinement < 3; refinement += 1) {
    float receiverHeight = terrainHeight(projectedWorld);
    projectedWorld = world - uSunDirection * max(0.0, worldHeight - receiverHeight) * uInverseSunSlope;
  }
  vec2 maskPosition = (projectedWorld - uMaskOrigin) * uMaskScale;
  mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
  gl_Position = vec4((mvp * vec3(maskPosition, 1.0)).xy, 0.0, 1.0);
}
`;

const UNIT_FRAGMENT = `#version 300 es
precision highp float;
out vec4 finalColor;
void main(void) { finalColor = vec4(1.0); }
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

/** Cached terrain visibility composited with an instanced projected-unit coverage mask. */
export class UnifiedGpuShadowLayer {
  constructor({ pixi = globalThis.PIXI, renderer, layer, recordDiagnostic = null, gpuTimer = null } = {}) {
    this.pixi = pixi;
    this.renderer = renderer;
    this.layer = layer;
    this.recordDiagnostic = recordDiagnostic;
    this.gpuTimer = gpuTimer;
    this.map = null;
    this.enabled = false;
    this.unitShadowsEnabled = false;
    this.unitProjectionSlope = 1;
    this.staticBuildCount = 0;
    this.staticMapBuildCount = 0;
    this.staticBuildDurationMs = 0;
    this.staticCacheWidth = 0;
    this.staticCacheHeight = 0;
    this.supported = Boolean(pixi?.Geometry && pixi?.Mesh && pixi?.Shader && pixi?.UniformGroup
      && pixi?.Buffer && pixi?.RenderTexture && renderer?.render);
    this.projectedEntityIds = new Set();
    if (!this.supported) return;
    this.geometry = new pixi.Geometry({
      attributes: { aPosition: { buffer: new Float32Array(8), format: "float32x2" } },
      indexBuffer: new Uint16Array([0, 1, 2, 0, 2, 3]),
    });
    this.uniforms = new pixi.UniformGroup({
      uMapWorldSize: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      uAlpha: { value: 0.25, type: "f32" },
      uUnitMaskOrigin: { value: new Float32Array([0, 0]), type: "vec2<f32>" },
      uUnitMaskWorldSize: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
    });
    this.terrainHeightTexture = elevationTexture(pixi, { width: 1, height: 1, elevation: [0] });
    this.staticShadowTexture = coverageTexture(pixi, {
      width: 1,
      height: 1,
      data: new Uint8Array([0]),
    });
    this.unitShadowTexture = createUnitMaskTexture(pixi, 1, 1);
    this.instanceCapacity = INITIAL_INSTANCE_CAPACITY;
    this.instanceData = new Float32Array(this.instanceCapacity * INSTANCE_FLOATS);
    this.instanceBuffer = new pixi.Buffer({
      data: this.instanceData,
      usage: pixi.BufferUsage.VERTEX | pixi.BufferUsage.COPY_DST,
      shrinkToFit: false,
      label: "projected-unit-shadow-instances",
    });
    this.unitGeometry = createUnitGeometry(pixi, this.instanceBuffer);
    this.unitUniforms = new pixi.UniformGroup({
      uMaskOrigin: { value: new Float32Array([0, 0]), type: "vec2<f32>" },
      uSunDirection: { value: new Float32Array([0, -1]), type: "vec2<f32>" },
      uInverseSunSlope: { value: 1, type: "f32" },
      uMaskScale: { value: OCCLUDER_TEXTURE_SCALE, type: "f32" },
      uMapWorldSize: { value: new Float32Array([1, 1]), type: "vec2<f32>" },
      uTileSize: { value: 32, type: "f32" },
    });
    this.unitShader = this._createUnitShader();
    this.unitMesh = new pixi.Mesh({ geometry: this.unitGeometry, shader: this.unitShader });
    this.unitMesh.eventMode = "none";
    this.unitMesh.visible = true;
    this.shader = this._createShader();
    this.mesh = new pixi.Mesh({ geometry: this.geometry, shader: this.shader });
    this.mesh.eventMode = "none";
    this.mesh.visible = false;
    layer?.addChild?.(this.mesh);
  }

  setMap(map) {
    if (!this.supported) return;
    this.map = normalizedMap(map);
    this.staticMapBuildCount = 0;
    this.staticBuildDurationMs = 0;
    this.staticCacheWidth = 0;
    this.staticCacheHeight = 0;
    this.enabled = supportsUnifiedGpuShadowPass(this.map);
    this.mesh.visible = false;
    this.projectedEntityIds.clear();
    if (!this.enabled) return;
    const worldWidth = this.map.width * this.map.tileSize;
    const worldHeight = this.map.height * this.map.tileSize;
    const buildStarted = globalThis.performance?.now?.() ?? Date.now();
    const staticMask = buildDirectionalHorizonMask(this.map);
    const staticTexture = coverageTexture(this.pixi, staticMask);
    this.staticBuildDurationMs = (globalThis.performance?.now?.() ?? Date.now()) - buildStarted;
    this.staticBuildCount += 1;
    this.staticMapBuildCount += 1;
    this.staticCacheWidth = staticMask.width;
    this.staticCacheHeight = staticMask.height;
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.staticBuilds", 1);
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.staticBuildMs", this.staticBuildDurationMs);
    const terrainTexture = elevationTexture(this.pixi, this.map);
    const unitTexture = createUnitMaskTexture(this.pixi, 1, 1);
    // This is ordinary coverage rather than encoded height data, so linear filtering is safe and
    // gives the half-resolution mask a stable one-pixel antialiased edge.
    const shader = this._createShader(staticTexture, unitTexture);
    const unitShader = this._createUnitShader(terrainTexture);
    this.mesh.shader = shader;
    this.unitMesh.shader = unitShader;
    this.shader?.destroy?.();
    this.unitShader?.destroy?.();
    this.terrainHeightTexture?.destroy?.(true);
    this.staticShadowTexture?.destroy?.(true);
    this.unitShadowTexture?.destroy?.(true);
    this.shader = shader;
    this.unitShader = unitShader;
    this.terrainHeightTexture = terrainTexture;
    this.staticShadowTexture = staticTexture;
    this.unitShadowTexture = unitTexture;
    const positionBuffer = this.geometry.getAttribute("aPosition").buffer;
    positionBuffer.data.set([0, 0, worldWidth, 0, worldWidth, worldHeight, 0, worldHeight]);
    positionBuffer.update();
    const sun = shadowSunModel(this.map.sun);
    this.unitProjectionSlope = sun.slope;
    this.uniforms.uniforms.uMapWorldSize[0] = worldWidth;
    this.uniforms.uniforms.uMapWorldSize[1] = worldHeight;
    this.unitUniforms.uniforms.uSunDirection[0] = sun.directionX;
    this.unitUniforms.uniforms.uSunDirection[1] = sun.directionY;
    this.unitUniforms.uniforms.uInverseSunSlope = 1 / this.unitProjectionSlope;
    this.unitUniforms.uniforms.uMapWorldSize[0] = worldWidth;
    this.unitUniforms.uniforms.uMapWorldSize[1] = worldHeight;
    this.unitUniforms.uniforms.uTileSize = this.map.tileSize;
    this.mesh.visible = true;
  }

  setUnitShadowsEnabled(enabled) {
    const next = enabled === true;
    if (next === this.unitShadowsEnabled) return;
    this.unitShadowsEnabled = next;
    this.projectedEntityIds.clear();
    if (next || !this.supported || !this.enabled || !this.map) return;
    this.renderer.render({
      container: this.unitMesh,
      target: this.unitShadowTexture,
      clear: true,
      clearColor: [0, 0, 0, 0],
    });
  }

  update(entities, viewport = null) {
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
    const mask = unitMaskBounds(viewport, this.map, OCCLUDER_TEXTURE_SCALE, MASK_FILTER_GUTTER_TEXELS);
    this.unitShadowTexture.resize(mask.widthPx, mask.heightPx, 1);
    this.unitUniforms.uniforms.uMaskOrigin[0] = mask.minX;
    this.unitUniforms.uniforms.uMaskOrigin[1] = mask.minY;
    this.uniforms.uniforms.uUnitMaskOrigin[0] = mask.minX;
    this.uniforms.uniforms.uUnitMaskOrigin[1] = mask.minY;
    this.uniforms.uniforms.uUnitMaskWorldSize[0] = mask.widthWorld;
    this.uniforms.uniforms.uUnitMaskWorldSize[1] = mask.heightWorld;
    this._writeInstances(shapes);
    const drawMask = () => this.renderer.render({
      container: this.unitMesh,
      target: this.unitShadowTexture,
      clear: true,
      clearColor: [0, 0, 0, 0],
    });
    if (this.gpuTimer) this.gpuTimer.measure("renderer.unitShadows.mask", drawMask);
    else drawMask();
    for (const shape of shapes) this.projectedEntityIds.add(shape.entityId);
    this.mesh.visible = true;
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.projectedBoxes", shapes.length);
    this.recordDiagnostic?.("renderer.unifiedGpuShadows.drawCalls", 2);
    return shapes.length;
  }

  _writeInstances(shapes) {
    if (shapes.length > this.instanceCapacity) {
      while (this.instanceCapacity < shapes.length) this.instanceCapacity *= 2;
      this.instanceData = new Float32Array(this.instanceCapacity * INSTANCE_FLOATS);
      this.instanceBuffer.data = this.instanceData;
    }
    let offset = 0;
    for (const shape of shapes) {
      const center = shape.center || [0, 0, 0];
      const size = shape.size || [1, 1, 1];
      this.instanceData.set([
        finite(shape.x, 0), finite(shape.y, 0), finite(shape.facing, 0),
        finite(center[0], 0), finite(center[1], 0), finite(center[2], 0),
        finite(size[0], 1), finite(size[1], 1), finite(size[2], 1),
        finite(shape.yaw, 0), finite(shape.pitch, 0),
      ], offset);
      offset += INSTANCE_FLOATS;
    }
    this.unitGeometry.instanceCount = shapes.length;
    this.instanceBuffer.setDataWithSize(this.instanceData, offset, true);
  }

  hasShadowFor(entityId) { return this.projectedEntityIds.has(entityId); }

  staticTerrainSummary() {
    return Object.freeze({
      buildCount: this.staticMapBuildCount,
      lifetimeBuildCount: this.staticBuildCount,
      buildMs: Math.round(this.staticBuildDurationMs * 1000) / 1000,
      width: this.staticCacheWidth,
      height: this.staticCacheHeight,
      samplesPerTile: STATIC_SHADOW_SAMPLES_PER_TILE,
    });
  }

  _createShader(terrain = this.staticShadowTexture, units = this.unitShadowTexture) {
    return this.pixi.Shader.from({
      gl: { vertex: VERTEX, fragment: FRAGMENT, name: "cached-unified-height-field-shadows" },
      resources: { shadowUniforms: this.uniforms, uStaticShadowTexture: terrain.source, uUnitShadowTexture: units.source },
    });
  }

  _createUnitShader(terrain = this.terrainHeightTexture) {
    return this.pixi.Shader.from({
      gl: { vertex: UNIT_VERTEX, fragment: UNIT_FRAGMENT, name: "instanced-projected-unit-shadows" },
      resources: { unitShadowUniforms: this.unitUniforms, uTerrainHeightTexture: terrain.source },
    });
  }

  destroy() {
    this.mesh?.parent?.removeChild?.(this.mesh);
    this.mesh?.destroy?.();
    this.geometry?.destroy?.(true);
    this.shader?.destroy?.();
    this.terrainHeightTexture?.destroy?.(true);
    this.staticShadowTexture?.destroy?.(true);
    this.unitShadowTexture?.destroy?.(true);
    this.unitMesh?.destroy?.();
    this.unitGeometry?.destroy?.(true);
    this.unitShader?.destroy?.();
    this.projectedEntityIds.clear();
    this.gpuTimer = null;
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
  const inverseSlope = 1 / Math.max(0.001, finite(sunSlope, 1));
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

export function unitMaskBounds(viewport, map, scale = OCCLUDER_TEXTURE_SCALE, gutterTexels = MASK_FILTER_GUTTER_TEXELS) {
  const safeScale = Math.max(0.01, finite(scale, OCCLUDER_TEXTURE_SCALE));
  const zoom = Math.max(0.01, finite(viewport?.zoom, 1));
  const mapWidth = Math.max(1, finite(map?.width, 1) * finite(map?.tileSize, 32));
  const mapHeight = Math.max(1, finite(map?.height, 1) * finite(map?.tileSize, 32));
  const rawLeft = finite(viewport?.x, 0);
  const rawTop = finite(viewport?.y, 0);
  const rawRight = rawLeft + Math.max(1, finite(viewport?.viewportWidth, mapWidth)) / zoom;
  const rawBottom = rawTop + Math.max(1, finite(viewport?.viewportHeight, mapHeight)) / zoom;
  const left = Math.min(mapWidth, Math.max(0, rawLeft));
  const top = Math.min(mapHeight, Math.max(0, rawTop));
  const right = Math.min(mapWidth, Math.max(left, rawRight));
  const bottom = Math.min(mapHeight, Math.max(top, rawBottom));
  const gutter = Math.max(0, Math.trunc(finite(gutterTexels, MASK_FILTER_GUTTER_TEXELS)));
  const minPixelX = Math.max(0, Math.floor(left * safeScale) - gutter);
  const minPixelY = Math.max(0, Math.floor(top * safeScale) - gutter);
  const maxPixelX = Math.min(Math.ceil(mapWidth * safeScale), Math.ceil(right * safeScale) + gutter);
  const maxPixelY = Math.min(Math.ceil(mapHeight * safeScale), Math.ceil(bottom * safeScale) + gutter);
  const widthPx = Math.max(1, maxPixelX - minPixelX);
  const heightPx = Math.max(1, maxPixelY - minPixelY);
  return Object.freeze({
    minX: minPixelX / safeScale,
    minY: minPixelY / safeScale,
    widthPx,
    heightPx,
    widthWorld: widthPx / safeScale,
    heightWorld: heightPx / safeScale,
  });
}

/** The single authored directional-light model shared by static terrain and dynamic receivers. */
export function shadowSunModel(sun) {
  const azimuth = finite(sun?.azimuthDegrees, 0) * Math.PI / 180;
  const elevation = finite(sun?.elevationDegrees, 45) * Math.PI / 180;
  return Object.freeze({
    directionX: Math.sin(azimuth),
    directionY: -Math.cos(azimuth),
    // The tiny floor is only a division guard below 0.057 degrees, not a presentation clamp.
    slope: Math.max(0.001, Math.tan(elevation)),
  });
}

/**
 * Build a light-space visibility cache in O(map pixels).
 *
 * Along one directional-light ray q=dot(world,sun), a blocker wins exactly when its transformed
 * height (z - slope*q) exceeds the receiver's. Sweeping from the sun-facing map edge therefore
 * carries the same maximum a conventional orthographic directional depth map would compare. The
 * non-dominant coordinate lands between samples; linear interpolation follows that ray with an
 * angular rasterization error bounded to one cache texel (1/4 tile by default).
 */
export function buildDirectionalHorizonMask(map, samplesPerTile = STATIC_SHADOW_SAMPLES_PER_TILE) {
  const normalized = normalizedMap(map);
  const samples = Math.max(1, Math.min(8, Math.trunc(finite(samplesPerTile, STATIC_SHADOW_SAMPLES_PER_TILE))));
  const width = normalized.width * samples;
  const height = normalized.height * samples;
  const horizon = new Float32Array(width * height);
  horizon.fill(Number.NEGATIVE_INFINITY);
  const data = new Uint8Array(width * height);
  const sun = shadowSunModel(normalized.sun);
  const sampleWorld = normalized.tileSize / samples;
  const penumbraWorld = normalized.tileSize * (normalized.sun?.elevationDegrees <= 20 ? 0.12 : 0.08);
  const lightHeightAt = (x, y) => {
    const worldX = (x + 0.5) * sampleWorld;
    const worldY = (y + 0.5) * sampleWorld;
    const elevation = bilinearElevationAtWorld(normalized, worldX, worldY) * normalized.tileSize;
    return elevation - sun.slope * (worldX * sun.directionX + worldY * sun.directionY);
  };
  const write = (x, y, upstream) => {
    const index = y * width + x;
    const receiver = lightHeightAt(x, y);
    const clearance = upstream - receiver;
    data[index] = Number.isFinite(upstream)
      ? Math.round(255 * smoothstep(0, Math.max(0.001, penumbraWorld), clearance))
      : 0;
    horizon[index] = Math.max(receiver, upstream);
  };
  const upstreamAt = (x, y) => {
    const low = Math.floor(y);
    const high = Math.ceil(y);
    if (x < 0 || x >= width) return Number.NEGATIVE_INFINITY;
    const lowValue = low >= 0 && low < height ? horizon[low * width + x] : Number.NEGATIVE_INFINITY;
    const highValue = high >= 0 && high < height ? horizon[high * width + x] : Number.NEGATIVE_INFINITY;
    return interpolateFinite(lowValue, highValue, y - low);
  };
  const upstreamAtTransposed = (x, y) => {
    const low = Math.floor(x);
    const high = Math.ceil(x);
    if (y < 0 || y >= height) return Number.NEGATIVE_INFINITY;
    const lowValue = low >= 0 && low < width ? horizon[y * width + low] : Number.NEGATIVE_INFINITY;
    const highValue = high >= 0 && high < width ? horizon[y * width + high] : Number.NEGATIVE_INFINITY;
    return interpolateFinite(lowValue, highValue, x - low);
  };

  if (Math.abs(sun.directionX) >= Math.abs(sun.directionY)) {
    const stepX = sun.directionX >= 0 ? -1 : 1;
    const startX = sun.directionX >= 0 ? width - 1 : 0;
    const secondaryStep = sun.directionY / Math.max(0.001, Math.abs(sun.directionX));
    for (let x = startX; x >= 0 && x < width; x += stepX) {
      const upstreamX = x - stepX;
      for (let y = 0; y < height; y += 1) write(x, y, upstreamAt(upstreamX, y + secondaryStep));
    }
  } else {
    const stepY = sun.directionY >= 0 ? -1 : 1;
    const startY = sun.directionY >= 0 ? height - 1 : 0;
    const secondaryStep = sun.directionX / Math.max(0.001, Math.abs(sun.directionY));
    for (let y = startY; y >= 0 && y < height; y += stepY) {
      const upstreamY = y - stepY;
      for (let x = 0; x < width; x += 1) write(x, y, upstreamAtTransposed(x + secondaryStep, upstreamY));
    }
  }
  return Object.freeze({ width, height, data, samplesPerTile: samples });
}

function interpolateFinite(low, high, amount) {
  if (!Number.isFinite(low)) return high;
  if (!Number.isFinite(high)) return low;
  return low * (1 - amount) + high * amount;
}

function coverageTexture(pixi, mask) {
  const canvas = createWorkerSafeCanvas();
  canvas.width = mask.width;
  canvas.height = mask.height;
  const ctx = canvas.getContext("2d", { alpha: false });
  const image = ctx.createImageData(mask.width, mask.height);
  for (let index = 0; index < mask.data.length; index += 1) {
    const value = mask.data[index];
    const offset = index * 4;
    image.data[offset] = value;
    image.data[offset + 1] = value;
    image.data[offset + 2] = value;
    image.data[offset + 3] = 255;
  }
  ctx.putImageData(image, 0, 0);
  const texture = pixi.Texture.from(canvas);
  texture.source.scaleMode = "linear";
  return texture;
}

function createUnitMaskTexture(pixi, width, height) {
  const texture = pixi.RenderTexture.create({
    width: Math.max(1, width),
    height: Math.max(1, height),
    resolution: 1,
    dynamic: true,
    format: "r8unorm",
  });
  texture.source.scaleMode = "linear";
  return texture;
}

function createUnitGeometry(pixi, instanceBuffer) {
  const corners = new Float32Array([
    // bottom
    -.5,-.5,-.5, .5,-.5,-.5, .5,.5,-.5, -.5,-.5,-.5, .5,.5,-.5, -.5,.5,-.5,
    // top
    -.5,-.5,.5, .5,.5,.5, .5,-.5,.5, -.5,-.5,.5, -.5,.5,.5, .5,.5,.5,
    // front/back
    -.5,-.5,-.5, .5,-.5,.5, .5,-.5,-.5, -.5,-.5,-.5, -.5,-.5,.5, .5,-.5,.5,
    -.5,.5,-.5, .5,.5,-.5, .5,.5,.5, -.5,.5,-.5, .5,.5,.5, -.5,.5,.5,
    // left/right
    -.5,-.5,-.5, -.5,.5,-.5, -.5,.5,.5, -.5,-.5,-.5, -.5,.5,.5, -.5,-.5,.5,
    .5,-.5,-.5, .5,-.5,.5, .5,.5,.5, .5,-.5,-.5, .5,.5,.5, .5,.5,-.5,
  ]);
  return new pixi.Geometry({
    attributes: {
      aCorner: { buffer: corners, format: "float32x3" },
      aInstanceOrigin: { buffer: instanceBuffer, format: "float32x3", stride: INSTANCE_STRIDE_BYTES, offset: 0, instance: true },
      aPartCenter: { buffer: instanceBuffer, format: "float32x3", stride: INSTANCE_STRIDE_BYTES, offset: 3 * 4, instance: true },
      aPartSize: { buffer: instanceBuffer, format: "float32x3", stride: INSTANCE_STRIDE_BYTES, offset: 6 * 4, instance: true },
      aPartRotation: { buffer: instanceBuffer, format: "float32x2", stride: INSTANCE_STRIDE_BYTES, offset: 9 * 4, instance: true },
    },
    instanceCount: 0,
  });
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

function bilinearElevationAtWorld(map, worldX, worldY) {
  const tileX = worldX / map.tileSize - 0.5;
  const tileY = worldY / map.tileSize - 0.5;
  const x0 = Math.floor(tileX);
  const y0 = Math.floor(tileY);
  const fx = tileX - x0;
  const fy = tileY - y0;
  const at = (x, y) => {
    const clampedX = Math.max(0, Math.min(map.width - 1, x));
    const clampedY = Math.max(0, Math.min(map.height - 1, y));
    return finite(map.elevation[clampedY * map.width + clampedX], 0);
  };
  const top = at(x0, y0) * (1 - fx) + at(x0 + 1, y0) * fx;
  const bottom = at(x0, y0 + 1) * (1 - fx) + at(x0 + 1, y0 + 1) * fx;
  return top * (1 - fy) + bottom * fy;
}

function smoothstep(edge0, edge1, value) {
  const amount = Math.max(0, Math.min(1, (value - edge0) / Math.max(0.000001, edge1 - edge0)));
  return amount * amount * (3 - 2 * amount);
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
