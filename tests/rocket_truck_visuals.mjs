import assert from 'node:assert/strict';
import { decodeServerMessage, COMPACT_SNAPSHOT_VERSION } from '../client/src/protocol.js';
import { RocketTruckVisuals, rocketFlightPose, rocketRackSlot, ROCKET_COUNT } from '../client/src/renderer/rocket_truck_visuals.js';
import { VisualEffectBuffers } from '../client/src/state_visual_effects.js';
import { KIND } from '../client/src/protocol.js';
import { lightenColor } from '../client/src/renderer/shared.js';
import { RecordingGraphics } from './client_contracts/pixi_fakes.mjs';
import { PRESENTATION_ENTITY_FIELDS } from '../client/src/presentation/entity_snapshot.js';
const slots = Array.from({length: ROCKET_COUNT}, (_, i) => rocketRackSlot(i));
assert.equal(new Set(slots.map(p => `${p.x},${p.y}`)).size, 16);
assert.ok(slots.every(p => p.x < 0 && p.x > -29 && Math.abs(p.y) < 10));
assert.ok(PRESENTATION_ENTITY_FIELDS.includes('rocketRackCount'));
const effects = new VisualEffectBuffers();
const event = {e:'mortarLaunch',from:42,fromX:100,fromY:200,toX:600,toY:300,delayTicks:30,rocket:true};
effects.applySnapshotEvents([event], 1000, () => ({owner:2,facing:Math.PI/4}));
const shot = effects.mortarShells[0];
assert.equal(shot.owner, 2);
assert.equal(shot.facing, Math.PI/4);
assert.equal(shot.from, 42);
const start = rocketFlightPose(shot, 1000);
assert.deepEqual({x:start.x,y:start.y}, {x:100,y:200});
assert.ok(Math.abs(start.facing-Math.atan2(100, 500))<1e-8);
const quarter = rocketFlightPose(shot, 1250);
const middle = rocketFlightPose(shot, 1500);
assert.deepEqual({x:quarter.x,y:quarter.y}, {x:225,y:225});
assert.deepEqual({x:middle.x,y:middle.y}, {x:350,y:250});
assert.equal(quarter.facing, middle.facing);
assert.equal(middle.x - quarter.x, quarter.x - start.x);
const end = rocketFlightPose(shot, 2000);
assert.deepEqual({x:end.x,y:end.y}, {x:600,y:300});
effects.addMortarImpact({x:600,y:300,rocket:true},2000);
assert.equal(effects.mortarShells.length, 0);
console.log('Rocket rack slots, presentation field, detached tint identity, launch pose and impact cleanup passed');

for (const count of [0, 7, 16]) {
  const fields = [42, 1, 27, 100, 200, 150, 150, 1];
  while (fields.length < 44) fields.push(null);
  fields[43] = count;
  const decoded = decodeServerMessage({t:'snapshot',v:COMPACT_SNAPSHOT_VERSION,s:[1,0,0,0,300],e:[fields],n:[0,0,0,0,0]});
  assert.equal(decoded.entities[0].rocketRackCount, count);
}
console.log('Compact rack-count decoder preserves loaded, partial and empty racks');

// Exercise both sprite paths with two owners, including after the launcher disappears.
class FakeSprite {
  position = { set(x, y) { this.x = x; this.y = y; } };
  destroy() { this.destroyed = true; }
}
class FakeContainer extends FakeSprite {
  children = [];
  addChild(sprite) { this.children.push(sprite); }
}
const visuals = Object.assign(Object.create(RocketTruckVisuals.prototype), {
  texture: {}, pixi: { Container: FakeContainer }, racks: new Map(), projectiles: new Map(),
  unitLayer: new FakeContainer(), projectileLayer: new FakeContainer(),
  sprite: () => new FakeSprite(),
});
const colors = new Map([[1, 0x0072b2], [2, 0xd55e00]]);
const entities = [1, 2].map(owner => ({
  id: owner, owner, kind: KIND.ROCKET_LAUNCHER, x: 100, y: owner * 100, rocketRackCount: 16,
}));
const shots = [1, 2].map(owner => ({ ...shot, owner, from: owner }));
const graphics = new RecordingGraphics();
visuals.update(entities, shots, colors, 1250, graphics);
for (const entity of entities) {
  assert.ok(visuals.racks.get(entity.id).children.every(sprite =>
    sprite.tint === lightenColor(colors.get(entity.owner), 0.18)));
}
assert.deepEqual([...visuals.projectiles.values()].map(sprite => sprite.tint),
  [...colors.values()].map(color => lightenColor(color, 0.18)));
entities[0].rocketRackCount = 7;
visuals.update(entities, shots, colors, 1300, graphics);
assert.equal(visuals.racks.get(1).children.filter(sprite => sprite.visible).length, 7);
const hiddenRack = visuals.racks.get(1);
entities[0].visionOnly = true;
visuals.update(entities, shots, colors, 1400, graphics);
assert.equal(visuals.racks.has(1), false, 'vision-only contacts must not render rack geometry');
assert.equal(hiddenRack.destroyed, true, 'a previously visible rack is removed when its body is hidden');
assert.equal(visuals.racks.has(2), true, 'ordinary visible trucks retain their racks');
entities[0].visionOnly = false;
visuals.update(entities, shots, colors, 1450, graphics);
assert.equal(visuals.racks.get(1).children.filter(sprite => sprite.visible).length, 7,
  'a revealed truck restores its current ammunition');
visuals.update([], shots, colors, 1500, graphics);
assert.equal(visuals.racks.size, 0);
assert.deepEqual([...visuals.projectiles.values()].map(sprite => sprite.tint),
  [...colors.values()].map(color => lightenColor(color, 0.18)));
visuals.update([], [], colors, 2000, graphics);
assert.equal(visuals.projectiles.size, 0);
console.log('Blue and orange rockets retain team tint on racks and in detached flight');
