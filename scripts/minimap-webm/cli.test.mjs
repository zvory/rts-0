import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const cli = fileURLToPath(new URL('../record-minimap.mjs', import.meta.url));
const run = args => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' });

test('CLI help works without native dependencies or server access', () => {
  const result = run(['--help']);
  assert.equal(result.status, 0);
  assert.match(result.stdout, /--from-samples/);
});
test('CLI rejects invalid input before dependencies or replay launch', () => {
  for (const args of [[], ['0', '/tmp/no.webm'], ['1', '/tmp/no.mp4'], ['1', '/tmp/no.webm', '--server', 'file:///tmp/'], ['1', '/tmp/no.webm', '--unknown']]) {
    assert.notEqual(run(args).status, 0);
  }
});
test('CLI preserves an existing output and does not contact the server', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'minimap-cli-test-'));
  try {
    const output = path.join(dir, 'recording.webm');
    fs.writeFileSync(output, 'existing recording');
    const result = run(['1', output, '--server', 'http://127.0.0.1:1']);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /already exists/);
    assert.equal(fs.readFileSync(output, 'utf8'), 'existing recording');
  } finally { fs.rmSync(dir, { recursive: true }); }
});

test('encoding preserves a replay shorter than one output frame', t => {
  if (process.env.RTS_RUN_MEDIA_TESTS === '0') {
    t.skip('media tests not selected');
    return;
  }
  const encoders = spawnSync('ffmpeg', ['-hide_banner', '-encoders'], { encoding: 'utf8' });
  if (!encoders.stdout?.includes('libvpx-vp9') || spawnSync('ffprobe', ['-version']).status !== 0) {
    t.skip('requires ffmpeg with libvpx-vp9 and ffprobe');
    return;
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'minimap-encode-test-'));
  const checked = (command, args) => {
    const result = spawnSync(command, args, { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout;
  };
  try {
    const master = path.join(dir, 'master.mkv');
    const prefix = path.join(dir, 'preview');
    checked('ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'color=s=16x16:r=45',
      '-frames:v', '1', '-c:v', 'ffv1', master]);
    checked(process.execPath, [fileURLToPath(new URL('./encode.mjs', import.meta.url)), master, prefix]);
    const results = JSON.parse(fs.readFileSync(`${prefix}-compression.json`, 'utf8'));
    assert.equal(results.length, 4);
    for (const { output } of results) {
      const probe = JSON.parse(checked('ffprobe', ['-v', 'error', '-count_frames',
        '-show_entries', 'stream=nb_read_frames', '-of', 'json', output]));
      assert.equal(probe.streams[0].nb_read_frames, '1');
      checked('ffmpeg', ['-v', 'error', '-i', output, '-f', 'null', '-']);
    }
  } finally { fs.rmSync(dir, { recursive: true }); }
});


import { MinimapUnitIcons, unitPngFacing, unitPngSize } from '../../client/src/minimap_unit_icons.js';

test('shared live/export portraits cache drawing, preserve scale and rotate with corrected facing', async () => {
  const canvases = [], draws = [], rotations = [];
  let loads = 0;
  const createCanvas = () => {
    const calls = [];
    const canvas = { width: 0, height: 0, getContext: () => ({
      drawImage: (...args) => calls.push(args), fillRect() {},
    }), calls };
    canvases.push(canvas); return canvas;
  };
  const image = { width: 100, height: 50 };
  const icons = new MinimapUnitIcons({ createCanvas, loadImage: async () => { loads++; return image; } });
  const ctx = { save() {}, restore() {}, translate() {}, rotate: x => rotations.push(x),
    drawImage: (...args) => draws.push(args) };
  const entity = { kind: 'machine_gunner', facing: 1 };
  icons.draw(ctx, entity, '#0072b2', { x: 0, y: 0 }, 480);
  assert.equal(draws.length, 0, 'loading units never fall back to dots');
  await icons.prepare([{ kind: entity.kind, color: '#0072b2' }]);
  icons.draw(ctx, entity, '#0072b2', { x: 0, y: 0 }, 480);
  const cached = draws[0][0];
  icons.draw(ctx, { ...entity, facing: 2 }, '#0072b2', { x: 0, y: 0 }, 480);
  assert.equal(draws[1][0], cached, 'changing facing reuses the portrait');
  assert.equal(loads, 1);
  assert.deepEqual(rotations, [1 - Math.PI / 2, 2 - Math.PI / 2]);
  assert.equal(cached.calls.length, 17, 'outline is composed once, not on every frame');
  icons.draw(ctx, entity, '#0072b2', { x: 0, y: 0 }, 480, true);
  assert.notEqual(draws[2][0], cached, 'attack flash uses its white silhouette composite');
  assert.equal(draws[2][0].calls.at(-1)[0], canvases[0]);
  assert.ok(unitPngSize('tank') > unitPngSize('rifleman') * 2.5);
  assert.equal(unitPngFacing({ kind: 'tank', facing: 1 }), 1);
  assert.equal(unitPngSize('tank', 960), 2 * unitPngSize('tank', 480));
  icons.destroy();
  assert.ok(canvases.every(canvas => canvas.width === 0 && canvas.height === 0));
});

