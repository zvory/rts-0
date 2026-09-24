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
