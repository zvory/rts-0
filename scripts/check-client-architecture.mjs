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
  "match_lab_tools.js": "app-shell",
  "match_combat_audio.js": "app-shell",
  "match_notice_presenter.js": "app-shell",
  "match_pointer_lock_diagnostics.js": "app-shell",
  "match_live_pause.js": "app-shell",
  "client_perf_report.js": "app-shell",
  "match_net_reporter.js": "app-shell",
  "match_observer_diagnostics.js": "app-shell",
  "match_settings_context.js": "app-shell",
  "match_settings_toggles.js": "app-shell",
  "match_auto_spectator.js": "app-shell",
  "spectator_controls_panel.js": "app-shell",
  "auto_spectator.js": "app-shell",
  "match_health.js": "app-shell",
  "frame_profiler.js": "app-shell",
  "stress_test.js": "app-shell",
  "stress_test_profile.js": "app-shell",
  "frame_recovery.js": "app-shell",
  "match_fixed_capture.js": "app-shell",
  "visual_clock.js": "app-shell",
  "frame_entity_views.js": "app-shell",
  "ai_diagnostics_panel.js": "app-shell",
  "observer_analysis_overlay.js": "app-shell",
  "observer_analysis_preferences.js": "app-shell",
  "observer_analysis_resources.js": "app-shell",
  "observer_analysis_rows.js": "app-shell",
  "observer_analysis_signatures.js": "app-shell",
  "floating_panel_positioner.js": "app-shell",
  "live_pause_overlay.js": "app-shell",
  "replay_controls.js": "app-shell",
  "replay_seek_notice.js": "app-shell",
  "room_time_panel.js": "app-shell",
  "replay_viewer.js": "app-shell",
  "lab_control_policy.js": "app-shell",
  "room_capabilities.js": "app-shell",
  "visual_profiles.js": "app-shell",
  "camera_view_selection.js": "app-shell",
  "launch_url.js": "app-shell",
  "interact_bridge.js": "app-shell",
  "interact_game_bridge.js": "app-shell",
  "clean_presentation.js": "app-shell",
  "map_editor_app.js": "app-shell",
  "map_editor_viewport.js": "app-shell",

  "state.js": "model",
  "state_control_groups.js": "model",
  "state_queries.js": "model",
  "state_ground_decals.js": "model",
  "state_visual_effects.js": "model",
  "client_intent.js": "model",
  "command_budget.js": "model",
  "command_composer.js": "model",
  "progress_extrapolator.js": "model",
  "prediction_controller.js": "model",
  "prediction_compatibility.js": "model",
  "sim_wasm_adapter.js": "model",
  "prediction_settings.js": "platform",
  "unit_range_settings.js": "platform",
  "auto_spectator_settings.js": "platform",

  "net.js": "transport",
  "snapshot_stream_net.js": "transport",
  "protocol.js": "transport",
  "protocol_constants.js": "transport",
  "protocol_frame.js": "transport",
  "protocol_snapshot.js": "transport",
  "protocol_snapshot_events.js": "transport",
  "protocol_snapshot_trenches.js": "transport",
  "lab_client.js": "transport",
  "map_editor_handoff.js": "transport",
  "report_window_aggregate.js": "platform",

  "config.js": "rules-mirror",

  "hud.js": "ui",
  "hud_ability_affordance.js": "ui",
  "hud_command_dom.js": "ui",
  "hud_command_card.js": "ui",
  "hud_train_card_helpers.js": "ui",
  "hud_control_groups.js": "ui",
  "hud_resources.js": "ui",
  "hud_selection_panel.js": "ui",
  "hud_unit_commands.js": "ui",
  "hotkey_editor.js": "ui",
  "hotkey_profiles.js": "ui",
  "lobby.js": "ui",
  "lobby_browser_view.js": "ui",
  "lobby_view.js": "ui",
  "match_history.js": "ui",
  "resource_icons.js": "ui",
  "status_badge.js": "ui",
  "minimap.js": "ui",
  "branch_staging.js": "ui",
  "lab_catalog.js": "ui",
  "lab_spawn_catalog.js": "ui",
  "lab_scenario_authoring.js": "ui",
  "lab_scenario_submission_capability.js": "ui",
  "lab_scenario_submission_flow.js": "ui",
  "lab_panel.js": "ui",
  "lab_tool_detail.js": "ui",
  "lab_panel_window.js": "ui",
  "map_editor_panel.js": "ui",
  "map_editor_session.js": "ui",
  "panel_touch_activation.js": "ui",
  "settings_container.js": "ui",
  "settings_panels.js": "ui",
  "scoreboard.js": "ui",

  "replay_camera_input.js": "input",

  "bootstrap.js": "platform",
  "audio.js": "platform",
  "audio_spatial.js": "platform",
  "sound_manifest.js": "platform",
  "combat_audio.js": "platform",
  "alerts.js": "platform",
  "fog.js": "platform",
  "camera.js": "platform",
  "camera_projection.js": "platform",
  "fixed_perspective_camera.js": "platform",
  "map_editor_launch.js": "platform",
  "stress_test_launch.js": "platform",
}));

