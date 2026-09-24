#!/usr/bin/env node
// Manual export only: one selected replay, using the production minimap renderer.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { parseArgs } from 'node:util';
import { spawn, spawnSync } from 'node:child_process';

const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(import.meta.url);
const help = `Usage: node scripts/record-minimap.mjs MATCH_ID OUTPUT.webm [options]
       node scripts/record-minimap.mjs --from-samples CAPTURE.jsonl OUTPUT.webm [options]

Manually record one replay as a silent 480x480 WebM, 15x speed, combined player
fog, game clock only. Uses the regular game minimap renderer. No batch jobs or uploads.

Options:
  --server URL           Replay server (default: https://rts-0-zvorygin-beta.fly.dev)
  --from-samples FILE    Reuse a completed schema-2 capture without server access
  --unit-pngs            Larger rotating unit portraits with white outlines; classic buildings
  --keep-work            Keep captured state and lossless master for later re-encoding
  --canvas-package PATH  Use an existing @napi-rs/canvas installation
  --help                Show this help

Requires Node >=22.18, ffmpeg (libvpx-vp9), and ffprobe. On first use, installs
Canvas 1.0.9 in ~/.cache/rts-minimap-webm; prepares repository Node dependencies
if capture needs them. Existing output files are never overwritten.
`;
let child;
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => { child?.kill(signal); process.exit(signal === 'SIGINT' ? 130 : 143); });
}
function run(command, args) {
  return new Promise((resolve, reject) => {
    child = spawn(command, args, { cwd: root, stdio: 'inherit' });
    child.once('error', reject);
    child.once('close', code => {
      child = undefined;
      code === 0 ? resolve() : reject(Error(`${path.basename(command)} failed (${code})`));
    });
  });
}
async function main() {
  const { values, positionals } = parseArgs({ allowPositionals: true, options: {
    'unit-pngs': { type: 'boolean' }, help: { type: 'boolean' }, server: { type: 'string', default: 'https://rts-0-zvorygin-beta.fly.dev' },
    'from-samples': { type: 'string' }, 'keep-work': { type: 'boolean' }, 'canvas-package': { type: 'string' },
  } });
  if (values.help) { console.log(help); return; }
  const fromSamples = values['from-samples'] && path.resolve(values['from-samples']);
  const [id, destination] = fromSamples ? [null, positionals[0]] : positionals;
  if (positionals.length !== (fromSamples ? 1 : 2) || (!fromSamples && !/^[1-9]\d*$/.test(id))) throw Error(help);
  const output = path.resolve(destination);
  if (!output.endsWith('.webm')) throw Error('Output filename must end in .webm');
  if (fs.existsSync(output)) throw Error(`Output already exists: ${output}`);
  fs.accessSync(path.dirname(output), fs.constants.W_OK);
  const server = new URL(values.server);
  if (!['http:', 'https:'].includes(server.protocol) || server.username || server.password || server.search || server.hash || server.pathname !== '/') {
    throw Error('--server must be an HTTP(S) origin, e.g. http://localhost:8080');
  }
  if (fromSamples) fs.accessSync(fromSamples, fs.constants.R_OK);
  for (const command of ['ffmpeg', 'ffprobe']) {
    if (spawnSync(command, ['-version'], { stdio: 'ignore' }).status !== 0) throw Error(`Install ${command} before recording.`);
  }
  const encoders = spawnSync('ffmpeg', ['-hide_banner', '-encoders'], { encoding: 'utf8' });
  if (!encoders.stdout?.includes('libvpx-vp9')) throw Error('ffmpeg must include the libvpx-vp9 encoder.');
  const cache = path.join(os.homedir(), '.cache', 'rts-minimap-webm');
  const canvas = values['canvas-package'] ? path.resolve(values['canvas-package']) : path.join(cache, 'node_modules', '@napi-rs', 'canvas');
  if (!values['canvas-package'] && !fs.existsSync(canvas)) {
    console.log(`Installing optional Canvas dependency in ${cache}`);
    await run('npm', ['install', '--prefix', cache, '--no-audit', '--no-fund', '@napi-rs/canvas@1.0.9']);
  }
  const sharp = values['unit-pngs'] ? path.join(cache, 'node_modules', 'sharp') : null;
  if (sharp && !fs.existsSync(sharp)) {
    console.log(`Installing optional PNG rasterizer in ${cache}`);
    await run('npm', ['install', '--prefix', cache, '--no-audit', '--no-fund', 'sharp@0.34.5']);
  }
  if (sharp) require(sharp);
  require(canvas); // Check native module availability before launching a replay.
  if (!fromSamples) {
    try { require.resolve('ws'); }
    catch { await run('bash', [path.join(root, 'scripts/ensure-node-deps.sh'), '--repo', root, '--quiet']); }
  }
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'rts-minimap-'));
  console.log(`Working files: ${work}`);
  try {
    const samples = fromSamples || path.join(work, 'capture.jsonl');
    const master = path.join(work, 'master.mkv');
    const prefix = path.join(work, 'preview');
    const script = name => path.join(root, 'scripts/minimap-webm', `${name}.mjs`);
    if (!fromSamples) {
      console.log(`Capturing match ${id}; replay simulation may take several minutes…`);
      await run(process.execPath, [script('capture'), server.origin, id, samples]);
    }
    console.log('Rendering with the game minimap, then encoding VP9…');
    await run(process.execPath, [script('render'), samples, master, canvas, ...(sharp ? [sharp] : [])]);
    await run(process.execPath, [script('encode'), master, prefix, '480-q44']);
    const encoded = `${prefix}-480-q44.webm`;
    await run('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-i', encoded, '-f', 'null', '-']);
    fs.copyFileSync(encoded, output, fs.constants.COPYFILE_EXCL);
    console.log(`Saved ${output} (${(fs.statSync(output).size / 1000000).toFixed(3)} MB)`);
    if (values['keep-work']) console.log(`Kept samples/master: ${work}`);
    else fs.rmSync(work, { recursive: true });
  } catch (error) {
    console.error(`Working files retained for diagnosis: ${work}`);
    throw error;
  }
}
main().catch(error => { console.error(error.message); process.exitCode = 1; });
