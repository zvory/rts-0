import assert from 'node:assert/strict';
import fs from 'node:fs';
import { PNG } from 'pngjs';
import { createLiveFrameStrips } from '../../client/src/renderer/rigs/frame_strip_routing.js';
import { goldenAttackTrailFrame } from '../../client/src/renderer/rigs/golden_attack_strip.js';
import { renderFrameStripUnit } from '../../client/src/renderer/rigs/frame_strip_runtime.js';
import { installFakePixi } from './pixi_fakes.mjs';
const specs = JSON.parse(fs.readFileSync(new URL('../../client/assets/rigs/cultivator-golden/source/specs.json', import.meta.url)));
const golden = [...createLiveFrameStrips()].filter(([, s]) => s.attackTrail);
assert.equal(golden.length, 7, 'Novice, three Adepts and three Masters have authored trails');
for (const [, file, , h] of specs) {
  const base = PNG.sync.read(fs.readFileSync(new URL(`../../client/assets/rigs/${file}`, import.meta.url)));
  const packed = PNG.sync.read(fs.readFileSync(new URL(`../../client/assets/rigs/cultivator-golden/generated/${file.split('/').at(-1)}`, import.meta.url)));
  assert.equal(packed.width, base.width);
  assert.equal(packed.height, h * 2);
  assert.deepEqual(packed.data.subarray(0, base.data.length), base.data, 'all original body pixels are preserved');
  for (let frame = 0; frame < 4; frame++) {
    let visible = 0;
    // War Master is rectangular; the remaining atlas cells are square.
    const cellWidth = base.width / (file.includes('war-yari') ? 7 : file.includes('warrior-runtime') ? 6 : 8);
    for (let y = h; y < h * 2; y++) for (let x = frame * cellWidth; x < (frame + 1) * cellWidth; x++) {
      if (packed.data[(y * packed.width + x) * 4 + 3] > 5) visible++;
    }
    assert.ok(visible > 20, `nonempty golden effect cell ${frame}: ${file}`);
  }
}
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
      assert.notEqual(teamTint, 0xffffff);
      assert.equal(rig.trailSprite.visible, false);
      for (let i = 0; i < 4; i++) {
        const context = { teamColor, recoilProgress: .8, recoilPhase: (i + .1) / 4 * (strip.firingFrameHoldPhase || 1),
          recoilWeaponKind: strip.firingWeaponKinds[0] };
        assert.equal(goldenAttackTrailFrame(strip, context), i);
        draw(context);
        assert.equal(rig.sprite.tint, teamTint, 'team tint remains active in every attack phase');
        assert.equal(rig.trailSprite.tint, 0xffffff, 'gold texture is independent of team tint');
        assert.equal(rig.trailSprite.visible, true);
        assert.equal(rig.trailSprite.texture, rig.trailTextures[i]);
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
