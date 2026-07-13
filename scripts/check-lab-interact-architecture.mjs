#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  LAB_INTERACT_COMMAND_REGISTRY,
  LAB_INTERACT_COMMANDS,
} from "./lab-interact/command_registry.mjs";
import { SESSION_EXECUTION_LANES } from "./lab-interact/session_coordinator.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const labRoot = path.join(repoRoot, "scripts", "lab-interact");
const failures = [];
const sources = new Map(readdirSync(labRoot)
  .filter((name) => name.endsWith(".mjs"))
  .map((name) => [name, readFileSync(path.join(labRoot, name), "utf8")]));

checkRegistry();
checkServiceRouting();
checkImports();
checkQueueOwnership();
checkSignalOwnership();
checkAdapterOwnership();
checkBlockingProcesses();
checkDependencyOwnership();
checkSizeRatchets();

if (failures.length) {
  console.error("Lab Interact architecture check failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(`Lab Interact architecture check passed (${LAB_INTERACT_COMMANDS.length} registry commands; responsive adapter and size ratchets passed)`);

function checkRegistry() {
  const names = Object.keys(LAB_INTERACT_COMMAND_REGISTRY);
  if (new Set(names).size !== names.length || names.length !== LAB_INTERACT_COMMANDS.length) {
    failures.push("public commands must be defined exactly once in command_registry.mjs");
  }
  const allowedScopes = new Set(["daemon", "session"]);
  const allowedLanes = new Set(SESSION_EXECUTION_LANES);
  const allowedTimeouts = new Set(["ordinary", "lifecycle-media"]);
  for (const [name, definition] of Object.entries(LAB_INTERACT_COMMAND_REGISTRY)) {
    if (definition.name !== name) failures.push(`${name} registry identity does not match its key`);
    if (!allowedScopes.has(definition.scope)) failures.push(`${name} has invalid scope ${definition.scope}`);
    if (!allowedLanes.has(definition.lane)) failures.push(`${name} has invalid lane ${definition.lane}`);
    if (!allowedTimeouts.has(definition.timeoutClass)) failures.push(`${name} has invalid timeout class ${definition.timeoutClass}`);
    if (typeof definition.validator !== "function") failures.push(`${name} has no validator reference`);
    if (typeof definition.handlerKey !== "string" || !definition.handlerKey) failures.push(`${name} has no handler key`);
    const help = definition.help;
    for (const field of ["summary", "acceptedShape"]) {
      if (typeof help?.[field] !== "string" || !help[field]) failures.push(`${name} help is missing ${field}`);
    }
    for (const field of ["variants", "defaults", "bounds"]) {
      if (!Array.isArray(help?.[field])) failures.push(`${name} help is missing ${field}`);
    }
    if (!help?.example || typeof help.example !== "object" || Array.isArray(help.example)) failures.push(`${name} help is missing an example object`);
  }

  expectMetadata("daemon scope", "scope", "daemon", ["open", "status", "shutdown"]);
  expectMetadata("observation lane", "lane", "observation", ["status", "record-wait"]);
  expectMetadata("cancellation lane", "lane", "cancellation", ["capture-cancel"]);
  expectMetadata("lifecycle lane", "lane", "lifecycle", ["open", "close", "shutdown"]);
  expectMetadata("lifecycle/media timeout", "timeoutClass", "lifecycle-media", [
    "close", "shutdown", "record-stop", "record-wait", "capture-fixed",
  ]);
}

function expectMetadata(label, field, value, expected) {
  const actual = LAB_INTERACT_COMMANDS.filter((name) => LAB_INTERACT_COMMAND_REGISTRY[name][field] === value).sort();
  const wanted = [...expected].sort();
  if (actual.join("\0") !== wanted.join("\0")) {
    failures.push(`${label} must be ${wanted.join(", ")}; got ${actual.join(", ")}`);
  }
}

function checkServiceRouting() {
  const service = sources.get("command_service.mjs") || "";
  if (!service.includes("executeSession(definition.handlerKey")) {
    failures.push("command_service.mjs must route registry handler keys into session handlers");
  }
  for (const definition of Object.values(LAB_INTERACT_COMMAND_REGISTRY)) {
    const key = definition.handlerKey.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&");
    if (!new RegExp(`(?:handlerKey|command)\\s*===\\s*["']${key}["']`).test(service)) {
      failures.push(`${definition.name} handler key ${definition.handlerKey} has no service route`);
    }
  }
}

function checkImports() {
  const imports = new Map([...sources].map(([name, source]) => [name, relativeImports(source)]));
  forbidImports(imports, "driver.mjs", ["command_inputs.mjs", "command_registry.mjs", "command_service.mjs", "session_coordinator.mjs", "cli.mjs", "daemon.mjs"]);
  forbidImports(imports, "runtime.mjs", ["command_inputs.mjs", "command_registry.mjs", "command_service.mjs", "session_coordinator.mjs", "cli.mjs", "daemon.mjs"]);
  for (const name of ["process_runner.mjs", "private_server.mjs", "recording.mjs", "fixed_capture.mjs", "tailnet_preview.mjs", "workspace_inspection.mjs"]) {
    forbidImports(imports, name, ["command_inputs.mjs", "command_registry.mjs", "command_service.mjs", "session_coordinator.mjs", "driver.mjs", "cli.mjs", "daemon.mjs"]);
  }
  for (const name of ["command_inputs.mjs", "command_registry.mjs", "command_help.mjs", "session_coordinator.mjs"]) {
    forbidImports(imports, name, ["command_service.mjs", "driver.mjs", "cli.mjs", "daemon.mjs"]);
  }
  forbidImports(imports, "cli.mjs", ["driver.mjs", "command_service.mjs", "session_coordinator.mjs"]);
  forbidImports(imports, "daemon.mjs", ["driver.mjs", "session_coordinator.mjs"]);
  if (!imports.get("command_service.mjs")?.includes("session_coordinator.mjs") ||
      !imports.get("command_service.mjs")?.includes("command_registry.mjs") ||
      !imports.get("command_service.mjs")?.includes("driver.mjs")) {
    failures.push("command_service.mjs must connect the registry/coordinator application layer to the driver adapter");
  }
}

function checkAdapterOwnership() {
  const driverImports = relativeImports(sources.get("driver.mjs") || "");
  if (!driverImports.includes("private_server.mjs")) failures.push("driver.mjs must delegate private-server lifecycle to private_server.mjs");
  if (!driverImports.includes("workspace_inspection.mjs")) failures.push("driver.mjs must keep bounded pre-request workspace inspection outside the request-path process check");
  const privateServerImports = relativeImports(sources.get("private_server.mjs") || "");
  if (!privateServerImports.includes("process_runner.mjs")) failures.push("private_server.mjs must use process_runner.mjs for finite Cargo builds");

  const allowedChildOwners = new Set([
    "cli.mjs", "process_runner.mjs", "private_server.mjs", "recording.mjs",
    "runtime.mjs", "workspace_inspection.mjs",
  ]);
  for (const [name, source] of sources) {
    if (/from\s+["']node:child_process["']/.test(source) && !allowedChildOwners.has(name)) {
      failures.push(`${name} imports child_process without owning an approved child lifecycle`);
    }
  }
  for (const owner of ["process_runner.mjs", "private_server.mjs", "recording.mjs"]) {
    if (!/from\s+["']node:child_process["']/.test(sources.get(owner) || "")) {
      failures.push(`${owner} must remain an explicit request-path/tool child owner`);
    }
  }
  const cli = sources.get("cli.mjs") || "";
  if (!/spawn\(process\.execPath,\s*\[path\.join\(scriptDir,\s*["']daemon\.mjs["']\)/.test(cli)) {
    failures.push("cli.mjs must remain the explicit daemon bootstrap child owner");
  }
}

function checkBlockingProcesses() {
  const requestPath = [
    "command_service.mjs", "driver.mjs", "private_server.mjs", "process_runner.mjs",
    "recording.mjs", "fixed_capture.mjs", "tailnet_preview.mjs", "daemon.mjs",
  ];
  for (const name of requestPath) {
    if (/\b(?:spawnSync|execSync|execFileSync)\b/.test(sources.get(name) || "")) {
      failures.push(`${name} contains a blocking child process in the daemon request path`);
    }
  }
  const workspaceInspection = sources.get("workspace_inspection.mjs") || "";
  if (!/spawnSync\("git"/.test(workspaceInspection) || !/timeout:\s*2_000/.test(workspaceInspection)) {
    failures.push("workspace_inspection.mjs must keep its documented synchronous Git exception explicitly bounded");
  }
  const labRuntimeSources = [...sources]
    .filter(([name]) => name !== "cli.mjs")
    .map(([, source]) => source)
    .join("\n");
  if (/(?:runOrThrow|\.run)\s*\(\s*["']npm["']|spawn\s*\(\s*["']npm["']/.test(labRuntimeSources)) {
    failures.push("Lab daemon/runtime modules may not install Node dependencies at request time");
  }
}

function checkDependencyOwnership() {
  const rootPackage = readJson(path.join(repoRoot, "package.json"));
  const rootLock = readJson(path.join(repoRoot, "package-lock.json"));
  const testPackage = readJson(path.join(repoRoot, "tests", "package.json"));
  if (rootPackage?.devDependencies?.["puppeteer-core"] !== "^23" ||
      rootLock?.packages?.[""]?.devDependencies?.["puppeteer-core"] !== "^23") {
    failures.push("repository package and lock must own the puppeteer-core runtime dependency");
  }
  if (testPackage?.devDependencies?.["puppeteer-core"] != null) {
    failures.push("tests/package.json may not retain a test-only puppeteer-core dependency");
  }
  const driver = sources.get("driver.mjs") || "";
  if (!/import\(["']puppeteer-core["']\)/.test(driver) || /testsDir|ensureTestNodeModules|createRequire/.test(driver)) {
    failures.push("driver.mjs must import repository-owned puppeteer-core without runtime hydration");
  }
}

function readJson(file) {
  try { return JSON.parse(readFileSync(file, "utf8")); } catch { return null; }
}

function relativeImports(source) {
  return [...source.matchAll(/from\s+["']\.\/([^"']+)["']/g)].map((match) => match[1]);
}

function forbidImports(imports, owner, forbidden) {
  for (const dependency of imports.get(owner) || []) {
    if (forbidden.includes(dependency)) failures.push(`${owner} may not import upward from ${dependency}`);
  }
}

function checkQueueOwnership() {
  for (const [name, source] of sources) {
    if (name === "session_coordinator.mjs") continue;
    if (/semanticTail|operationTail|\benqueue\s*\(/.test(source)) {
      failures.push(`${name} contains a generic semantic queue; session_coordinator.mjs is the sole owner`);
    }
  }
  const coordinator = sources.get("session_coordinator.mjs") || "";
  if (!/semanticTails/.test(coordinator) || !/drain\(sessionId\)/.test(coordinator)) {
    failures.push("session_coordinator.mjs must own the semantic FIFO and close drain");
  }
}

function checkSignalOwnership() {
  for (const [name, source] of sources) {
    if (name === "daemon.mjs") continue;
    if (/process\.(?:once|on|addListener)\s*\(/.test(source)) {
      failures.push(`${name} installs a process listener; daemon.mjs is the sole process-signal owner`);
    }
  }
  const daemon = sources.get("daemon.mjs") || "";
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    if (!daemon.includes(`"${signal}"`)) failures.push(`daemon.mjs does not own ${signal}`);
  }
}

function checkSizeRatchets() {
  for (const [name, maximum] of [
    ["command_service.mjs", 925],
    ["driver.mjs", 1_200],
    ["process_runner.mjs", 190],
    ["private_server.mjs", 275],
    ["recording.mjs", 525],
    ["fixed_capture.mjs", 125],
    ["tailnet_preview.mjs", 375],
  ]) {
    const lines = countLines(sources.get(name) || "");
    if (lines > maximum) failures.push(`${name} is ${lines} lines; responsive-adapter ratchet is ${maximum}`);
  }
}

function countLines(source) {
  if (!source) return 0;
  const lines = source.split(/\r?\n/).length;
  return source.endsWith("\n") ? lines - 1 : lines;
}