test('portrait failures are visible to capture readiness and teardown discards late loads', async () => {
  const broken = new MinimapUnitIcons({ loadImage: async () => { throw Error('asset failed'); } });
  await assert.rejects(broken.prepare([{ kind: 'rifleman', color: '#0072b2' }]), /asset failed/);
  assert.equal(broken.readiness().ready, false);
  assert.equal(broken.readiness().failedAssets.length, 1);
  broken.destroy();
  let resolve, closed = 0;
  const late = new MinimapUnitIcons({ loadImage: () => new Promise(r => { resolve = r; }),
    createCanvas: () => { throw Error('must not allocate after destroy'); } });
  const pending = late.prepare([{ kind: 'rifleman', color: '#0072b2' }]);
  await Promise.resolve();
  late.destroy();
  resolve({ width: 10, height: 10, close: () => closed++ });
  await pending;
  assert.equal(closed, 1);
  assert.equal(late.entries.size, 0);
});

import { createMinimapUnitIconLoader } from '../../client/src/minimap_icon_image.js';
test('browser portrait loader produces standalone SVG images with a namespace', async () => {
  let source = '';
  const image = { width: 100, height: 50,
    set src(value) { source = value; if (value) queueMicrotask(() => this.onload?.()); } };
  const loader = createMinimapUnitIconLoader(() => '<svg width="100" height="50"><path d="M0 0h10v10z"/></svg>',
    { ownerDocument: { createElement: () => image } });
  const loaded = await loader('tank', '#0072b2', { signal: new AbortController().signal });
  assert.equal(loaded, image);
  assert.match(decodeURIComponent(source), /<svg xmlns="http:\/\/www.w3.org\/2000\/svg"/);
  assert.equal(image.onload, null);
});

import { liveUnitIconMarkupFor } from '../../client/src/renderer/rigs/unit_icon_sources.js';
test('articulated minimap layers follow live facing and setup without loading each angle', async () => {
  const loaded = [], rotations = [];
  const icons = new MinimapUnitIcons({
    createCanvas: () => ({ width: 0, height: 0, getContext: () => ({ drawImage() {}, fillRect() {} }) }),
    loadImage: async (kind, color, options) => {
      loaded.push([kind, options.minimapPart]); return { width: 128, height: 128 };
    },
  });
  await icons.prepare([{ kind: 'tank', color: '#0072b2' }, { kind: 'anti_tank_gun', color: '#0072b2' }]);
  const ctx = { save() {}, restore() {}, translate() {}, drawImage() {}, rotate: a => rotations.push(a) };
  const draw = e => icons.draw(ctx, e, '#0072b2', { x: 0, y: 0 }, 480);
  draw({ kind: 'tank', facing: 1, weaponFacing: 2 });
  draw({ kind: 'anti_tank_gun', facing: 1, weaponFacing: 2, setupFacing: 3, setupState: 'packed' });
  draw({ kind: 'anti_tank_gun', facing: 1, weaponFacing: 2, setupFacing: 3, setupState: 'deployed' });
  assert.deepEqual(rotations, [1, 2, 1, 1, 3, 2]);
  assert.equal(loaded.length, 6);
  draw({ kind: 'tank', facing: 2, weaponFacing: 3 });
  assert.equal(loaded.length, 6, 'continuous aim uses cached components');
  const packed = liveUnitIconMarkupFor('anti_tank_gun', { minimapPart: 'packed-base' });
  const deployed = liveUnitIconMarkupFor('anti_tank_gun', { minimapPart: 'deployed-base' });
  assert.doesNotMatch(packed, /data-unit-icon-component="[^"]*Trail/);
  assert.match(deployed, /data-unit-icon-component="sprite.at.leftTrail.deployed"/);
  assert.match(deployed, /data-unit-icon-component="sprite.at.rightTrail.deployed"/);
  assert.equal(packed.match(/viewBox="[^"]*"/)[0], deployed.match(/viewBox="[^"]*"/)[0]);
  icons.destroy();
});

