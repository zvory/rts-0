#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const clientSrc = path.join(repoRoot, "client/src");

const AREA_BY_FILE = new Map(Object.entries({
  "main.js": "app-shell",
  "app.js": "app-shell",
  "match.js": "app-shell",
  "match_health.js": "app-shell",
  "observer_analysis_overlay.js": "app-shell",
  "replay_controls.js": "app-shell",
  "replay_viewer.js": "app-shell",

  "state.js": "model",
  "client_intent.js": "model",
  "command_budget.js": "model",
  "command_composer.js": "model",
  "progress_extrapolator.js": "model",
  "prediction_controller.js": "model",
  "prediction_compatibility.js": "model",
  "sim_wasm_adapter.js": "model",
  "prediction_settings.js": "platform",

  "net.js": "transport",
  "protocol.js": "transport",

  "config.js": "rules-mirror",

  "hud.js": "ui",
  "hud_command_card.js": "ui",
  "hotkey_editor.js": "ui",
  "hotkey_profiles.js": "ui",
  "lobby.js": "ui",
  "lobby_view.js": "ui",
  "match_history.js": "ui",
  "resource_icons.js": "ui",
  "status_badge.js": "ui",
  "minimap.js": "ui",
  "branch_staging.js": "ui",
  "settings_container.js": "ui",
  "settings_panels.js": "ui",
  "scoreboard.js": "ui",

  "replay_camera_input.js": "input",

  "bootstrap.js": "platform",
  "audio.js": "platform",
  "combat_audio.js": "platform",
  "alerts.js": "platform",
  "fog.js": "platform",
  "camera.js": "platform",
}));

const AREA_PREFIXES = [
  ["renderer/", "renderer"],
  ["input/", "input"],
];

const ALLOWED_CROSS_AREA_IMPORTS = new Map();

const ALLOWED_PROTOTYPE_GRAFTS = new Set([
  "input/index.js:Input",
  "renderer/index.js:Renderer",
]);

// Phase 6 client-boundary ratchet: these are the cleanup-phase byte counts for
// the largest modules. Future growth should either extract a focused helper or
// update this table with the phase-specific reason.
const LARGE_FILE_BASELINES = new Map(Object.entries({
  // Tank Trap Phase 5 extends placement feedback to draw multi-site line previews.
  "renderer/feedback.js": 46315,
  // Hold Position adds a distinct command-card intent dispatch while preserving legacy Stop.
  "hud.js": 44080,
  "state.js": 38576,
  // Tank Trap Phase 5 adds placement-drag lifecycle hooks while line math lives in input/tank_trap_line.js.
  "input/index.js": 38854,
  "match.js": 38289,
  // Lab MVP Phase 3 adds mirrored lab request/result tags, vision modes, and typed request builders.
  "protocol.js": 37805,
  // Hold Position moves the unit hold button to the W grid slot with a distinct command id.
  "hud_command_card.js": 29393,
  "renderer/shared.js": 28113,
  "observer_analysis_overlay.js": 27903,
  "audio.js": 27339,
}));

const FORBIDDEN_GAMESTATE_INTENT_SHIMS = [
  "commandTarget",
  "placement",
  "commandCardMode",
  "resourceMiningPreview",
  "antiTankGunSetupPreview",
  "abilityTargetPreview",
  "liveCommandFeedback",
  "openWorkerBuildMenu",
  "closeCommandCardMenu",
  "beginPlacement",
  "updatePlacement",
  "endPlacement",
  "beginCommandTarget",
  "endCommandTarget",
  "holdCommandTarget",
  "issueCommandTarget",
  "releaseCommandTargetKey",
  "releaseCommandTargetShift",
  "updateResourceMiningPreview",
  "updateAntiTankGunSetupPreview",
  "updateAbilityTargetPreview",
];