const AREA_PREFIXES = [
  ["config/", "rules-mirror"],
  ["config_", "rules-mirror"],
  ["presentation/", "presentation"],
  ["renderer/", "renderer"],
  ["input/", "input"],
];

const ALLOWED_CROSS_AREA_IMPORTS = new Map(Object.entries({
  "net.js -> report_window_aggregate.js": "Net Report Phase 1 shares bounded report-window aggregation with client perf and command diagnostics.",
  "prediction_controller.js -> report_window_aggregate.js": "Net Report Phase 2 reuses the bounded report-window helper for command milestone diagnostics.",
  "minimap.js -> input/artillery_targeting.js": "Minimap command targeting shares the pure artillery target-lock predictor with viewport input so local feedback matches server locking.",
  "renderer/backend_bundle.js -> camera.js": "The selected Pixi bundle owns construction of its orthographic semantic camera.",
  "renderer/babylon/backend_bundle.js -> fixed_perspective_camera.js": "The selected Babylon bundle owns construction of its engine-independent semantic camera.",
}));

const ALLOWED_PROTOTYPE_GRAFTS = new Set([
  "input/index.js:Input",
  "renderer/index.js:Renderer",
]);

const FORBIDDEN_MATCH_LAB_IMPORTS = new Set([
  "lab_client.js",
  "lab_panel.js",
]);

const FORBIDDEN_PRESENTATION_IMPORT_AREAS = new Set(["transport", "ui", "renderer"]);
const PIXI_COMPATIBILITY_ADAPTER = "renderer/pixi_compatibility_adapter.js";
const forbiddenPresentationRuntimeRe = /\b(?:PIXI|BABYLON|WebSocket|GameState|ClientIntent)\b/;

// Phase 6 client-boundary ratchet: these are the cleanup-phase byte counts for
// the largest modules. Future growth should either extract a focused helper or
// update this table with the phase-specific reason.
const LARGE_FILE_BASELINES = new Map(Object.entries({
  // Tank Trap Phase 5 extends placement feedback to draw multi-site line previews.
  "renderer/feedback.js": 46315,
  // Lab MVP Phase 5 injects explicit lab control policy into command-card context.
  "hud.js": 44208,
  // Hotspot Cleanup Phase 6 extracted GameState query and visual-effect helpers.
  // Death decals add a narrow browser-local decal queue owned by GameState.
  "state.js": 30123,
  // Lab MVP2 Phase 5 routes lab setup-tool cancellation through the input controller.
  "input/index.js": 40927,
  // Visual Experimentation Phase 1 injects local lab visual profile state for renderer-only samples.
  // Interact Phase 6 adds only the public fixed-capture lifecycle seam; its state machine lives
  // in match_fixed_capture.js so Match retains renderer/rAF ownership without absorbing the logic.
  // Render3D Phase 4 replaces direct Pixi construction with one injected selected-backend bundle.
  "match.js": 48131,
  // Artillery minimap markers add a compact visual-only firing event.
  "protocol.js": 45366,
  // Protocol cleanup split compact snapshot decoding behind protocol.js.
  // Entrenchment Phase 3 adds the occupiedTrenchId slot; Panzerfaust reserves compact events.
  "protocol_snapshot.js": 24192,
  // Lab MVP Phase 5 lets command descriptors ask the injected policy which owner is controllable.
  "hud_command_card.js": 29498,
  // Interact Phase 6 centralizes renderer visual-clock fallback so extracted draw modules
  // share the injected clock without each growing its own compatibility expression.
  "renderer/shared.js": 28217,
  // Render Lag Phase 3 lets the observer Army Value tab consume frame-local entity views.
  "observer_analysis_overlay.js": 28009,
  "audio.js": 27339,
}));

const FORBIDDEN_GAMESTATE_INTENT_SHIMS = [
  "commandTarget",
  "placement",
  "commandCardMode",
  "attackTargetPreview",
  "resourceMiningPreview",
  "antiTankGunSetupPreview",
  "abilityTargetPreview",
  "liveCommandFeedback",
  "activeLabTool",
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
  "beginLabTool",
  "cancelLabTool",
  "updateResourceMiningPreview",
  "updateAttackTargetPreview",
  "updateAntiTankGunSetupPreview",
  "updateAbilityTargetPreview",
];

// Render3D Phase 1.75 raw-camera ratchet. Orthographic representation stays private to the
// camera and named Pixi adapters; shared consumers use only semantic camera operations.
const PRIVATE_RAW_CAMERA_ADAPTERS = new Set([
  "camera.js",
  "camera_projection.js",
  "map_editor_viewport.js",
  "renderer/index.js",
]);

const rawCameraRepresentationRe = /\b(?:(?:this|match)\.)?(?:camera|cam)(?:\?\.|\.)(?:x|y|zoom|viewW|viewH|worldW|worldH|centerOn|setZoom|setView|setBounds)\b/g;

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
  checkRawCameraRepresentation(file, source);
  checkRoomCapabilityParser(file, source);
  checkRigOnlyUnitVisuals(file, source);
  checkPresentationBoundary(file, source);
  checkMatchRendererSeam(file, source);
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

