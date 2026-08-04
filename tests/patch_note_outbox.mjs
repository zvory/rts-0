#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { PNG } from "pngjs";

import {
  composeBeforeAfter,
  deliverPatchNote,
  outboxPath,
  parseArgs,
  parseEnvValue,
  postDiscord,
  renderContent,
  resolveWebhooks,
  stagePatchNote,
} from "../scripts/patch-note-outbox.mjs";

const waiter = fs.readFileSync(new URL("../scripts/wait-pr.sh", import.meta.url), "utf8");
assert.doesNotMatch(waiter, /patch-note-outbox|deliver_patch_note|Discord patch-note/);

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  assert.equal(result.status, 0, `${command} ${args.join(" ")} failed\n${result.stderr || result.stdout}`);
  return result.stdout.trim();
}

function solidPng(filename, red, green, blue, width = 320, height = 180) {
  const png = new PNG({ width, height });
  for (let offset = 0; offset < png.data.length; offset += 4) {
    png.data[offset] = red;
    png.data[offset + 1] = green;
    png.data[offset + 2] = blue;
    png.data[offset + 3] = 255;
  }
  fs.writeFileSync(filename, PNG.sync.write(png));
}

assert.deepEqual(
  parseArgs(["stage", "--headline", "Visual refresh", "--change", "Resources look clearer."]),
  {
    command: "stage", repoRoot: path.resolve(new URL("..", import.meta.url).pathname), branch: "",
    headline: "Visual refresh", changes: ["Resources look clearer."], before: "", after: "", output: "",
    ffmpeg: process.env.RTS_PATCH_NOTES_FFMPEG || "ffmpeg", ffprobe: process.env.RTS_PATCH_NOTES_FFPROBE || "ffprobe", help: false,
  },
);
assert.equal(renderContent({ headline: "Update", changes: ["One.", "Two."] }), "**Update**\n• One.\n• Two.");
assert.throws(() => renderContent({ changes: ["x".repeat(1_801)] }), /between 1 and 1800/);
assert.equal(parseEnvValue("A=x\nRTS_PATCH_NOTES_DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/1/token'\n", "RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL"), "https://discord.com/api/webhooks/1/token");
assert.equal(
  outboxPath(new URL("..", import.meta.url).pathname, "zvorygin/a/b") ===
    outboxPath(new URL("..", import.meta.url).pathname, "zvorygin/a-b"),
  false,
  "distinct branch names cannot collide in the shared outbox",
);
assert(path.basename(outboxPath(new URL("..", import.meta.url).pathname, `zvorygin/${"long-".repeat(100)}`)).length <= 145);
assert.deepEqual(
  resolveWebhooks(new URL("..", import.meta.url).pathname, {
    RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL: "https://discord.com/api/webhooks/1/token",
    RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL_SECONDARY: "https://discord.com/api/webhooks/1/token",
  }),
  ["https://discord.com/api/webhooks/1/token"],
  "the same Discord destination is posted only once",
);