const importRe = /\bimport\s+(?:[\s\S]*?\s+from\s+)?["']([^"']+)["']/g;
const prototypeGraftRe = /\bObject\.assign\s*\(\s*([A-Za-z_$][\w$]*)\.prototype\s*,/g;

const failures = [];
const warnings = [];

const files = listJsFiles(clientSrc);
const fileSet = new Set(files);
const modules = new Map();

for (const file of files) {
  const abs = path.join(clientSrc, file);
  const source = fs.readFileSync(abs, "utf8");
  const area = classify(file);
  if (!area) {
    failures.push(`${file}: missing client architecture area classification`);
  }
  modules.set(file, {
    file,
    area,
    bytes: Buffer.byteLength(source, "utf8"),
    imports: parseImports(source, file),
    prototypeGrafts: parsePrototypeGrafts(source, file),
    fanIn: 0,
  });
  checkLargeFileBaseline(file, source);
  checkForbiddenGameStateIntentShims(file, source);
}

for (const mod of modules.values()) {
  for (const specifier of mod.imports) {
    if (!specifier.startsWith(".")) continue;
    const resolved = resolveImport(mod.file, specifier);
    if (!resolved) {
      failures.push(`${mod.file}: cannot resolve import ${JSON.stringify(specifier)}`);
      continue;
    }
    if (!fileSet.has(resolved)) {
      failures.push(`${mod.file}: import ${JSON.stringify(specifier)} resolves outside client/src JS modules (${resolved})`);
      continue;
    }
    modules.get(resolved).fanIn += 1;
    checkImport(mod.file, resolved);
  }

  for (const className of mod.prototypeGrafts) {
    const key = `${mod.file}:${className}`;
    if (!ALLOWED_PROTOTYPE_GRAFTS.has(key)) {
      failures.push(`${mod.file}: unexpected Object.assign(${className}.prototype, ...) facade graft`);
    }
  }
}

emitMetrics();

if (warnings.length > 0) {
  console.warn("\nclient architecture warnings:");
  for (const warning of warnings) console.warn(`  - ${warning}`);
}

if (failures.length > 0) {
  console.error("\nclient architecture check failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log("\nclient architecture check passed");

function classify(file) {
  for (const [prefix, area] of AREA_PREFIXES) {
    if (file.startsWith(prefix)) return area;
  }
  return AREA_BY_FILE.get(file) ?? null;
}

function listJsFiles(dir) {
  const out = [];
  walk(dir, "");
  return out.sort();

  function walk(absDir, relDir) {
    for (const entry of fs.readdirSync(absDir, { withFileTypes: true })) {
      const rel = relDir ? `${relDir}/${entry.name}` : entry.name;
      const abs = path.join(absDir, entry.name);
      if (entry.isDirectory()) {
        walk(abs, rel);
      } else if (entry.isFile() && entry.name.endsWith(".js")) {
        out.push(rel);
      }
    }
  }
}

function parseImports(source) {
  const imports = [];
  for (const match of source.matchAll(importRe)) {
    imports.push(match[1]);
  }
  return imports;
}

function parsePrototypeGrafts(source) {
  const grafts = [];
  for (const match of source.matchAll(prototypeGraftRe)) {
    grafts.push(match[1]);
  }
  return grafts;
}

function checkLargeFileBaseline(file, source) {
  const baseline = LARGE_FILE_BASELINES.get(file);
  if (baseline == null) return;
  const bytes = Buffer.byteLength(source, "utf8");
  if (bytes > baseline) {
    failures.push(`${file}: ${bytes} bytes exceeds large-file baseline ${baseline}; extract a focused helper or update the ratchet with a reason`);
  }
}

function checkForbiddenGameStateIntentShims(file, source) {
  for (const name of FORBIDDEN_GAMESTATE_INTENT_SHIMS) {
    const directStateRe = new RegExp(`(?:\\bstate|\\bthis\\.state)(?:\\.${name}\\b|\\?\\.${name}\\b)`);
    if (directStateRe.test(source)) {
      failures.push(`${file}: forbidden GameState intent shim reference ${name}; use injected ClientIntent or a narrow view model`);
    }
  }
}

function resolveImport(fromFile, specifier) {
  const fromDir = path.dirname(fromFile);
  let resolved = path.normalize(path.join(fromDir, specifier)).split(path.sep).join("/");
  if (!resolved.endsWith(".js")) resolved = `${resolved}.js`;
  if (resolved.startsWith("../") || path.isAbsolute(resolved)) return null;
  return resolved;
}

function checkImport(fromFile, toFile) {
  const from = modules.get(fromFile);
  const to = modules.get(toFile);
  if (!from?.area || !to?.area) return;
  if (toFile === "protocol.js" || toFile === "config.js") return;
  if (from.area === "app-shell") return;
  if (from.area === to.area) return;

  const key = `${fromFile} -> ${toFile}`;
  const reason = ALLOWED_CROSS_AREA_IMPORTS.get(key);
  if (!reason) {
    failures.push(`${key}: ${from.area} may not import ${to.area} without an allowlist reason`);
    return;
  }
  if (from.area === "model" && to.area === "input") {
    warnings.push(`${key}: grandfathered model -> input dependency: ${reason}`);
  }
}

function emitMetrics() {
  const rows = [...modules.values()]
    .map((mod) => ({
      file: mod.file,
      area: mod.area ?? "unclassified",
      bytes: mod.bytes,
      fanIn: mod.fanIn,
      fanOut: mod.imports.filter((specifier) => specifier.startsWith(".")).length,
    }))
    .sort((a, b) => b.bytes - a.bytes || a.file.localeCompare(b.file));

  console.log("client architecture baseline:");
  console.log("  largest files:");
  for (const row of rows.slice(0, 10)) {
    console.log(`    ${row.file.padEnd(28)} ${String(row.bytes).padStart(6)} bytes  fan-in ${String(row.fanIn).padStart(2)}  fan-out ${String(row.fanOut).padStart(2)}  ${row.area}`);
  }

  console.log("  highest fan-in:");
  for (const row of [...rows].sort((a, b) => b.fanIn - a.fanIn || a.file.localeCompare(b.file)).slice(0, 10)) {
    console.log(`    ${row.file.padEnd(28)} fan-in ${String(row.fanIn).padStart(2)}  fan-out ${String(row.fanOut).padStart(2)}  ${row.area}`);
  }

  console.log("  highest fan-out:");
  for (const row of [...rows].sort((a, b) => b.fanOut - a.fanOut || a.file.localeCompare(b.file)).slice(0, 10)) {
    console.log(`    ${row.file.padEnd(28)} fan-out ${String(row.fanOut).padStart(2)}  fan-in ${String(row.fanIn).padStart(2)}  ${row.area}`);
  }
}