function checkRawCameraRepresentation(file, source) {
  const references = [...source.matchAll(rawCameraRepresentationRe)].map((match) => match[0]);
  if (references.length === 0) return;
  if (PRIVATE_RAW_CAMERA_ADAPTERS.has(file)) return;
  failures.push(
    `${file}: raw camera representation ${[...new Set(references)].join(", ")}; use semantic camera operations`,
  );
}

function checkRoomCapabilityParser(file, source) {
  if (file !== "room_capabilities.js") return;
  const forbidden = [
    ["devWatch", "dev-watch route identity"],
    ["replayViewer", "replay-viewer shell identity"],
    ["startPayload?.replay", "replay start metadata"],
    ["startPayload.replay", "replay start metadata"],
    ["debugMode", "legacy debug-mode flag"],
  ];
  for (const [needle, label] of forbidden) {
    if (source.includes(needle)) {
      failures.push(`${file}: room capabilities must read startPayload.capabilities/diagnostics, not ${label}`);
    }
  }
}

function checkRigOnlyUnitVisuals(file, source) {
  if (file === "renderer/units.js") {
    const forbidden = [
      ["new PIXI.Graphics", "direct Pixi graphics allocation"],
      ["PIXI.Graphics", "direct Pixi graphics access"],
      ["drawPolygon", "procedural polygon drawing"],
      ["drawCircle", "procedural circle drawing"],
      ["drawRect", "procedural rectangle drawing"],
      ["beginFill", "procedural fill drawing"],
      ["lineStyle", "procedural stroke drawing"],
      ["_slot(", "direct renderer graphics pool access"],
      ["renderRigLegacyComparison", "legacy comparison renderer"],
      ["createLegacyUnitPartCapture", "legacy part capture"],
      ["LEGACY_PARTS", "legacy part metadata"],
      ["partCapture", "migration part-capture hook"],
      ["skipLiveRig", "live rig bypass flag"],
      ["skipRigComparison", "comparison bypass flag"],
    ];
    for (const [needle, label] of forbidden) {
      if (source.includes(needle)) {
        failures.push(`${file}: unit visuals must route through SVG rigs, found ${label}`);
      }
    }
  }

  if (file === "renderer/rigs/runtime.js") {
    const forbidden = [
      ["renderRigLegacyComparison", "legacy comparison renderer"],
      ["_rigComparisonPool", "legacy comparison pool"],
      ["skipRigComparison", "comparison bypass flag"],
    ];
    for (const [needle, label] of forbidden) {
      if (source.includes(needle)) {
        failures.push(`${file}: rig runtime must not keep migration comparison plumbing, found ${label}`);
      }
    }
  }
}

function checkPresentationBoundary(file, source) {
  if (!file.startsWith("presentation/")) return;
  const match = source.match(forbiddenPresentationRuntimeRe);
  if (match) {
    failures.push(`${file}: presentation boundary must not reference mutable/runtime internal ${match[0]}`);
  }
}

function checkMatchRendererSeam(file, source) {
  if (file.startsWith("renderer/babylon/")) {
    for (const forbidden of ["requestAnimationFrame", "runRenderLoop", "PixiPresentationAdapter", "GameState", "ClientIntent"]) {
      if (source.includes(forbidden)) {
        failures.push(`${file}: Babylon backend must not reference ${forbidden}; Match owns timing and detached presentation`);
      }
    }
  }
  if (file === "frame_recovery.js") {
    const calls = source.match(/match\.renderer\.render\([^\n]*/g) || [];
    if (calls.length !== 1 || calls[0] !== "match.renderer.render(presentationFrame));") {
      failures.push(`${file}: Match frame orchestration must have exactly one backend render(presentationFrame) seam`);
    }
    if (source.includes("consumePendingGroundDecals")) {
      failures.push(`${file}: frame orchestration must reconcile decal queues before assembly, not use the legacy renderer consumer`);
    }
  }
  if (file === "match.js") {
    for (const forbidden of ["this.renderer.drawSelectionBox", "this.renderer.buildStaticMap"]) {
      if (source.includes(forbidden)) failures.push(`${file}: backend operation ${forbidden} bypasses render(frame)`);
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
  if (fromFile === "match.js" && FORBIDDEN_MATCH_LAB_IMPORTS.has(toFile)) {
    failures.push(`${fromFile} -> ${toFile}: Match must receive lab UI/services through app-shell dependency injection`);
    return;
  }
  if (toFile === PIXI_COMPATIBILITY_ADAPTER && fromFile !== "renderer/backend_bundle.js") {
    failures.push(`${fromFile} -> ${toFile}: Pixi compatibility adapter is private to the Pixi backend bundle and unavailable to other backends`);
    return;
  }

  const from = modules.get(fromFile);
  const to = modules.get(toFile);
  if (!from?.area || !to?.area) return;
  if (from.area === "presentation" && FORBIDDEN_PRESENTATION_IMPORT_AREAS.has(to.area)) {
    failures.push(`${fromFile} -> ${toFile}: presentation may not import ${to.area} internals`);
    return;
  }
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
