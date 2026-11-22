import assert from "node:assert/strict";
import { EVENT, KIND, WEAPON_KIND } from "../../client/src/protocol.js";
import { VisualEffectBuffers } from "../../client/src/state_visual_effects.js";
import { weaponRecoilDurationMs } from "../../client/src/weapon_recoil_cycle.js";
import { entityAttackRangeTiles } from "../../client/src/config.js";
import { DiscipleProjectileVisuals, sampleDiscipleFlight, DISCIPLE_PROJECTILE_LIMIT } from "../../client/src/renderer/disciple_projectiles.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

const shooter = { id: 1, kind: KIND.WARRIOR, owner: 1, x: 10, y: 20 };
const target = { id: 2, kind: KIND.RIFLEMAN, x: 170, y: 20 };
const effects = new VisualEffectBuffers();
effects.applySnapshotEvents([{ e: EVENT.ATTACK, from: 1, to: 2, weaponKind: WEAPON_KIND.DISCIPLE_ARROW }], 1000,
  id => id === 1 ? shooter : target);
const arrow = effects.muzzleFlashes[0];
shooter.x = 900; shooter.owner = 2; target.x = 900;
assert.equal(arrow.daoOwner, 1, "arrow retains the firing owner even if its shooter changes");
assert.deepEqual(arrow.daoOrigin, { x: 10, y: 20 }, "moving shooter cannot drag an already fired arrow");
assert.deepEqual(arrow.targetPos, { x: 170, y: 20 }, "flight endpoint is frozen instead of tracking hidden/moving targets");
assert.equal(sampleDiscipleFlight(arrow, 1099), null, "projectile waits for authored release pose");
assert.equal(sampleDiscipleFlight(arrow, 1100).x, 10);
assert(Math.abs(sampleDiscipleFlight(arrow, 1200).x - 75) < 1e-9);
assert.equal(sampleDiscipleFlight(arrow, 1400), null, "flight retires when it reaches its endpoint");
assert.equal(effects.liveMuzzleFlashes(1300).length, 1, "Dao flight outlives the gun flash TTL");
assert.equal(effects.liveMuzzleFlashes(1601).length, 0);
for (const weaponKind of [WEAPON_KIND.DISCIPLE_ARROW, WEAPON_KIND.DISCIPLE_ORB]) {
  assert.equal(weaponRecoilDurationMs(KIND.WARRIOR, weaponKind), 400);
  const hidden = new VisualEffectBuffers();
  hidden.applySnapshotEvents([{ e: EVENT.ATTACK, from: 1, to: 99, weaponKind }], 0, () => null);
  assert.equal(sampleDiscipleFlight(hidden.muzzleFlashes[0], 120), null, "no guessed endpoints for absent entities");
  hidden.applyAttackReveals([{ e: EVENT.ATTACK, from: 1, to: 99, weaponKind,
    reveal: { owner: 2, kind: KIND.WARRIOR, x: 50, y: 60 } }], 0);
  assert.equal(hidden.shotRevealsById.get(1).dao, weaponKind === WEAPON_KIND.DISCIPLE_ARROW ? "lightness" : "magic",
    "firing reveals retain the specialized silhouette using the authorized weapon hint");
}
assert.equal(entityAttackRangeTiles({ kind: KIND.LIGHTNESS_ADEPT, dao: "lightness" }), 0.75);
assert.equal(entityAttackRangeTiles({ kind: KIND.MAGIC_ADEPT, dao: "magic" }), 5);
assert.equal(entityAttackRangeTiles({ kind: KIND.WAR_ADEPT, dao: "war" }), 1.75);
assert.equal(entityAttackRangeTiles({ kind: KIND.WARRIOR }), 0.75);

let allocations = 0, destroyed = 0, textureDestroyed = 0;
class Sprite {
  constructor() { allocations++; this.anchor = { set() {} }; this.position = { set(x, y) { this.x = x; this.y = y; } }; }
  destroy() { destroyed++; }
}
let loading;
const visuals = new DiscipleProjectileVisuals({ pixi: { Sprite }, layer: { addChild() {} },
  loadTexture: async () => ({ width: 128, height: 13, destroy() { textureDestroyed++; } }),
  trackAsset: (id, promise) => { loading = promise; } });
await loading;
const g = new RecordingGraphics();
const manyArrows = Array.from({ length: 1000 }, () => arrow);
visuals.update(manyArrows, 1200, g);
assert.equal(allocations, DISCIPLE_PROJECTILE_LIMIT, "dense fire has a bounded sprite pool");
const teamColors = new Map([[1, 0x0072b2], [2, 0xd55e00]]);
visuals.update([arrow], 1200, g, teamColors);
assert.equal(visuals.sprites[0].tint, 0x0072b2, "arrow uses its firing owner's assigned color");
visuals.update([{ ...arrow, daoOwner: 2 }], 1200, g, teamColors);
assert.equal(visuals.sprites[0].tint, 0xd55e00, "reused sprite updates for the next firing team");
visuals.update([{ ...arrow, daoOwner: null }], 1200, g, teamColors);
assert.equal(visuals.sprites[0].tint, 0xffffff, "unknown owner clears the previous sprite tint");
visuals.update([], 1210, g);
assert(visuals.sprites.every(s => !s.visible));
visuals.update(manyArrows, 1220, g);
assert.equal(allocations, DISCIPLE_PROJECTILE_LIMIT, "later volleys reuse the same sprites");
const orb = { ...arrow, weaponKind: WEAPON_KIND.DISCIPLE_ORB };
visuals.update(Array.from({ length: 1000 }, () => orb), 1200, g);
assert.equal(allocations, DISCIPLE_PROJECTILE_LIMIT, "orbs allocate no display objects or particles");
assert.equal(g.calls.filter(c => c[0] === "drawCircle").length, DISCIPLE_PROJECTILE_LIMIT * 3,
  "each orb uses exactly three fixed circles, plus a two-line shimmer");
visuals.destroy();
assert.equal(destroyed, allocations);
assert.equal(textureDestroyed, 1);
console.log("disciple_projectile_contracts: frozen endpoints, release timing, bounded pool, fixed-cost shimmer, teardown passed");
