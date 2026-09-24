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
  await assert.rejects(broken.prepare([{ kind: 'tank', color: '#0072b2' }]), /asset failed/);
  assert.equal(broken.readiness().ready, false);
  assert.equal(broken.readiness().failedAssets.length, 1);
  broken.destroy();
  let resolve, closed = 0;
  const late = new MinimapUnitIcons({ loadImage: () => new Promise(r => { resolve = r; }),
    createCanvas: () => { throw Error('must not allocate after destroy'); } });
  const pending = late.prepare([{ kind: 'tank', color: '#0072b2' }]);
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