const root = fs.mkdtempSync(path.join(os.tmpdir(), "rts-patch-note-outbox-test-"));
try {
  run("git", ["init", "-b", "main"], root);
  run("git", ["config", "user.email", "qa@example.invalid"], root);
  run("git", ["config", "user.name", "Patch Note Test"], root);
  fs.writeFileSync(path.join(root, "README.md"), "fixture\n");
  run("git", ["add", "README.md"], root);
  run("git", ["commit", "-m", "Base"], root);
  run("git", ["checkout", "-b", "zvorygin/visual-refresh"], root);

  const before = path.join(root, "before.png");
  const after = path.join(root, "after.png");
  const composed = path.join(root, "comparison.mp4");
  solidPng(before, 200, 20, 20);
  solidPng(after, 20, 20, 200);
  const oversized = path.join(root, "oversized.png");
  const oversizedHeader = Buffer.alloc(24);
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).copy(oversizedHeader);
  oversizedHeader.write("IHDR", 12, "ascii");
  oversizedHeader.writeUInt32BE(100_000, 16);
  oversizedHeader.writeUInt32BE(100_000, 20);
  fs.writeFileSync(oversized, oversizedHeader);
  assert.throws(
    () => composeBeforeAfter({ before: oversized, after, output: composed }),
    /dimensions are outside the supported capture bound/,
    "oversized declared dimensions are rejected before PNG decoding",
  );
  const probe = composeBeforeAfter({ before, after, output: composed });
  assert.equal(probe.codec, "h264");
  assert(probe.duration >= 3.9 && probe.duration <= 4.1);
  assert(probe.bytes < 9.5 * 1024 * 1024);

  const firstFrame = path.join(root, "first.png");
  const lastFrame = path.join(root, "last.png");
  run("ffmpeg", ["-hide_banner", "-loglevel", "error", "-ss", "1", "-i", composed, "-frames:v", "1", "-y", firstFrame], root);
  run("ffmpeg", ["-hide_banner", "-loglevel", "error", "-ss", "3", "-i", composed, "-frames:v", "1", "-y", lastFrame], root);
  const decodedBefore = PNG.sync.read(fs.readFileSync(firstFrame));
  const decodedAfter = PNG.sync.read(fs.readFileSync(lastFrame));
  const sample = (png) => Array.from(png.data.subarray((100 * png.width + 200) * 4, (100 * png.width + 200) * 4 + 3));
  assert(sample(decodedBefore)[0] > sample(decodedBefore)[2], "first half retains before image");
  assert(sample(decodedAfter)[2] > sample(decodedAfter)[0], "second half retains after image");
  assert(decodedBefore.data[3] === 255 && decodedBefore.data[0] < 80, "BEFORE label adds an opaque dark label bar");

  const staged = stagePatchNote({
    repoRoot: root, branch: "", headline: "Visual refresh", changes: ["Resources are easier to read."],
    before, after, ffmpeg: "ffmpeg", ffprobe: "ffprobe",
  });
  assert.equal(staged.manifest.media.filename, "before-after.mp4");
  assert.equal(fs.existsSync(path.join(outboxPath(root, staged.branch), "manifest.json")), true);
  assert.equal(run("git", ["status", "--short"], root).includes("rts-patch-note-outbox"), false, "outbox remains outside the worktree");

  const delivered = [];
  const env = {
    RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL: "https://discord.com/api/webhooks/1/token",
    RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL_SECONDARY: "https://discord.com/api/webhooks/2/token",
  };
  let secondaryAttempts = 0;
  const post = async (request) => {
    delivered.push(request.webhookUrl);
    if (request.webhookUrl.includes("/2/") && secondaryAttempts++ === 0) throw new Error("secondary unavailable");
    assert.equal(fs.existsSync(request.mediaPath), true);
    return { id: `message-${delivered.length}` };
  };
  await assert.rejects(deliverPatchNote({ repoRoot: root, branch: staged.branch, env, post }), /secondary unavailable/);
  assert.equal(fs.existsSync(outboxPath(root, staged.branch)), true, "partial failure retains outbox");
  const reorderedEnv = {
    RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL: env.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL_SECONDARY,
    RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL_SECONDARY: env.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL,
  };
  const retried = await deliverPatchNote({ repoRoot: root, branch: staged.branch, env: reorderedEnv, post });
  assert.equal(retried.status, "sent");
  assert.deepEqual(delivered, [env.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL, env.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL_SECONDARY, env.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL_SECONDARY]);
  assert.equal(fs.existsSync(outboxPath(root, staged.branch)), false, "complete delivery clears outbox");
  assert.equal((await deliverPatchNote({ repoRoot: root, branch: staged.branch, env, post })).status, "not-staged");

  stagePatchNote({ repoRoot: root, branch: staged.branch, headline: "Text only", changes: ["A small update."], before: "", after: "" });
  assert.equal((await deliverPatchNote({ repoRoot: root, branch: staged.branch, env: {}, post })).status, "not-configured");
  assert.equal(fs.existsSync(outboxPath(root, staged.branch)), true, "missing configuration retains outbox");

  let postedUrl = "";
  const response = await postDiscord({
    content: "• Fixture", webhookUrl: env.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL,
    fetchImpl: async (url, request) => {
      postedUrl = String(url);
      assert(request.body instanceof FormData);
      return { ok: true, json: async () => ({ id: "123" }) };
    },
  });
  assert.equal(response.id, "123");
  assert.match(postedUrl, /[?&]wait=true/);
  await assert.rejects(
    postDiscord({ content: "x", webhookUrl: "https://example.invalid/hook", fetchImpl: async () => ({ ok: true }) }),
    /official HTTPS webhook endpoint/,
  );
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}

console.log("patch_note_outbox: composition, staging, delivery, and best-effort recovery passed");
