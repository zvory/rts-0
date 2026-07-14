import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

import {
  INTERACT_PREVIEW_TTL_MS,
  InteractTailnetPreview,
  MAX_TAILNET_PREVIEW_ARTIFACT_BYTES,
} from "../scripts/interact/tailnet_preview.ts";
import { createPreviewServer, stagePreview } from "../scripts/tailnet-preview.mjs";

const root = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-durable-preview-"));
const workspaceRoot = path.join(root, "worktree");
const durableRoot = path.join(root, "durable");
const artifactRoot = path.join(workspaceRoot, "target", "interact", "lab", "lab_test", "captures");
fs.mkdirSync(artifactRoot, { recursive: true });
const pngPath = path.join(artifactRoot, "scene.png");
const png = Buffer.from("fixture-png-bytes");
fs.writeFileSync(pngPath, png);

const server = createPreviewServer({ root: durableRoot });
const port = await listen(server);
let publications = 0;
const preview = new InteractTailnetPreview({
  workspaceRoot,
  publishArtifact: async ({ source, ttlMs }) => {
    publications += 1;
    const staged = stagePreview({ root: durableRoot, source, ttlMs });
    return {
      url: `http://127.0.0.1:${port}/p/${staged.id}/${staged.name}`,
      expiresAt: staged.expiresAt,
    };
  },
});

try {
  assert.equal(INTERACT_PREVIEW_TTL_MS, 24 * 60 * 60 * 1000, "Lab artifacts receive a full 24-hour retention window");
  const published = await preview.publish({ filePath: pngPath, mimeType: "image/png" });
  assert.match(published.url, new RegExp(`^http://127\\.0\\.0\\.1:${port}/p/[A-Za-z0-9_-]{16,64}/scene\\.png$`));
  assert.equal(published.url.includes(workspaceRoot), false, "preview URL does not disclose the worktree path");
  assert.equal(published.bytes, png.length, "preview reports bounded artifact bytes");
  assert.ok(published.expiresAt >= Date.now() + INTERACT_PREVIEW_TTL_MS - 1_000, "preview expiry is at least 24 hours after publication");
  assert.equal(published.availability, "available for at least 24 hours after publication");

  const repeated = await preview.publish({ filePath: pngPath, mimeType: "image/png" });
  assert.equal(repeated.url, published.url, "the same immutable artifact keeps one stable URL during the Lab session");
  assert.equal(publications, 1, "deduplication does not create redundant durable copies");

  await preview.close();
  fs.rmSync(workspaceRoot, { recursive: true, force: true });
  const afterShutdownAndCleanup = await fetch(published.url);
  assert.equal(afterShutdownAndCleanup.status, 200, "the copied preview survives Lab daemon shutdown and worktree removal");
  assert.deepEqual(Buffer.from(await afterShutdownAndCleanup.arrayBuffer()), png);
} finally {
  await close(server);
  fs.rmSync(root, { recursive: true, force: true });
}

const validationRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-preview-validation-"));
const validationArtifacts = path.join(validationRoot, "target", "interact", "lab", "lab_test", "captures");
fs.mkdirSync(validationArtifacts, { recursive: true });
const validationPng = path.join(validationArtifacts, "scene.png");
fs.writeFileSync(validationPng, png);
const validationPreview = new InteractTailnetPreview({
  workspaceRoot: validationRoot,
  publishArtifact: async () => ({ url: "http://127.0.0.1:8091/p/fixture/scene.png", expiresAt: Date.now() + INTERACT_PREVIEW_TTL_MS }),
});
try {
  assert.throws(
    () => new InteractTailnetPreview({ workspaceRoot: validationRoot, ttlMs: INTERACT_PREVIEW_TTL_MS - 1 }),
    (error) => error?.code === "invalidPreviewTtl",
    "Lab preview retention cannot be configured below 24 hours",
  );
  await assert.rejects(
    validationPreview.publish({ filePath: "/etc/passwd", mimeType: "image/png" }),
    (error) => error?.code === "unsafePreviewArtifact",
    "preview rejects files outside target/interact",
  );
  await assert.rejects(
    validationPreview.publish({ filePath: validationPng, mimeType: "text/html" }),
    (error) => error?.code === "invalidPreviewMimeType",
    "preview cannot publish arbitrary MIME types",
  );
  const oversized = path.join(validationArtifacts, "oversized.mp4");
  fs.writeFileSync(oversized, Buffer.alloc(1));
  fs.truncateSync(oversized, MAX_TAILNET_PREVIEW_ARTIFACT_BYTES + 1);
  await assert.rejects(
    validationPreview.publish({ filePath: oversized, mimeType: "video/mp4" }),
    (error) => error?.code === "previewArtifactTooLarge",
    "preview rejects artifacts above the delivery bound before copying",
  );
} finally {
  await validationPreview.close();
  fs.rmSync(validationRoot, { recursive: true, force: true });
}

console.log("✅ interact_tailnet_preview_contracts.mjs: durable 24-hour Tailnet artifact delivery passed");

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
