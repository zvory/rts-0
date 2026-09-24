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

// These checks run without optional native rasterizers.
import { installUnitPngs, unitPngFacing, unitPngSize } from './unit-pngs.mjs';

test('PNG style keeps infantry smaller and corrects only machine-gunner orientation', () => {
  assert.ok(unitPngSize('tank') > 2.5 * unitPngSize('rifleman'));
  assert.equal(unitPngFacing({ kind: 'tank', facing: 1 }), 1);
  assert.equal(unitPngFacing({ kind: 'machine_gunner', facing: 1 }), 1 - Math.PI / 2);
  assert.equal(unitPngFacing({ kind: 'rifleman' }), 0);
});

test('PNG adapter retains classic buildings and restores drawing hooks', async () => {
  const calls = [], masks = [];
  const originalBlip = (...args) => calls.push(['original', ...args]);
  const originalOutline = entities => calls.push(['outline', entities]);
  const minimap = { _drawEntityBlip: originalBlip, _drawPlayerOwnedEntityOutline: originalOutline,
    _worldToCanvas: (x, y) => ({ x, y }) };
  const view = { minimap, canvas: { width: 480 }, state: { players: [{ id: 1, color: '#0072b2' }] } };
  let rasterizations = 0;
  const adapter = installUnitPngs(view, {
    createCanvas: () => ({ getContext: () => ({ drawImage() {}, fillRect() {} }) }),
    loadImage: async () => ({ width: 100, height: 50 }),
    rasterizeSvg: async () => { rasterizations++; return Buffer.from('fake'); },
  });
  const unit = { kind: 'machine_gunner', owner: 1, x: 10, y: 20, facing: 1 };
  const building = { kind: 'barracks', owner: 1 };
  const context = { save() {}, restore() {}, translate() {},
    rotate: angle => calls.push(['rotate', angle]), drawImage: (...args) => masks.push(args) };
  assert.throws(() => minimap._drawEntityBlip(context, unit), /not prepared/);
  await adapter.prepare([unit, building]);
  await adapter.prepare([unit]);
  assert.equal(rasterizations, 1, 'portrait cache survives successive frames');
  minimap._drawEntityBlip(context, building, '#0072b2', true);
  assert.deepEqual(calls.pop(), ['original', context, building, '#0072b2', true]);
  minimap._drawPlayerOwnedEntityOutline([unit, building]);
  assert.deepEqual(calls.pop(), ['outline', [building]]);
  minimap._drawEntityBlip(context, unit);
  assert.deepEqual(calls.pop(), ['rotate', 1 - Math.PI / 2]);
  assert.equal(masks.length, 17, 'white contour and final portrait are both drawn');
  adapter.destroy();
  assert.equal(minimap._drawEntityBlip, originalBlip);
  assert.equal(minimap._drawPlayerOwnedEntityOutline, originalOutline);
});
