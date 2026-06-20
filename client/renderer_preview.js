/**
 * Renderer Preview — standalone visual sandbox.
 *
 * Mounts the real Renderer with a synthetic map, one of every building and unit
 * kind laid out in two rows.  No server connection needed; just reload after
 * editing any renderer or rig file.
 *
 * Animations shown:
 *   - Slow facing + weaponFacing rotation on all units
 *   - Staggered periodic weapon recoil (barrel kick + prong kick)
 *   - Setup/teardown cycle on deployed weapons (anti-tank gun, mortar, artillery)
 *   - Worker busy chevron
 *
 * Controls: zoom slider · team color picker · animate toggle
 */

import { Renderer } from "./src/renderer/index.js";
import { KIND, SETUP, STATE } from "./src/protocol.js";
import { PLAYER_PALETTE, STATS } from "./src/config.js";

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const TILE_SIZE = 32;
const MAP_W     = 44;
const MAP_H     = 14;

const BUILDINGS_Y = 120;
const UNITS_Y     = 300;

// World-space centre of the whole layout (used for camera).
const WORLD_CX = 680;
const WORLD_CY = 210;

// ---------------------------------------------------------------------------
// Building and unit kinds in display order
// ---------------------------------------------------------------------------

const BUILDING_KINDS = [
  KIND.CITY_CENTRE,
  KIND.ZAMOK,
  KIND.DEPOT,
  KIND.BARRACKS,
  KIND.TRAINING_CENTRE,
  KIND.RESEARCH_COMPLEX,
  KIND.FACTORY,
  KIND.STEELWORKS,
];

const UNIT_KINDS = [
  KIND.WORKER,
  KIND.RIFLEMAN,
  KIND.MACHINE_GUNNER,
  KIND.ANTI_TANK_GUN,
  KIND.MORTAR_TEAM,
  KIND.ARTILLERY,
  KIND.SCOUT_CAR,
  KIND.TANK,
  KIND.COMMAND_CAR,
  KIND.EKAT,
];

// Kinds that have a setup/deploy cycle and a visible barrel when deployed.
const DEPLOYED_WEAPON_KINDS = new Set([KIND.ANTI_TANK_GUN, KIND.MORTAR_TEAM, KIND.ARTILLERY]);

// ---------------------------------------------------------------------------
// Entity layout
// ---------------------------------------------------------------------------

function buildingX() {
  const positions = [];
  let x = 100;
  for (const kind of BUILDING_KINDS) {
    const stat = STATS[kind] || {};
    const fw = (stat.footW || 2) * TILE_SIZE;
    positions.push(x);
    x += Math.max(fw, TILE_SIZE * 2) + 64;
  }
  return positions;
}

function unitX() {
  const positions = [];
  let x = 80;
  for (const _kind of UNIT_KINDS) {
    positions.push(x);
    x += 120;
  }
  return positions;
}

const BUILDING_X = buildingX();
const UNIT_X     = unitX();

// ---------------------------------------------------------------------------
// Recoil animation
// Mirrors the curve and per-kind timings from state.js / palette.js so the
// preview drives the same visual as a live game.
// ---------------------------------------------------------------------------

const WEAPON_RECOIL_MS = Object.freeze({
  [KIND.RIFLEMAN]:      420,
  [KIND.MACHINE_GUNNER]: 160,
  [KIND.ANTI_TANK_GUN]: 820,
  [KIND.MORTAR_TEAM]:   520,
  [KIND.ARTILLERY]:     980,
  [KIND.SCOUT_CAR]:     160,
  [KIND.TANK]:          650,
});

// How often each unit fires (ms). Longer than the longest recoil so there is
// a clear pause between shots.
const FIRE_INTERVAL_MS = 3200;

function recoilCurve(t) {
  const p = t < 0 ? 0 : t > 1 ? 1 : t;
  if (p < 0.18) return 1 - p * 0.12;
  const settle = (p - 0.18) / 0.82;
  return Math.cos(settle * Math.PI * 0.5) * 0.88;
}

