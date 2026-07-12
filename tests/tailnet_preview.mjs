#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

import {
  cleanupExpiredPreviews,
  createPreviewServer,
  isTailnetIpv4,
  parseArgs,
  parseByteRange,
  parseDuration,
  safeFileName,
  stagePreview,
  tailnetIpv4FromStatus,
} from "../scripts/tailnet-preview.mjs";

const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-tailnet-preview-test-"));
const previewRoot = path.join(tempRoot, "previews");
let server;

try {
  assert.equal(parseDuration("30m"), 1_800_000);
  assert.equal(parseDuration("2h"), 7_200_000);
  assert.equal(parseDuration("1d"), 86_400_000);
  assert.throws(() => parseDuration("0h"), /out of range/);
  assert.throws(() => parseDuration("forever"), /invalid TTL/);
  assert.deepEqual(parseByteRange(null, 5), { start: 0, end: 4, partial: false });
  assert.deepEqual(parseByteRange("bytes=1-3", 5), { start: 1, end: 3, partial: true });
  assert.deepEqual(parseByteRange("bytes=3-", 5), { start: 3, end: 4, partial: true });
  assert.deepEqual(parseByteRange("bytes=-2", 5), { start: 3, end: 4, partial: true });
  assert.equal(parseByteRange("bytes=5-6", 5), null);
  assert.equal(parseByteRange("bytes=0-1,3-4", 5), null);
  assert.equal(parseByteRange(null, -1), null);
  assert.equal(safeFileName("/tmp/My clip (final).mp4"), "My_clip_final_.mp4");

  const parsed = parseArgs(["--ttl", "2h", "--port", "9000", "My clip.mp4"]);
  assert.equal(parsed.port, 9000);
  assert.equal(parsed.ttlMs, 7_200_000);
  assert.equal(path.basename(parsed.source), "My clip.mp4");
  const defaultTtl = parseArgs(["clip.mp4"]);
  assert.equal(defaultTtl.ttlMs, 86_400_000, "previews default to 24 hours");
  assert.throws(() => parseArgs(["--keep", "--ttl", "1h", "clip.mp4"]), /cannot be combined/);
  assert.throws(() => parseArgs(["--keep", "--ttl", "24h", "clip.mp4"]), /cannot be combined/);
  assert.throws(() => parseArgs(["--serve", "--host", "100.64.0.1", "--ttl", "24h"]), /only accepts/);
  assert.throws(() => parseArgs(["--host", "100.64.0.1", "clip.mp4"]), /only valid with --serve/);
  assert.throws(() => parseArgs(["--root", ""]), /requires a directory/);

  assert.equal(isTailnetIpv4("100.64.0.1"), true);
  assert.equal(isTailnetIpv4("100.127.255.255"), true);
  assert.equal(isTailnetIpv4("100.128.0.1"), false);
  assert.equal(isTailnetIpv4("192.168.1.1"), false);
  assert.equal(tailnetIpv4FromStatus({
    TailscaleIPs: ["fd7a::1"],
    Self: { TailscaleIPs: ["100.119.17.21"] },
  }), "100.119.17.21");

  const source = path.join(tempRoot, "My clip (final).mp4");
  fs.writeFileSync(source, "0123456789");
  const createdAt = 1_000_000;
  const preview = stagePreview({ root: previewRoot, source, ttlMs: 10_000, now: createdAt });
  assert.equal(preview.name, "My_clip_final_.mp4");
  assert.equal(fs.readFileSync(preview.path, "utf8"), "0123456789");

  const defaultPreview = stagePreview({ root: previewRoot, source, now: createdAt });
  assert.equal(defaultPreview.expiresAt, createdAt + 86_400_000, "staged previews default to 24 hours");

  let clock = createdAt + 1;
  server = createPreviewServer({ root: previewRoot, now: () => clock });
  const port = await listen(server);
  const previewPath = `/p/${preview.id}/${preview.name}`;

  const health = await request({ port, pathname: "/_tailnet-preview/health" });
  assert.equal(health.statusCode, 200);
  const healthBody = JSON.parse(health.body);
  assert.equal(healthBody.service, "rts-tailnet-preview");
  assert.equal(healthBody.root, undefined, "health response does not reveal the local temporary path");
  assert.match(healthBody.rootTag, /^[A-Za-z0-9_-]{16}$/);

  const full = await request({ port, pathname: previewPath });
  assert.equal(full.statusCode, 200);
  assert.equal(full.headers["content-type"], "video/mp4");
  assert.equal(full.headers["accept-ranges"], "bytes");
  assert.equal(full.headers["content-security-policy"], "default-src 'none'; img-src 'self'; media-src 'self'; style-src 'unsafe-inline'");
  assert.equal(full.headers["referrer-policy"], "no-referrer");
  assert.equal(full.body, "0123456789");

  const partial = await request({
    port,
    pathname: previewPath,
    headers: { Range: "bytes=2-5" },
  });
  assert.equal(partial.statusCode, 206);
  assert.equal(partial.headers["content-range"], "bytes 2-5/10");
  assert.equal(partial.body, "2345");

  const suffix = await request({
    port,
    pathname: previewPath,
    headers: { Range: "bytes=-3" },
  });
  assert.equal(suffix.statusCode, 206);
  assert.equal(suffix.body, "789");

  const invalidRange = await request({
    port,
    pathname: previewPath,
    headers: { Range: "bytes=99-100" },
  });
  assert.equal(invalidRange.statusCode, 416);

  const head = await request({ port, pathname: previewPath, method: "HEAD" });
  assert.equal(head.statusCode, 200);
  assert.equal(head.body, "");

  const traversal = await request({ port, pathname: `/p/${preview.id}/..%2Fmanifest.json` });
  assert.equal(traversal.statusCode, 404);
  const post = await request({ port, pathname: previewPath, method: "POST" });
  assert.equal(post.statusCode, 405);

  const outside = path.join(tempRoot, "outside.txt");
  fs.writeFileSync(outside, "private");
  fs.rmSync(preview.path);
  fs.symlinkSync(outside, preview.path);
  const symlink = await request({ port, pathname: previewPath });
  assert.equal(symlink.statusCode, 404, "a staged file cannot be swapped for a symlink");

  clock = preview.expiresAt + 1;
  const expired = await request({ port, pathname: previewPath });
  assert.equal(expired.statusCode, 404);
  assert.equal(fs.existsSync(path.dirname(preview.path)), false);

  const stale = stagePreview({ root: previewRoot, source, ttlMs: 1, now: createdAt });
  const retained = stagePreview({ root: previewRoot, source, keep: true, now: createdAt });
  assert.equal(cleanupExpiredPreviews(previewRoot, createdAt + 2), 1);
  assert.equal(fs.existsSync(path.dirname(stale.path)), false);
  assert.equal(fs.existsSync(path.dirname(retained.path)), true);

  console.log("tailnet preview utility tests passed");
} finally {
  if (server?.listening) await close(server);
  fs.rmSync(tempRoot, { recursive: true, force: true });
}

function listen(server) {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.removeListener("error", reject);
      resolve(server.address().port);
    });
  });
}

function close(server) {
  return new Promise((resolve) => server.close(resolve));
}

function request({ port, pathname, method = "GET", headers = {} }) {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: "127.0.0.1", port, path: pathname, method, headers }, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => {
        resolve({
          statusCode: response.statusCode,
          headers: response.headers,
          body: Buffer.concat(chunks).toString("utf8"),
        });
      });
    });
    req.once("error", reject);
    req.end();
  });
}
