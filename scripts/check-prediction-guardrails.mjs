#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(new URL("..", import.meta.url).pathname);

const predictionJsFiles = [
  "client/src/prediction_controller.js",
  "client/src/sim_wasm_adapter.js",
  "client/src/prediction_settings.js",
  "client/src/state.js",
  "tests/tri_state/lanes/local_lane.mjs",
];

const forbiddenJsImports = [
  "match_history",
  "replay_viewer",
  "replay_controls",
  "dev_scenarios",
  "full_world",
  "full-world",
  "server/",
  "server\\",
];

const wasmCargo = "server/crates/sim-wasm/Cargo.toml";
const wasmRustFiles = ["server/crates/sim-wasm/src/lib.rs"];
const forbiddenWasmDeps = [
  "rts-ai",
  "rts-server",
  "tokio",
  "axum",
  "tower-http",
  "sqlx",
  "dotenvy",
  "tracing-subscriber",
];
const forbiddenWasmSource = [
  "snapshot_for_all",
  "snapshot_full",
  "full_world",
  "full-world",
  "new_dev_scenario",
  "match_history",
  "sqlx::",
  "rts_ai::",
  "rts_server::",
  "ReplayArtifact",
];

const failures = [];

for (const file of predictionJsFiles) {
  const source = read(file);
  checkImports(file, source);
}

const cargo = read(wasmCargo);
for (const dep of forbiddenWasmDeps) {
  if (new RegExp(`(^|\\n)\\s*${escapeRegExp(dep)}\\s*=`, "m").test(cargo)) {
    failures.push(`${wasmCargo}: forbidden browser-prediction dependency ${dep}`);
  }
}

for (const file of wasmRustFiles) {
  const source = read(file);
  for (const token of forbiddenWasmSource) {
    if (source.includes(token)) {
      failures.push(`${file}: forbidden browser-prediction source reference ${JSON.stringify(token)}`);
    }
  }
}

if (failures.length > 0) {
  console.error("prediction guardrail check failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log("prediction guardrail check passed");

function checkImports(file, source) {
  const importRe = /\bimport\s+(?:[\s\S]*?\s+from\s+)?["']([^"']+)["']/g;
  for (const match of source.matchAll(importRe)) {
    const specifier = match[1];
    const normalized = specifier.replace(/\\/g, "/").toLowerCase();
    for (const forbidden of forbiddenJsImports) {
      if (normalized.includes(forbidden)) {
        failures.push(`${file}: forbidden prediction import ${JSON.stringify(specifier)}`);
      }
    }
  }
}

function read(file) {
  const fullPath = path.join(repoRoot, file);
  if (!existsSync(fullPath)) {
    failures.push(`${file}: missing guardrail input`);
    return "";
  }
  return readFileSync(fullPath, "utf8");
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