/**
 * Returns 0..1 recoil progress for a unit, staggered so each unit fires at a
 * different time within the shared cycle.
 */
function previewWeaponRecoil(entityIndex, kind, now) {
  const recoilMs = WEAPON_RECOIL_MS[kind];
  if (!recoilMs) return 0;

  const phaseOffset = entityIndex * (FIRE_INTERVAL_MS / UNIT_KINDS.length);
  const cycleTime   = (now + phaseOffset) % FIRE_INTERVAL_MS;

  if (cycleTime > recoilMs) return 0;
  return recoilCurve(cycleTime / recoilMs);
}

// ---------------------------------------------------------------------------
// Deploy cycle
// Deployed weapons alternate between DEPLOYED and PACKED so the setup /
// teardown animation is visible in the preview.
// ---------------------------------------------------------------------------

const DEPLOY_CYCLE_MS  = 5000; // total period: half deployed, half packed
const DEPLOY_PHASE_GAP = 1200; // stagger between the three deployed kinds

function deploySetupState(entityIndex, now) {
  const phase    = entityIndex * DEPLOY_PHASE_GAP;
  const cyclePos = (now + phase) % DEPLOY_CYCLE_MS;
  return cyclePos < DEPLOY_CYCLE_MS / 2 ? SETUP.DEPLOYED : SETUP.PACKED;
}

// ---------------------------------------------------------------------------
// Entity factories
// ---------------------------------------------------------------------------

let _entities = null;   // built once, mutated in place each frame
let _unitStart = 0;     // index into _entities where units begin

function buildEntities() {
  const out = [];
  let id = 1;

  for (let i = 0; i < BUILDING_KINDS.length; i++) {
    out.push({
      id: id++,
      kind: BUILDING_KINDS[i],
      x: BUILDING_X[i],
      y: BUILDINGS_Y,
      owner: 1,
      hp: 500,
      maxHp: 500,
      buildProgress: 1,
    });
  }

  _unitStart = out.length;

  for (let i = 0; i < UNIT_KINDS.length; i++) {
    const kind = UNIT_KINDS[i];
    out.push({
      id: id++,
      kind,
      x: UNIT_X[i],
      y: UNITS_Y,
      owner: 1,
      hp: 100,
      maxHp: 100,
      facing: 0,
      weaponFacing: 0,
      state: STATE.ATTACK,   // non-MOVE so deployed weapons show their barrel
      setupState: DEPLOYED_WEAPON_KINDS.has(kind) ? SETUP.DEPLOYED : SETUP.PACKED,
      shotReveal: false,
      recoilProgress: 0,
      // Worker busy chevron (always on in preview so the indicator is visible)
      latchedNode: kind === KIND.WORKER ? 1 : undefined,
    });
  }

  return out;
}

// ---------------------------------------------------------------------------
// Per-frame entity mutation
// Updates facing, weaponFacing, setupState, and recoil index in place.
// ---------------------------------------------------------------------------

// Map: entity id → unit index (for recoil staggering)
const _unitIndex = new Map();

