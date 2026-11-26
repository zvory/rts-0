import assert from "node:assert/strict";
import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import { KIND, SETUP } from "../../client/src/protocol.js";
import { buildSelectionScene, pickSelectionProxy, selectionProxiesInScreenRect } from "../../client/src/input/selection_projection.js";

const projection = (zoom = 1) => createOrthographicProjectionSnapshot({
  x: 0, y: 0, zoom, worldW: 1024, worldH: 1024, viewW: 800, viewH: 800,
}, 800 / zoom);
const unit = (kind, extras = {}) => ({ id: 1, owner: 2, kind, x: 100, y: 100, ...extras });
const scene = (entities, zoom = 1) => buildSelectionScene({ entities, projection: projection(zoom) });
function edge(kind, inside, outside, extras = {}) {
  const s = scene([unit(kind, extras)]);
  assert.equal(pickSelectionProxy(s, { x: 100 + inside, y: 100 })?.id, 1, `${kind} inside reviewed radius`);
  assert.equal(pickSelectionProxy(s, { x: 100 + outside, y: 100 }), null, `${kind} outside reviewed radius`);
}
for (const kind of [KIND.WORKER, KIND.RIFLEMAN, KIND.PANZERFAUST]) edge(kind, 13.4, 13.6);
edge(KIND.MACHINE_GUNNER, 14.9, 15.1);
for (const kind of [KIND.WARRIOR, KIND.WIND_ADEPT, KIND.MAGIC_ADEPT]) edge(kind, 19.4, 19.6);
edge(KIND.WAR_ADEPT, 22.7, 22.8);
edge(KIND.WAR_ADEPT, 32.7, 32.8, { daoMaster: true });
for (const kind of [KIND.WIND_ADEPT, KIND.MAGIC_ADEPT]) edge(kind, 31.1, 31.3, { daoMaster: true });
edge(KIND.MORTAR_TEAM, 21.5, 21.7);

// Minimum screen tolerance scales as well, even where the world footprint is tiny.
const zoomedOut = scene([unit(KIND.WORKER)], 0.1);
assert.equal(pickSelectionProxy(zoomedOut, { x: 18.9, y: 10 })?.id, 1);
assert.equal(pickSelectionProxy(zoomedOut, { x: 19.1, y: 10 }), null);
const noviceProxy = scene([unit(KIND.WARRIOR)]).proxies[0];
assert.equal(noviceProxy.footprint.radiusPx, 17, "Novice collision/marquee footprint is 17 px");
assert.equal(noviceProxy.directClickFootprint.radiusPx, 19.5, "Novice direct click area remains 19.5 px");
const smallNovice = scene([unit(KIND.WARRIOR)], 0.1);
assert.equal(pickSelectionProxy(smallNovice, { x: 14.4, y: 10 })?.id, 1);
assert.equal(pickSelectionProxy(smallNovice, { x: 14.6, y: 10 }), null);

