import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  LabInteractTailnetPreview,
  LAB_INTERACT_PREVIEW_ROUTE,
  MAX_TAILNET_PREVIEW_ARTIFACT_BYTES,
  isTailnetIpv4,
  resolveTailnetHost,
  tailnetHostFromStatus,
} from "../scripts/lab-interact/tailnet_preview.mjs";

const root = fs.mkdtempSync(path.join(os.tmpdir(), "rts-li-tailnet-preview-"));
const artifactRoot = path.join(root, "target", "lab-interact", "lab_test", "captures");
fs.mkdirSync(artifactRoot, { recursive: true });
const pngPath = path.join(artifactRoot, "scene.png");
const png = Buffer.from("fixture-png-bytes");
fs.writeFileSync(pngPath, png);
let accesses = 0;
const preview = new LabInteractTailnetPreview({
  workspaceRoot: root,
  host: "127.0.0.1",
  onAccess: () => { accesses += 1; },
});

try {
  const published = await preview.publish({ filePath: pngPath, mimeType: "image/png" });
  assert.match(published.url, new RegExp(`^http://127\\.0\\.0\\.1:\\d+${LAB_INTERACT_PREVIEW_ROUTE}[a-f0-9]{64}$`), "preview URL is a bound loopback/Tailnet listener plus an opaque token");
  assert.equal(published.url.includes(root), false, "preview URL does not disclose the workspace path");
  assert.equal(published.bytes, png.length, "preview reports bounded artifact bytes");
  assert.equal((await preview.publish({ filePath: pngPath, mimeType: "image/png" })).url, published.url, "the same immutable artifact keeps one stable URL");

  const image = await fetch(published.url);
  assert.equal(image.status, 200, "registered preview serves an artifact");
  assert.equal(image.headers.get("content-type"), "image/png", "registered preview preserves explicit MIME type");
  assert.equal(image.headers.get("cache-control"), "private, no-store, max-age=0", "preview responses stay private and uncacheable");
  assert.deepEqual(Buffer.from(await image.arrayBuffer()), png, "preview response contains exactly the registered artifact bytes");

  const head = await fetch(published.url, { method: "HEAD" });
  assert.equal(head.status, 200, "preview accepts HEAD without returning a body");
  assert.equal(head.headers.get("content-length"), String(png.length), "HEAD retains the exact artifact length");

  const range = await fetch(published.url, { headers: { Range: "bytes=2-7" } });
  assert.equal(range.status, 206, "preview supports video/image byte ranges");
  assert.equal(range.headers.get("content-range"), `bytes 2-7/${png.length}`, "range response is bounded to the requested bytes");
  assert.deepEqual(Buffer.from(await range.arrayBuffer()), png.subarray(2, 8), "range response streams the selected slice");
  assert.ok(accesses >= 3, "successful preview reads extend the Lab daemon's idle lease");

  const invalidRange = await fetch(published.url, { headers: { Range: "bytes=999-1000" } });
  assert.equal(invalidRange.status, 416, "out-of-bounds ranges are rejected without exposing the file");
  const unknown = await fetch(new URL(`${LAB_INTERACT_PREVIEW_ROUTE}${"a".repeat(64)}`, published.url));
  assert.equal(unknown.status, 404, "unguessable URLs are the only allowed artifact selector");
  const post = await fetch(published.url, { method: "POST" });
  assert.equal(post.status, 405, "preview server is read-only");

  await assert.rejects(
    preview.publish({ filePath: "/etc/passwd", mimeType: "image/png" }),
    (error) => error?.code === "unsafePreviewArtifact",
    "preview rejects files outside target/lab-interact",
  );
  await assert.rejects(
    preview.publish({ filePath: pngPath, mimeType: "text/html" }),
    (error) => error?.code === "invalidPreviewMimeType",
    "preview cannot be repurposed as arbitrary file serving",
  );

  fs.writeFileSync(pngPath, Buffer.from("replacement"));
  const changed = await fetch(published.url);
  assert.equal(changed.status, 404, "a replaced artifact invalidates its old preview token");
} finally {
  await preview.close();
  fs.rmSync(root, { recursive: true, force: true });
}

assert.equal(isTailnetIpv4("100.64.0.1"), true, "the lower Tailscale IPv4 bound is accepted");
assert.equal(isTailnetIpv4("100.127.255.255"), true, "the upper Tailscale IPv4 bound is accepted");
assert.equal(isTailnetIpv4("100.128.0.1"), false, "non-Tailscale CGNAT-adjacent addresses are rejected");
assert.equal(tailnetHostFromStatus({ TailscaleIPs: ["fd7a::1"], Self: { TailscaleIPs: ["100.119.17.21"] } }), "100.119.17.21", "status resolves a usable IPv4 Tailnet listener");
assert.equal(resolveTailnetHost({ env: { RTS_LAB_INTERACT_TEST_TAILNET_PREVIEW_HOST: "127.0.0.1" } }), "127.0.0.1", "test-only loopback override avoids external Tailscale dependencies");
assert.equal(resolveTailnetHost({
  env: {},
  readStatus: () => ({ TailscaleIPs: ["100.119.17.21"] }),
}), "100.119.17.21", "normal resolution reads the active Tailnet IP from status");
assert.throws(
  () => resolveTailnetHost({ env: {}, readStatus: () => ({ TailscaleIPs: [] }) }),
  (error) => error?.code === "tailnetUnavailable",
  "a missing Tailscale address has an actionable preview failure",
);
assert.equal(MAX_TAILNET_PREVIEW_ARTIFACT_BYTES, 64 * 1024 * 1024, "Tailnet preview matches Lab's recording size cap");

console.log("✅ lab_interact_tailnet_preview_contracts.mjs: opaque Tailnet artifact delivery passed");