function tickEntities(now, t) {
  let unitIdx = 0;
  for (let i = _unitStart; i < _entities.length; i++) {
    const e = _entities[i];

    // Cache the per-unit index for the recoil helper.
    if (!_unitIndex.has(e.id)) _unitIndex.set(e.id, unitIdx);

    e.facing       = t * 0.5;
    e.weaponFacing = t * 0.8;

    if (DEPLOYED_WEAPON_KINDS.has(e.kind)) {
      e.setupState = deploySetupState(unitIdx, now);
      // During teardown the barrel should also hide — packed state handles this
      // via the barrel visibility animation on the rig.
    }

    unitIdx++;
  }
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

function makeMockMap() {
  return {
    width: MAP_W,
    height: MAP_H,
    tileSize: TILE_SIZE,
    terrain: new Array(MAP_W * MAP_H).fill(0),
    resources: [],
  };
}

const mockFog = Object.freeze({
  width: MAP_W,
  height: MAP_H,
  isVisible:  () => true,
  isExplored: () => true,
});

function makeMockState(entities, paletteIndex, now) {
  // Build a lookup from entity id → unit index for the recoil function.
  return {
    entitiesInterpolated: () => entities,
    selection: new Set(),
    playerId: 1,
    players: [{ id: 1, color: PLAYER_PALETTE[paletteIndex % PLAYER_PALETTE.length] }],
    resources: { steel: 999, oil: 99, supplyUsed: 0, supplyCap: 20 },
    rememberedBuildings: [],
    map: { width: MAP_W, height: MAP_H, tileSize: TILE_SIZE, resources: [] },

    /**
     * Weapon recoil: mirrors the real state.weaponRecoil() signature.
     * Staggered so units fire at different points in the cycle.
     */
    weaponRecoil(id, kind, _now) {
      const idx = _unitIndex.get(id) ?? 0;
      return previewWeaponRecoil(idx, kind, _now ?? now);
    },
  };
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

function makeCamera(zoom, canvasW, canvasH) {
  return {
    x: WORLD_CX - (canvasW / 2) / zoom,
    y: WORLD_CY - (canvasH / 2) / zoom,
    zoom,
  };
}

// ---------------------------------------------------------------------------
// Label overlay (Canvas2D on top of the PixiJS canvas)
// ---------------------------------------------------------------------------

const labelCanvas = document.getElementById("preview-labels");
const lctx = labelCanvas.getContext("2d");

function resizeLabels() {
  const dpr = window.devicePixelRatio || 1;
  labelCanvas.width  = Math.round(window.innerWidth  * dpr);
  labelCanvas.height = Math.round(window.innerHeight * dpr);
  lctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

function drawLabels(entities, camera) {
  lctx.clearRect(0, 0, labelCanvas.width, labelCanvas.height);
  lctx.font = "11px monospace";
  lctx.textAlign = "center";

  for (const e of entities) {
    const stat      = STATS[e.kind] || {};
    const label     = stat.label || e.kind;
    const isBuilding = BUILDING_KINDS.includes(e.kind);

    const sx = (e.x - camera.x) * camera.zoom;
    const fh = isBuilding
      ? ((stat.footH || 2) * TILE_SIZE * camera.zoom) / 2
      : 0;
    const sy = (e.y - camera.y) * camera.zoom + (isBuilding ? fh : 36) + 10;

    lctx.fillStyle = "rgba(0,0,0,0.7)";
    lctx.fillText(label, sx + 1, sy + 1);
    lctx.fillStyle = "#d8d0b0";
    lctx.fillText(label, sx, sy);
  }
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

const mount = document.getElementById("preview-mount");
const renderer = new Renderer(mount);

renderer.buildStaticMap(makeMockMap());
resizeLabels();

_entities = buildEntities();

// ---------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------

let zoom         = 1.5;
let paletteIndex = 0;
let animate      = true;

document.getElementById("zoom-slider").addEventListener("input",  (e) => { zoom = Number(e.target.value); });
document.getElementById("team-select").addEventListener("change", (e) => { paletteIndex = Number(e.target.value); });
document.getElementById("animate-toggle").addEventListener("change", (e) => { animate = e.target.checked; });

window.addEventListener("resize", () => {
  renderer.resize(window.innerWidth, window.innerHeight);
  resizeLabels();
});

// ---------------------------------------------------------------------------
// Render loop
// ---------------------------------------------------------------------------

let startedAt = performance.now();

function tick(now) {
  const t      = animate ? (now - startedAt) / 1000 : 0;
  const camera = makeCamera(zoom, window.innerWidth, window.innerHeight);
  const state  = makeMockState(_entities, paletteIndex, now);

  tickEntities(now, t);

  renderer.render(state, camera, mockFog, 1);
  drawLabels(_entities, camera);

  requestAnimationFrame(tick);
}

requestAnimationFrame(tick);