for (const [kind, inside, outside, side] of [
  [KIND.TANK, 29.3, 29.4, 15.9],
  [KIND.ROCKET_LAUNCHER, 27.7, 27.8, 14.4],
]) {
  edge(kind, inside, outside);
  const s = scene([unit(kind, { facing: Math.PI / 2 })]);
  assert.equal(pickSelectionProxy(s, { x: 100, y: 100 + inside })?.id, 1);
  assert.equal(pickSelectionProxy(s, { x: 100, y: 100 - inside })?.id, 1);
  assert.equal(pickSelectionProxy(s, { x: 100, y: 100 + outside }), null);
  assert.equal(pickSelectionProxy(s, { x: 100 + side + 0.1, y: 100 }), null, "vehicle width does not grow");
}
const packed = scene([unit(KIND.ANTI_TANK_GUN, { setupState: SETUP.PACKED })]);
assert.equal(pickSelectionProxy(packed, { x: 56.1, y: 100 })?.id, 1, "rear trails are clickable");
assert.equal(pickSelectionProxy(packed, { x: 123.1, y: 100 }), null, "no excess forward expansion");
assert.equal(pickSelectionProxy(packed, { x: 100, y: 114.1 }), null, "packed box fits narrow carriage");
for (const setupState of [SETUP.DEPLOYED, SETUP.SETTING_UP, SETUP.TEARING_DOWN]) {
  const s = scene([unit(KIND.ANTI_TANK_GUN, { setupState })]);
  assert.equal(pickSelectionProxy(s, { x: 57, y: 122 })?.id, 1, "deployed and transitional trails fit");
  assert.equal(pickSelectionProxy(s, { x: 100, y: 123.1 }), null);
}
const turned = scene([unit(KIND.ANTI_TANK_GUN, { setupState: SETUP.PACKED, facing: Math.PI / 2, weaponFacing: 0 })]);
assert.equal(pickSelectionProxy(turned, { x: 100, y: 56.1 })?.id, 1, "packed sprite box follows hull facing despite stale weapon facing");
assert.equal(pickSelectionProxy(turned, { x: 114.1, y: 100 }), null);
const aimed = scene([unit(KIND.ANTI_TANK_GUN, { setupState: SETUP.DEPLOYED, facing: 0, weaponFacing: Math.PI / 2 })]);
assert.equal(pickSelectionProxy(aimed, { x: 100, y: 56.1 })?.id, 1, "deployed sprite box follows weapon facing");
assert.equal(pickSelectionProxy(aimed, { x: 56.1, y: 100 }), null);
// Both ends alone miss the corners swept through intermediate setup poses.
for (const setupState of [SETUP.SETTING_UP, SETUP.TEARING_DOWN]) {
  for (const [facing, delta] of [[0, Math.PI / 2], [0.4, -Math.PI / 2], [3, 0.8]]) {
    const weaponFacing = (facing + delta + Math.PI) % (Math.PI * 2) - Math.PI;
    const s = scene([unit(KIND.ANTI_TANK_GUN, { setupState, facing, weaponFacing })]);
    for (let step = 0; step <= 12; step += 1) {
      const angle = facing + delta * step / 12;
      for (const [x, y] of [[-43.9, -22.9], [22.9, -22.9], [22.9, 22.9], [-43.9, 22.9]]) {
        assert.equal(pickSelectionProxy(s, {
          x: 100 + x * Math.cos(angle) - y * Math.sin(angle),
          y: 100 + x * Math.sin(angle) + y * Math.cos(angle),
        })?.id, 1, "transition box contains each intermediate rotated corner, including wrapped angles");
      }
    }
  }
}

// A direct hit beats a closer halo and even a halo with own-unit preference.
const clump = scene([
  unit(KIND.RIFLEMAN, { id: 1, owner: 1 }),
  unit(KIND.WAR_ADEPT, { id: 2, x: 130 }),
]);
assert.equal(pickSelectionProxy(clump, { x: 112, y: 100 }, {
  preference: p => p.owner === 1 ? 1 : 0,
})?.id, 2);
// Boundary distance wins between halos, rather than distance to unit centers.
const halos = scene([
  unit(KIND.RIFLEMAN, { id: 1 }),
  unit(KIND.WAR_ADEPT, { id: 2, x: 132 }),
]);
assert.equal(pickSelectionProxy(halos, { x: 112.5, y: 100 })?.id, 2);
assert.equal(pickSelectionProxy(halos, { x: 112.5, y: 100 }, { eligible: p => p.id === 1 })?.id, 1);
const ties = scene([unit(KIND.RIFLEMAN, { id: 2 }), unit(KIND.RIFLEMAN, { id: 1 })]);
assert.equal(pickSelectionProxy(ties, { x: 112, y: 100 })?.id, 1);
// Direct building hits cannot be stolen by a nearby unit's halo.
const building = scene([unit(KIND.RIFLEMAN), unit(KIND.DEPOT, { id: 2, x: 140 })]);
assert.equal(pickSelectionProxy(building, { x: 112, y: 100 })?.id, 2);
// Click geometry never changes marquee bodies or admits reveal-only entities.
assert.deepEqual(selectionProxiesInScreenRect(scene([unit(KIND.RIFLEMAN)]), {
  x0: 112, y0: 99, x1: 113, y1: 101,
}), []);
assert.equal(scene([unit(KIND.RIFLEMAN, { shotReveal: true })]).proxies.length, 0);
assert.equal(scene([unit(KIND.RIFLEMAN, { visionOnly: true })]).proxies.length, 0);
// No fallback anchor hit may survive a depth-clipped expanded footprint.
const clippedProjection = { version: 1, viewport: { widthCssPx: 400, heightCssPx: 400 },
  project: p => ({ x: p.x, y: p.y, depth: 1, clip: p.x > 112 ? "outsideDepth" : "inside" }),
};
const clipped = buildSelectionScene({ entities: [unit(KIND.RIFLEMAN)], projection: clippedProjection });
assert.equal(pickSelectionProxy(clipped, { x: 100, y: 100 }), null);
console.log("unit_click_geometry_contracts: ok");
