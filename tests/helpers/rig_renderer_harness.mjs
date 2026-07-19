import {
  _drawUnit,
  _rigRenderContextFor,
} from "../../client/src/renderer/units.js";
import {
  createInspectionPngPixiFactory,
  FakeContainer,
  FakeGraphics,
} from "./rig_inspection_pixi.mjs";

export function makeRigRenderer() {
  return {
    _liveRigDefinitionsByKind: new Map(),
    _liveFrameStripsByKind: new Map(),
    _liveFrameStripTextures: new Map(),
    _liveRigPools: {
      liveUnitRigShadows: new Map(),
      liveUnitRigs: new Map(),
      liveUnitRigOverlays: new Map(),
      liveUnitRigEffects: new Map(),
      liveShotRevealRigShadows: new Map(),
      liveShotRevealRigs: new Map(),
      liveShotRevealRigOverlays: new Map(),
      liveShotRevealRigEffects: new Map(),
    },
    _liveRigRoutes: {
      liveUnitRigShadows: { poolName: "liveUnitRigShadows", layerName: "unitShadows" },
      liveUnitRigs: { poolName: "liveUnitRigs", layerName: "units" },
      liveUnitRigOverlays: { poolName: "liveUnitRigOverlays", layerName: "units" },
      liveUnitRigEffects: { poolName: "liveUnitRigEffects", layerName: "units" },
      liveShotRevealRigShadows: { poolName: "liveShotRevealRigShadows", layerName: "shotRevealShadows" },
      liveShotRevealRigs: { poolName: "liveShotRevealRigs", layerName: "shotReveals" },
      liveShotRevealRigOverlays: { poolName: "liveShotRevealRigOverlays", layerName: "shotReveals" },
      liveShotRevealRigEffects: { poolName: "liveShotRevealRigEffects", layerName: "shotReveals" },
    },
    _rigPixiFactory: createInspectionPngPixiFactory(),
    _pools: { unitShadows: new Map(), units: new Map(), shotRevealShadows: new Map(), shotReveals: new Map() },
    _seen: {
      unitShadows: new Set(),
      units: new Set(),
      shotRevealShadows: new Set(),
      shotReveals: new Set(),
      liveUnitRigShadows: new Set(),
      liveUnitRigs: new Set(),
      liveUnitRigOverlays: new Set(),
      liveUnitRigEffects: new Set(),
      liveShotRevealRigShadows: new Set(),
      liveShotRevealRigs: new Set(),
      liveShotRevealRigOverlays: new Set(),
      liveShotRevealRigEffects: new Set(),
    },
    layers: {
      unitShadows: new FakeContainer(),
      units: new FakeContainer(),
      shotRevealShadows: new FakeContainer(),
      shotReveals: new FakeContainer(),
    },
    _drawUnit(entity, colorByOwner, state, pools = {}) {
      return _drawUnit.call(this, entity, colorByOwner, state, pools);
    },
    _slot(poolName, id) {
      const pool = this._pools[poolName];
      let graphic = pool.get(id);
      if (!graphic) {
        graphic = new FakeGraphics();
        pool.set(id, graphic);
        this.layers[poolName].addChild(graphic);
      }
      this._seen[poolName].add(id);
      graphic.visible = true;
      graphic.alpha = 1;
      graphic.clear();
      return graphic;
    },
    _shadow(g, cx, cy, radius) {
      g.ellipse(cx, cy + radius * 0.35, radius, radius * 0.6).fill({ color: 0x000000, alpha: 0.28 });
    },
    _vehicleShadow() {
      throw new Error("worker comparison test should not draw vehicle shadow");
    },
    _tintFor(owner, colorByOwner) {
      return colorByOwner.get(owner) ?? 0x9aa0a8;
    },
    _rigRenderContextFor(entity, colorByOwner, state) {
      return _rigRenderContextFor.call(this, entity, colorByOwner, state);
    },
    _deployedWeaponSetupVisual: () => ({ prongFactor: 0, barrel: false }),
    _tankMotionVisual: () => ({ activity: 0 }),
    _map: { tileSize: 32 },
  };
}

export function fakeAtlasTexture() {
  return { source: { id: "fake-tank-atlas" } };
}

export function fakeFrameStripTexture() {
  return { source: { id: "fake-rifleman-strip" } };
}
