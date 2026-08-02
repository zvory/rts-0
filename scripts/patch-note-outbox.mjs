#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { PNG } from "pngjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, "..");
const MAX_CONTENT_CHARS = 1_800;
const MAX_MEDIA_BYTES = Math.floor(9.5 * 1024 * 1024);
const MAX_SOURCE_BYTES = 64 * 1024 * 1024;
const MAX_PIXELS = 16_777_216;
const LABEL_SCALE = 4;

const GLYPHS = {
  A: ["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
  B: ["11110", "10001", "10001", "11110", "10001", "10001", "11110"],
  E: ["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
  F: ["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
  O: ["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
  R: ["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
  T: ["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
};

function usage() {
  return `Usage:
  node scripts/patch-note-outbox.mjs stage --change TEXT [--change TEXT ...]
      [--headline TEXT] [--before PNG --after PNG] [--repo DIR]
  node scripts/patch-note-outbox.mjs compose --before PNG --after PNG --output MP4
  node scripts/patch-note-outbox.mjs deliver --branch BRANCH [--repo DIR]
  node scripts/patch-note-outbox.mjs status [--branch BRANCH] [--repo DIR]

Staging is optional. With --before and --after, the helper creates a four-second labeled MP4.
Delivery reads webhook URLs from the environment or the ignored primary-checkout .env file.`;
}

export function parseArgs(argv) {
  const options = {
    command: "", repoRoot: defaultRepoRoot, branch: "", headline: "", changes: [],
    before: "", after: "", output: "", ffmpeg: process.env.RTS_PATCH_NOTES_FFMPEG || "ffmpeg",
    ffprobe: process.env.RTS_PATCH_NOTES_FFPROBE || "ffprobe", help: false,
  };
  if (argv[0] && !argv[0].startsWith("-")) options.command = argv.shift();
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = () => {
      index += 1;
      if (index >= argv.length || argv[index].startsWith("--")) throw new Error(`${arg} requires a value`);
      return argv[index];
    };
    if (arg === "-h" || arg === "--help") options.help = true;
    else if (arg === "--repo") options.repoRoot = path.resolve(value());
    else if (arg === "--branch") options.branch = value();
    else if (arg === "--headline") options.headline = singleLine(value());
    else if (arg === "--change") options.changes.push(singleLine(value()));
    else if (arg === "--before") options.before = resolveSource(value());
    else if (arg === "--after") options.after = resolveSource(value());
    else if (arg === "--output") options.output = path.resolve(value());
    else if (arg === "--ffmpeg") options.ffmpeg = value();
    else if (arg === "--ffprobe") options.ffprobe = value();
    else throw new Error(`unknown argument: ${arg}`);
  }
  return options;
}

function singleLine(value) {
  return String(value || "").replace(/\s+/g, " ").trim();
}

function resolveSource(value) {
  return /^https?:\/\//i.test(value) ? value : path.resolve(value);
}

function run(command, args, { cwd, input } = {}) {
  const result = spawnSync(command, args, { cwd, input, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(result.stderr?.trim() || result.stdout?.trim() || `${command} exited ${result.status}`);
  }
  return result.stdout?.trim() || "";
}

function git(repoRoot, args) {
  return run("git", args, { cwd: repoRoot });
}

export function branchSlug(branch) {
  return branch.replace(/^zvorygin\//, "").replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "") || "change";
}

function branchOutboxKey(branch) {
  const digest = crypto.createHash("sha256").update(branch).digest("hex");
  return `${branchSlug(branch).slice(0, 80)}-${digest}`;
}

export function gitCommonDir(repoRoot) {
  return path.resolve(repoRoot, git(repoRoot, ["rev-parse", "--git-common-dir"]));
}

export function outboxPath(repoRoot, branch) {
  return path.join(gitCommonDir(repoRoot), "rts-patch-note-outbox", branchOutboxKey(branch));
}

function requireRegularFile(filename, label, maxBytes = MAX_SOURCE_BYTES) {
  const stat = fs.lstatSync(filename, { throwIfNoEntry: false });
  if (!stat?.isFile() || stat.isSymbolicLink()) throw new Error(`${label} must be a regular file: ${filename}`);
  if (stat.size <= 0 || stat.size > maxBytes) throw new Error(`${label} must be between 1 and ${maxBytes} bytes`);
  return stat;
}

function readPng(filename, label) {
  requireRegularFile(filename, label);
  const contents = fs.readFileSync(filename);
  const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (
    contents.length < 24 ||
    !contents.subarray(0, pngSignature.length).equals(pngSignature) ||
    contents.toString("ascii", 12, 16) !== "IHDR"
  ) {
    throw new Error(`${label} is not a readable PNG: missing PNG signature or IHDR`);
  }
  const declaredWidth = contents.readUInt32BE(16);
  const declaredHeight = contents.readUInt32BE(20);
  if (
    declaredWidth < 2 || declaredHeight < 2 ||
    declaredWidth > Math.floor(MAX_PIXELS / declaredHeight)
  ) {
    throw new Error(`${label} dimensions are outside the supported capture bound`);
  }
  let png;
  try { png = PNG.sync.read(contents); }
  catch (error) { throw new Error(`${label} is not a readable PNG: ${error.message}`); }
  if (png.width !== declaredWidth || png.height !== declaredHeight) {
    throw new Error(`${label} dimensions are outside the supported capture bound`);
  }
  return png;
}

function blendPixel(png, x, y, red, green, blue, alpha) {
  if (x < 0 || y < 0 || x >= png.width || y >= png.height) return;
  const offset = (y * png.width + x) * 4;
  const sourceAlpha = alpha / 255;
  png.data[offset] = Math.round(red * sourceAlpha + png.data[offset] * (1 - sourceAlpha));
  png.data[offset + 1] = Math.round(green * sourceAlpha + png.data[offset + 1] * (1 - sourceAlpha));
  png.data[offset + 2] = Math.round(blue * sourceAlpha + png.data[offset + 2] * (1 - sourceAlpha));
  png.data[offset + 3] = 255;
}

function labelPng(png, text) {
  const padding = 4 * LABEL_SCALE;
  const glyphWidth = 5 * LABEL_SCALE;
  const gap = LABEL_SCALE;
  const width = padding * 2 + text.length * glyphWidth + (text.length - 1) * gap;
  const height = padding * 2 + 7 * LABEL_SCALE;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) blendPixel(png, x, y, 0, 0, 0, 205);
  }
  let originX = padding;
  for (const character of text) {
    const glyph = GLYPHS[character];
    if (!glyph) throw new Error(`unsupported label character: ${character}`);
    for (let row = 0; row < glyph.length; row += 1) {
      for (let column = 0; column < glyph[row].length; column += 1) {
        if (glyph[row][column] !== "1") continue;
        for (let dy = 0; dy < LABEL_SCALE; dy += 1) {
          for (let dx = 0; dx < LABEL_SCALE; dx += 1) {
            blendPixel(png, originX + column * LABEL_SCALE + dx, padding + row * LABEL_SCALE + dy, 255, 255, 255, 255);
          }
        }
      }
    }
    originX += glyphWidth + gap;
  }
  return png;
}

function probeComposedMedia(filename, ffprobe) {
  const raw = run(ffprobe, [
    "-v", "error", "-select_streams", "v:0", "-show_entries",
    "stream=codec_name,width,height:format=duration,size", "-of", "json", filename,
  ]);
  const probe = JSON.parse(raw);
  const stream = probe.streams?.[0];
  const duration = Number(probe.format?.duration);
  const bytes = Number(probe.format?.size);
  if (stream?.codec_name !== "h264" || !Number.isInteger(stream.width) || !Number.isInteger(stream.height)) {
    throw new Error("composed media is not a valid H.264 video");
  }
  if (!Number.isFinite(duration) || duration < 3.9 || duration > 4.1) {
    throw new Error(`composed media duration must be four seconds, got ${probe.format?.duration}`);
  }
  if (!Number.isFinite(bytes) || bytes <= 0 || bytes > MAX_MEDIA_BYTES) {
    throw new Error(`composed media exceeds the ${MAX_MEDIA_BYTES}-byte Discord target`);
  }
  return { bytes, codec: stream.codec_name, duration, width: stream.width, height: stream.height };
}

export function composeBeforeAfter({ before, after, output, ffmpeg = "ffmpeg", ffprobe = "ffprobe" }) {
  if (!before || !after || !output) throw new Error("compose requires --before, --after, and --output");
  fs.mkdirSync(path.dirname(output), { recursive: true });
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "rts-patch-note-compose-"));
  const beforeLabeled = path.join(tempDir, "before.png");
  const afterLabeled = path.join(tempDir, "after.png");
  try {
    const beforeSource = materializeSource(before, path.join(tempDir, "before-source.png"));
    const afterSource = materializeSource(after, path.join(tempDir, "after-source.png"));
    const beforePng = readPng(beforeSource, "before capture");
    const afterPng = readPng(afterSource, "after capture");
    if (beforePng.width !== afterPng.width || beforePng.height !== afterPng.height) {
      throw new Error(`before/after captures must have identical dimensions (${beforePng.width}x${beforePng.height} vs ${afterPng.width}x${afterPng.height})`);
    }
    fs.writeFileSync(beforeLabeled, PNG.sync.write(labelPng(beforePng, "BEFORE")));
    fs.writeFileSync(afterLabeled, PNG.sync.write(labelPng(afterPng, "AFTER")));
    run(ffmpeg, [
      "-hide_banner", "-loglevel", "error", "-y",
      "-loop", "1", "-t", "2", "-i", beforeLabeled,
      "-loop", "1", "-t", "2", "-i", afterLabeled,
      "-filter_complex", "[0:v]fps=30,format=yuv420p[b];[1:v]fps=30,format=yuv420p[a];[b][a]concat=n=2:v=1:a=0,pad=ceil(iw/2)*2:ceil(ih/2)*2[v]",
      "-map", "[v]", "-an", "-c:v", "libx264", "-preset", "veryfast", "-crf", "23",
      "-pix_fmt", "yuv420p", "-tag:v", "avc1", "-movflags", "+faststart", output,
    ]);
    return probeComposedMedia(output, ffprobe);
  } catch (error) {
    fs.rmSync(output, { force: true });
    throw error;
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

function materializeSource(source, destination) {
  if (!/^https?:\/\//i.test(source)) return source;
  run("curl", [
    "--fail", "--silent", "--show-error", "--max-time", "20", "--max-filesize", String(MAX_SOURCE_BYTES),
    "--proto", "=http,https", "--output", destination, "--", source,
  ]);
  return destination;
}

export function renderContent({ headline = "", changes }) {
  const lines = [];
  if (headline) lines.push(`**${singleLine(headline)}**`);
  lines.push(...changes.map((change) => `• ${singleLine(change).replace(/^[-*•]\s*/, "")}`));
  const content = lines.filter(Boolean).join("\n");
  if (!content || content.length > MAX_CONTENT_CHARS) {
    throw new Error(`patch-note content must be between 1 and ${MAX_CONTENT_CHARS} characters`);
  }
  return content;
}

function sha256File(filename) {
  return crypto.createHash("sha256").update(fs.readFileSync(filename)).digest("hex");
}

export function stagePatchNote(options) {
  const branch = options.branch || git(options.repoRoot, ["branch", "--show-current"]);
  if (!branch) throw new Error("stage requires a branch checkout or --branch");
  const changes = options.changes.map(singleLine).filter(Boolean);
  if (changes.length === 0) throw new Error("stage requires at least one --change");
  if (Boolean(options.before) !== Boolean(options.after)) throw new Error("--before and --after must be supplied together");
  const content = renderContent({ headline: options.headline, changes });
  const destination = outboxPath(options.repoRoot, branch);
  const outboxRoot = path.dirname(destination);
  fs.mkdirSync(outboxRoot, { recursive: true, mode: 0o700 });
  const temporary = fs.mkdtempSync(path.join(outboxRoot, `.${branchSlug(branch)}-`));
  try {
    let media = null;
    if (options.before) {
      const mediaPath = path.join(temporary, "before-after.mp4");
      const probe = composeBeforeAfter({
        before: options.before, after: options.after, output: mediaPath,
        ffmpeg: options.ffmpeg, ffprobe: options.ffprobe,
      });
      media = {
        filename: "before-after.mp4", description: "Before and after comparison", bytes: probe.bytes,
        sha256: sha256File(mediaPath), durationSeconds: probe.duration,
      };
    }
    const manifest = { version: 1, branch, headline: options.headline, changes, content, media };
    fs.writeFileSync(path.join(temporary, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, { mode: 0o600 });
    fs.rmSync(destination, { recursive: true, force: true });
    fs.renameSync(temporary, destination);
    return { branch, destination, manifest };
  } catch (error) {
    fs.rmSync(temporary, { recursive: true, force: true });
    throw error;
  }
}

export function parseEnvValue(contents, name) {
  for (const line of contents.split(/\r?\n/)) {
    const match = line.match(/^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$/);
    if (!match || match[1] !== name) continue;
    const value = match[2];
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) return value.slice(1, -1);
    return value;
  }
  return "";
}

function resolveWebhook(repoRoot, name, env = process.env) {
  if (env[name]?.trim()) return env[name].trim();
  const primaryCheckout = path.dirname(gitCommonDir(repoRoot));
  for (const filename of [path.join(repoRoot, ".env"), path.join(primaryCheckout, ".env")]) {
    if (!fs.existsSync(filename)) continue;
    const value = parseEnvValue(fs.readFileSync(filename, "utf8"), name).trim();
    if (value) return value;
  }
  return "";
}

export function resolveWebhooks(repoRoot, env = process.env) {
  return [...new Set([
    resolveWebhook(repoRoot, "RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL", env),
    resolveWebhook(repoRoot, "RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL_SECONDARY", env),
  ].filter(Boolean))];
}

function validateWebhookUrl(value) {
  const url = new URL(value);
  const allowedHost = url.hostname === "discord.com" || url.hostname === "discordapp.com";
  if (url.protocol !== "https:" || !allowedHost || !/^\/api\/webhooks\/[^/]+\/[^/]+\/?$/.test(url.pathname)) {
    throw new Error("Discord webhook URL must use the official HTTPS webhook endpoint");
  }
  url.searchParams.set("wait", "true");
  return url;
}

export async function postDiscord({ content, mediaPath = "", mediaDescription = "", webhookUrl, fetchImpl = globalThis.fetch }) {
  const url = validateWebhookUrl(webhookUrl);
  const payload = { content, allowed_mentions: { parse: [] } };
  const form = new FormData();
  if (mediaPath) {
    const bytes = fs.readFileSync(mediaPath);
    payload.attachments = [{ id: 0, filename: path.basename(mediaPath), description: mediaDescription }];
    form.append("files[0]", new Blob([bytes], { type: "video/mp4" }), path.basename(mediaPath));
  }
  form.append("payload_json", JSON.stringify(payload));
  const response = await fetchImpl(url, {
    method: "POST", body: form, redirect: "error", signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`Discord rejected the patch note (${response.status} ${response.statusText})`);
  return response.json().catch(() => ({}));
}

function loadManifest(repoRoot, branch) {
  const directory = outboxPath(repoRoot, branch);
  const manifestPath = path.join(directory, "manifest.json");
  if (!fs.existsSync(manifestPath)) return null;
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (manifest?.version !== 1 || manifest.branch !== branch || !Array.isArray(manifest.changes)) {
    throw new Error(`invalid patch-note outbox manifest for ${branch}`);
  }
  if (renderContent(manifest) !== manifest.content) throw new Error("patch-note manifest content does not match its fields");
  let mediaPath = "";
  if (manifest.media) {
    if (manifest.media.filename !== "before-after.mp4") throw new Error("unsupported patch-note media filename");
    mediaPath = path.join(directory, manifest.media.filename);
    const stat = requireRegularFile(mediaPath, "patch-note media", MAX_MEDIA_BYTES);
    if (stat.size !== manifest.media.bytes || sha256File(mediaPath) !== manifest.media.sha256) {
      throw new Error("patch-note media no longer matches its staged manifest");
    }
  }
  return { directory, manifest, mediaPath };
}

export async function deliverPatchNote({ repoRoot, branch, env = process.env, post = postDiscord }) {
  const staged = loadManifest(repoRoot, branch);
  if (!staged) return { status: "not-staged", branch };
  const webhooks = resolveWebhooks(repoRoot, env);
  if (webhooks.length === 0) return { status: "not-configured", branch };
  let delivered = 0;
  for (const webhook of webhooks) {
    const destinationKey = crypto.createHash("sha256").update(webhook).digest("hex");
    const digest = crypto.createHash("sha256")
      .update(webhook).update("\0").update(staged.manifest.content).update("\0")
      .update(staged.manifest.media?.sha256 || "").digest("hex");
    const receiptPath = path.join(staged.directory, `destination-${destinationKey}.receipt`);
    if (fs.existsSync(receiptPath) && fs.readFileSync(receiptPath, "utf8").trim().split(/\s+/)[0] === digest) continue;
    const response = await post({
      content: staged.manifest.content, mediaPath: staged.mediaPath,
      mediaDescription: staged.manifest.media?.description || "", webhookUrl: webhook,
    });
    fs.writeFileSync(receiptPath, `${digest} ${singleLine(response?.id || "accepted")}\n`, { mode: 0o600 });
    delivered += 1;
  }
  fs.rmSync(staged.directory, { recursive: true, force: true });
  return { status: delivered > 0 ? "sent" : "already-sent", branch, destinations: webhooks.length };
}

export async function execute(options) {
  if (options.help || !options.command) {
    process.stdout.write(`${usage()}\n`);
    return null;
  }
  if (options.command === "compose") {
    const result = composeBeforeAfter(options);
    process.stdout.write(`patch-note: composed ${options.output} (${result.bytes} bytes)\n`);
    return result;
  }
  if (options.command === "stage") {
    const result = stagePatchNote(options);
    process.stdout.write(`patch-note: staged ${result.manifest.changes.length} change(s) for ${result.branch}${result.manifest.media ? " with before/after media" : ""}\n`);
    return result;
  }
  const branch = options.branch || git(options.repoRoot, ["branch", "--show-current"]);
  if (!branch) throw new Error(`${options.command} requires --branch or a branch checkout`);
  if (options.command === "status") {
    const staged = loadManifest(options.repoRoot, branch);
    process.stdout.write(staged ? `${JSON.stringify(staged.manifest, null, 2)}\n` : `patch-note: nothing staged for ${branch}\n`);
    return staged;
  }
  if (options.command === "deliver") {
    const result = await deliverPatchNote({ repoRoot: options.repoRoot, branch });
    process.stdout.write(`patch-note: ${result.status} for ${branch}\n`);
    return result;
  }
  throw new Error(`unknown command: ${options.command}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  execute(parseArgs(process.argv.slice(2))).catch((error) => {
    process.stderr.write(`patch-note: ${error.message}\n`);
    process.exitCode = 1;
  });
}