import { createMinimapAnimation } from '../../client/src/renderer/rigs/minimap_animation.js';
test('live minimap primes animation frames before the first setup or firing transition', async () => {
  const animation = createMinimapAnimation();
  const requested = [];
  const icons = new MinimapUnitIcons({ animation,
    createCanvas: () => ({ width: 0, height: 0, getContext: () => ({ drawImage() {}, fillRect() {} }) }),
    loadImage: async (kind, color, { minimapPart }) => {
      requested.push(minimapPart);
      return { width: 128, height: 128 };
    },
  });
  let draws = 0;
  const context = { globalAlpha: 1, save() {}, restore() {}, translate() {}, rotate() {}, scale() {},
    drawImage() { draws++; } };
  const entity = { id: 1, kind: 'machine_gunner', setupState: 'packed' };
  icons.beginFrame([entity], {}, 0);
  icons.draw(context, entity, '#0072b2', { x: 0, y: 0 }, 480);
  await Promise.all([...icons.entries.values()].map(entry => entry.promise));
  assert.deepEqual(requested, animation.parts(entity.kind), 'first live draw loads every bounded frame');
  entity.setupState = 'deployed';
  icons.beginFrame([entity], { weaponRecoil: () => 1, weaponRecoilPhase: () => 0.5 }, 100);
  icons.draw(context, entity, '#0072b2', { x: 0, y: 0 }, 480);
  assert.equal(draws, 1, 'the first firing frame draws immediately without capture preparation');
  assert.equal(requested.length, animation.parts(entity.kind).length);
  icons.destroy();
  assert.equal(icons.primedAnimations.size, 0);
});

test('minimap uses production setup frames and shot envelopes, with a bounded image cache', async () => {
  const animation = createMinimapAnimation();
  let recoil = 0, phase = 0;
  const state = { weaponRecoil: () => recoil, weaponRecoilPhase: () => phase,
    weaponRecoilKind: () => 'rifleman_rifle' };
  const mg = { id: 1, kind: 'machine_gunner', facing: 1, weaponFacing: 2, setupState: 'packed' };
  const sample = now => { animation.beginFrame([mg], state, now); return animation.layers(mg); };
  assert.equal(sample(0)[0].part, 'frame:0');
  mg.setupState = 'setting_up';
  assert.equal(sample(100)[0].part, 'frame:6');
  assert.equal(sample(600)[0].part, 'frame:9');
  mg.setupState = 'deployed';
  assert.equal(sample(1100)[0].part, 'frame:11');
  assert.equal(sample(1100)[0].rotation, 2 - Math.PI / 2);
  recoil = 1; phase = 0.5;
  assert.equal(sample(1150)[0].part, 'frame:13');
  recoil = 0;
  mg.setupState = 'tearing_down';
  assert.equal(sample(1200)[0].part, 'frame:11');
  assert.equal(sample(1700)[0].part, 'frame:9');
  assert.equal(sample(2200)[0].part, 'frame:6');
  const tank = { id: 2, kind: 'tank', facing: 0.3, weaponFacing: 1.2 };
  animation.beginFrame([tank], state, 2300);
  const resting = animation.layers(tank);
  recoil = 1;
  animation.beginFrame([tank], state, 2310);
  const firing = animation.layers(tank);
  assert.notDeepEqual(firing, resting, 'recoil moves the hull/turret and retracts the barrel');
  assert.equal(firing.find(layer => layer.part.includes('barrel')).rotation, 1.2);
  const requested = [];
  const icons = new MinimapUnitIcons({ animation,
    createCanvas: () => ({ width: 0, height: 0, getContext: () => ({ drawImage() {}, fillRect() {} }) }),
    loadImage: async (kind, color, { minimapPart }) => {
      requested.push(minimapPart);
      assert.match(liveUnitIconMarkupFor(kind, { minimapPart }), /<image/);
      return { width: 128, height: 128 };
    },
  });
  await icons.prepare([{ kind: 'tank', color: '#0072b2' }, { kind: 'machine_gunner', color: '#0072b2' }]);
  const count = requested.length;
  const context = { globalAlpha: 1, save() {}, restore() {}, translate() {}, rotate() {}, scale() {}, drawImage() {} };
  for (let i = 0; i < 50; i++) {
    tank.weaponFacing = i / 7; recoil = i / 50;
    icons.beginFrame([tank], state, 2400 + i);
    icons.draw(context, tank, '#0072b2', { x: 0, y: 0 }, 480);
  }
  assert.equal(requested.length, count, 'angles and recoil never create image variants');
  icons.destroy();
});

