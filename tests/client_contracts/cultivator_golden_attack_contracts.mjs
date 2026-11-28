import assert from 'node:assert/strict';
import fs from 'node:fs';
import { PNG } from 'pngjs';
import { createLiveFrameStrips } from '../../client/src/renderer/rigs/frame_strip_routing.js';
import { goldenAttackTrailFrame } from '../../client/src/renderer/rigs/golden_attack_strip.js';
import { WARRIOR_PNG_FRAME_STRIP } from '../../client/src/renderer/rigs/warrior_png_strip.js';
import { frameStripWorldScale, frameStripSpriteOffset, frameStripVisualFacing, renderFrameStripUnit } from '../../client/src/renderer/rigs/frame_strip_runtime.js';
import { createMinimapAnimation } from '../../client/src/renderer/rigs/minimap_animation.js';
import { installFakePixi } from './pixi_fakes.mjs';
const specs = JSON.parse(fs.readFileSync(new URL('../../client/assets/rigs/cultivator-golden/source/specs.json', import.meta.url)));
const golden = [...createLiveFrameStrips()].filter(([, s]) => s.attackTrail);
assert.equal(golden.length, 7, 'Novice, three Adepts and three Masters have authored trails');
for (const [, file, w, h, , options = {}] of specs) {
  const base = PNG.sync.read(fs.readFileSync(new URL(`../../client/assets/rigs/${file}`, import.meta.url)));
  const packed = PNG.sync.read(fs.readFileSync(new URL(`../../client/assets/rigs/cultivator-golden/generated/${file.split('/').at(-1)}`, import.meta.url)));
  assert.equal(packed.width, base.width + (options.bodyFrames || 0) * w);
  assert.equal(packed.height, h * 2);
  for (let y = 0; y < h; y++) assert.deepEqual(
    packed.data.subarray(y * packed.width * 4, (y * packed.width + base.width) * 4),
    base.data.subarray(y * base.width * 4, (y + 1) * base.width * 4), 'all original body pixels are preserved');
  if (options.bodyFrames) {
    const distinct = new Set();
    for (let f = 0; f < options.bodyFrames; f++) {
      const body = new PNG({ width: w, height: h });
      PNG.bitblt(packed, body, base.width + f * w, 0, w, h, 0, 0);
      assert.ok(body.data.some((v, i) => i % 4 === 3 && v > 200), 'additional body pose is visible');
      distinct.add(body.data.toString('base64'));
    }
    assert.equal(distinct.size, 6, 'Novice has six distinct articulated attack body poses');
  }
  for (let frame = 0; frame < (options.effectFrames || 4); frame++) {
    let visible = 0;
    // War Master is rectangular; the remaining atlas cells are square.
    const cellWidth = w;
    for (let y = h; y < h * 2; y++) for (let x = frame * cellWidth; x < (frame + 1) * cellWidth; x++) {
      if (packed.data[(y * packed.width + x) * 4 + 3] > 5) visible++;
    }
    assert.ok(visible > 20, `nonempty golden effect cell ${frame}: ${file}`);
  }
}
// Compare the actual head pixels at rendered scale, rather than accepting equal cell sizes.
const novice = WARRIOR_PNG_FRAME_STRIP;
const novicePng = PNG.sync.read(fs.readFileSync(new URL(`../../${novice.source.runtimeStrip}`, import.meta.url)));
function headSample(frame, cx, cy, radius) {
  const points = [];
  for (let y = cy - radius; y <= cy + radius; y++) for (let x = cx - radius; x <= cx + radius; x++) {
    const i = (y * novicePng.width + frame * 160 + x) * 4;
    if (novicePng.data[i + 3] > 230 && Math.max(...novicePng.data.subarray(i, i + 3)) < 105) points.push([x, y]);
  }
  const x = points.reduce((n, p) => n + p[0], 0) / points.length;
  const y = points.reduce((n, p) => n + p[1], 0) / points.length;
  const radiusPx = Math.sqrt(points.reduce((n, p) => n + (p[0] - x) ** 2 + (p[1] - y) ** 2, 0) / points.length);
  return { x, y, radiusPx };
}
const idleHead = headSample(0, 58, 72, 24);
const subject = { kind: 'warrior', weaponFacing: 0, facing: 0 };
function worldHead(frame, head, context) {
  const scale = frameStripWorldScale(novice, subject, context, frame);
  const offset = frameStripSpriteOffset(novice, frame, context);
  const angle = frameStripVisualFacing(novice, subject, context, frame);
  const x = (head.x - 80) * scale + offset.x, y = (head.y - 80) * scale + offset.y;
  return { x: x * Math.cos(angle) - y * Math.sin(angle), y: x * Math.sin(angle) + y * Math.cos(angle), radius: head.radiusPx * scale };
}
const idleWorld = worldHead(0, idleHead, {});
for (const [i, frame] of novice.firingFrames.entries()) {
  const context = { recoilProgress: 1, recoilWeaponKind: 'warrior_sword', recoilPhase: (i + .1) / 6 * novice.firingFrameHoldPhase };
  const head = worldHead(frame, headSample(frame, 40, 69, 17), context);
  assert.ok(Math.abs(head.radius / idleWorld.radius - 1) < .03, 'attack head size matches idle within 3%');
  assert.ok(Math.hypot(head.x - idleWorld.x, head.y - idleWorld.y) < .35, 'head anchor does not jump during swing');
}
const minimap = createMinimapAnimation();
const minimapUnit = { ...subject, id: 42, x: 0, y: 0 };
minimap.beginFrame([minimapUnit], {}, 0);
const idleLayer = minimap.layers(minimapUnit)[0];
minimap.beginFrame([minimapUnit], {
  weaponRecoil: () => 1, weaponRecoilPhase: () => .2, weaponRecoilKind: () => 'warrior_sword',
}, 1);
const attackLayer = minimap.layers(minimapUnit)[0];
assert.ok(Math.abs(attackLayer.scaleX / idleLayer.scaleX - 1.59) < 1e-12, 'minimap shares corrected Novice attack sizing');
function minimapHead(layer, head) {
  const x = (head.x - 80) / 160 * layer.scaleX;
  const y = (head.y - 80) / 160 * layer.scaleY;
  return { x: layer.x + x * Math.cos(layer.rotation) - y * Math.sin(layer.rotation),
    y: layer.y + x * Math.sin(layer.rotation) + y * Math.cos(layer.rotation) };
}
const iconWorldScale = 1 / (153 * 1.05 * novice.worldScale);
for (const facing of [0, Math.PI / 2, 2.3]) {
  minimapUnit.facing = minimapUnit.weaponFacing = facing;
  minimap.beginFrame([minimapUnit], {}, 0);
  const idle = minimapHead(minimap.layers(minimapUnit)[0], idleHead);
  for (const [i, frame] of novice.firingFrames.entries()) {
    minimap.beginFrame([minimapUnit], {
      weaponRecoil: () => 1, weaponRecoilKind: () => 'warrior_sword',
      weaponRecoilPhase: () => (i + .1) / 6 * novice.firingFrameHoldPhase,
    }, 1);
    const head = minimapHead(minimap.layers(minimapUnit)[0], headSample(frame, 40, 69, 17));
    assert.ok(Math.hypot(head.x - idle.x, head.y - idle.y) < .35 * iconWorldScale,
      'minimap preserves the head anchor across firing rotations and scales');
  }
}
minimap.destroy();
const restore = installFakePixi();
try {
  for (const [kind, strip] of golden) {
    const texture = PIXI.Texture.from(`gold-test-${kind}`);
    const renderer = { _liveRigPools: { units: new Map() }, _seen: { units: new Set() }, layers: { units: new PIXI.Container() } };
    const entity = { id: 1, kind: kind.split(':')[0], x: 50, y: 50, weaponFacing: 0 };
    const draw = context => renderFrameStripUnit(renderer, entity, strip, texture,
      { poolName: 'units', layerName: 'units', renderContext: context });
    for (const teamColor of [0x0072b2, 0xd55e00]) {
      const rig = draw({ teamColor, recoilProgress: 0 });
      const teamTint = rig.sprite.tint;
      assert.equal(teamTint, 0xffffff, "team-paint palettes preserve fixed materials without a whole-body multiply");
      assert.equal(rig.trailSprite.visible, false);
      for (let i = 0; i < strip.attackTrail.frameCount; i++) {
        const context = { teamColor, recoilProgress: .8, recoilPhase: (i + .1) / strip.attackTrail.frameCount * (strip.firingFrameHoldPhase || 1),
          recoilWeaponKind: strip.firingWeaponKinds[0] };
        assert.equal(goldenAttackTrailFrame(strip, context), i);
        draw(context);
        assert.equal(rig.sprite.tint, teamTint, 'fixed-material palette stays unmultiplied in every attack phase');
        assert.equal(rig.trailSprite.tint, 0xffffff, 'gold texture is independent of team tint');
        assert.equal(rig.trailSprite.visible, true);
        assert.equal(rig.trailSprite.texture, rig.trailTextures[i]);
        if (kind.startsWith('magic_adept')) {
          assert.ok(Math.abs(rig.trailSprite.scaleX / rig.sprite.scaleX - 2) < 1e-12, 'Magic energy has twice its original diameter');
          const center = strip.attackTrail.centers[i];
          const sourceCenter = strip.attackTrail.sourceCenters[i];
          assert.ok(Math.abs(rig.trailSprite.x + (sourceCenter[0] - 128) * rig.trailSprite.scaleX
            - rig.sprite.x - (center[0] - 128) * rig.sprite.scaleX) < 1e-8, 'energy stays centered horizontally on the hand anchor');
          assert.ok(Math.abs(rig.trailSprite.y + (sourceCenter[1] - 128) * rig.trailSprite.scaleY
            - rig.sprite.y - (center[1] - 128) * rig.sprite.scaleY) < 1e-8, 'energy stays centered vertically on the hand anchor');
        }
      }
      draw({ teamColor, recoilProgress: 1, recoilPhase: 1, recoilWeaponKind: strip.firingWeaponKinds[0] });
      assert.equal(rig.trailSprite.visible, false, 'recovery ends the trail');
      draw({ teamColor, recoilProgress: 1, recoilPhase: .3, recoilWeaponKind: 'wrong_weapon' });
      assert.equal(rig.trailSprite.visible, false);
    }
    const rig = renderer._liveRigPools.units.get(1);
    rig.destroy();
    assert.equal(rig.trailTextures.length, 0);
    const shadow = renderFrameStripUnit(renderer, { ...entity, id: 2 }, strip, texture,
      { poolName: 'units', layerName: 'units', silhouetteShadow: true });
    assert.equal(shadow.trailSprite, undefined, 'gold light does not cast a silhouette shadow');
    shadow.destroy();
  }
} finally { restore(); }
console.log('cultivator_golden_attack_contracts: ok');