test('all production atlas and strip minimap routes can sample each setup state', () => {
  const animation = createMinimapAnimation();
  const kinds = ['rifleman', 'panzerfaust', 'machine_gunner', 'anti_tank_gun', 'mortar_team',
    'artillery', 'scout_car', 'tank', 'command_car', 'rocket_launcher', 'scout_plane'];
  for (const kind of kinds) for (const setupState of ['packed', 'setting_up', 'deployed', 'tearing_down']) {
    const entity = { id: 1, kind, setupState, facing: 0.4, weaponFacing: 1.1 };
    animation.beginFrame([entity], { weaponRecoil: () => 1, weaponRecoilPhase: () => 0.2 }, 100);
    const layers = animation.layers(entity);
    assert.ok(layers.length > 0, `${kind} ${setupState}`);
    const parts = animation.parts(kind);
    for (const layer of layers) {
      assert.ok(parts.includes(layer.part));
      for (const field of ['rotation', 'x', 'y', 'scaleX', 'scaleY', 'alpha']) {
        assert.ok(Number.isFinite(layer[field]), `${kind} ${field}`);
      }
    }
  }
  animation.destroy();
});


test('animated strip images retain full setup silhouettes and switch unloaded Panzerfaust art', () => {
  const markup = liveUnitIconMarkupFor('machine_gunner', { minimapPart: 'frame:11' });
  assert.match(markup, /viewBox="1408 0 128 128"/,
    'deployed heads and barrels must not be clipped to the carry portrait bounds');
  const animation = createMinimapAnimation();
  const entity = { id: 1, kind: 'panzerfaust', panzerfaustLoaded: false };
  animation.beginFrame([entity], {}, 0);
  assert.equal(animation.layers(entity)[0].part, 'unloaded-frame:0');
  const unloaded = liveUnitIconMarkupFor('panzerfaust', { minimapPart: 'unloaded-frame:0' });
  assert.match(unloaded, /rifleman-recoil-review-strip/);
  entity.panzerfaustLoaded = true;
  animation.beginFrame([entity], {}, 0);
  assert.equal(animation.layers(entity)[0].part, 'frame:0');
  animation.destroy();
});


test('minimap tank omits only atlas cells declared and verified transparent', async () => {
  const { PNG } = await import('pngjs');
  const { createLivePngRigAtlases } = await import('../../client/src/renderer/rigs/png_routing.js');
  const { createMinimapAnimation } = await import('../../client/src/renderer/rigs/minimap_animation.js');
  const atlas = createLivePngRigAtlases().get('tank');
  const image = PNG.sync.read(fs.readFileSync(new URL(`../../client${atlas.image}`, import.meta.url)));
  const blank = new Set(atlas.grid.normalization.blankCells);
  const animation = createMinimapAnimation();
  const parts = animation.parts('tank');
  assert.equal(parts.length, 3, 'retain hull, turret and the visible independent barrel');
  for (const sprite of atlas.sprites) {
    const { x, y, w, h } = sprite.frame;
    let visible = false;
    for (let row = y; row < y + h && !visible; row++) {
      for (let col = x; col < x + w; col++) {
        if (image.data[(row * image.width + col) * 4 + 3]) { visible = true; break; }
      }
    }
    assert.equal(visible, !blank.has(sprite.sourceCell), `${sprite.id} matches blank-cell metadata`);
    assert.equal(parts.includes(`sprite:${sprite.id}`), visible);
  }
  animation.destroy();
});
