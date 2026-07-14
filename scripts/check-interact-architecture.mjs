#!/usr/bin/env node

import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  INTERACT_COMMAND_REGISTRY,
  INTERACT_COMMANDS,
} from "./interact/command_registry.ts";
import { SESSION_EXECUTION_LANES } from "./interact/session_coordinator.ts";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const interactRoot = path.join(repoRoot, "scripts", "interact");
const failures = [];
const sources = new Map(readdirSync(interactRoot)
  .filter((name) => name.endsWith(".ts") || name === "cli.mjs")
  .map((name) => [name, readFileSync(path.join(interactRoot, name), "utf8")]));

checkRegistry();
checkCliNamespace();
checkServiceRouting();
checkImports();
checkQueueOwnership();
checkSignalOwnership();
checkAdapterOwnership();
checkBlockingProcesses();
checkDependencyOwnership();
checkSizeRatchets();

if (failures.length) {
  console.error("Interact architecture check failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(`Interact architecture check passed (${INTERACT_COMMANDS.length} registry commands; responsive adapter and size ratchets passed)`);

function checkRegistry() {
  const names = Object.keys(INTERACT_COMMAND_REGISTRY);
  if (new Set(names).size !== names.length || names.length !== INTERACT_COMMANDS.length) {
    failures.push("public commands must be defined exactly once in command_registry.ts");
  }
  const allowedScopes = new Set(["daemon", "session"]);
  const allowedLanes = new Set(SESSION_EXECUTION_LANES);
  const allowedTimeouts = new Set(["ordinary", "lifecycle-media"]);
  for (const [name, definition] of Object.entries(INTERACT_COMMAND_REGISTRY)) {
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

function checkCliNamespace() {
  const cli = sources.get("cli.ts") || "";
  if (!cli.includes('const USAGE = "node scripts/interact/cli.mjs lab <command> [JSON-object]";')) {
    failures.push("the Interact CLI usage must require the Lab namespace");
  }
  if (!cli.includes('argv[0] !== "lab"') || !cli.includes('"unknownNamespace"')) {
    failures.push("the Interact CLI must reject bare or unknown namespaces before command dispatch");
  }
}

function expectMetadata(label, field, value, expected) {
  const actual = INTERACT_COMMANDS.filter((name) => INTERACT_COMMAND_REGISTRY[name][field] === value).sort();
  const wanted = [...expected].sort();
  if (actual.join("\0") !== wanted.join("\0")) {
    failures.push(`${label} must be ${wanted.join(", ")}; got ${actual.join(", ")}`);
  }
}

function checkServiceRouting() {
  const service = sources.get("command_service.ts") || "";
  if (!service.includes("executeSession(definition.handlerKey")) {
    failures.push("command_service.ts must route registry handler keys into session handlers");
  }
  for (const definition of Object.values(INTERACT_COMMAND_REGISTRY)) {
    const key = definition.handlerKey.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&");
    if (!new RegExp(`(?:handlerKey|command)\\s*===\\s*["']${key}["']`).test(service)) {
      failures.push(`${definition.name} handler key ${definition.handlerKey} has no service route`);
    }
  }
}

function checkImports() {
  const imports = new Map([...sources].map(([name, source]) => [name, relativeImports(source)]));
  forbidImports(imports, "driver.ts", ["command_inputs.ts", "command_registry.ts", "command_service.ts", "session_coordinator.ts", "cli.mjs", "daemon.ts"]);
  forbidImports(imports, "runtime.ts", ["command_inputs.ts", "command_registry.ts", "command_service.ts", "session_coordinator.ts", "cli.mjs", "daemon.ts"]);
  for (const name of ["abort_signal.ts", "process_runner.ts", "private_server.ts", "recording.ts", "fixed_capture.ts", "tailnet_preview.ts", "workspace_inspection.ts"]) {
    forbidImports(imports, name, ["command_inputs.ts", "command_registry.ts", "command_service.ts", "session_coordinator.ts", "driver.ts", "cli.mjs", "daemon.ts"]);
  }
  for (const name of ["command_inputs.ts", "command_registry.ts", "command_help.ts", "session_coordinator.ts"]) {
    forbidImports(imports, name, ["command_service.ts", "driver.ts", "cli.mjs", "daemon.ts"]);
  }
  forbidImports(imports, "cli.ts", ["driver.ts", "command_service.ts", "session_coordinator.ts"]);
  forbidImports(imports, "daemon.ts", ["driver.ts", "session_coordinator.ts"]);
  if (!imports.get("command_service.ts")?.includes("session_coordinator.ts") ||
      !imports.get("command_service.ts")?.includes("command_registry.ts") ||
      !imports.get("command_service.ts")?.includes("driver.ts")) {
    failures.push("command_service.ts must connect the registry/coordinator application layer to the driver adapter");
  }
  if ((sources.get("command_service.ts") || "").includes("puppeteer-core")) {
    failures.push("command_service.ts must use structural application types instead of Puppeteer adapter types");
  }
}

function checkAdapterOwnership() {
  const driverImports = relativeImports(sources.get("driver.ts") || "");
  if (!driverImports.includes("private_server.ts")) failures.push("driver.ts must delegate private-server lifecycle to private_server.ts");
  if (!driverImports.includes("workspace_inspection.ts")) failures.push("driver.ts must keep bounded pre-request workspace inspection outside the request-path process check");
  const privateServerImports = relativeImports(sources.get("private_server.ts") || "");
  if (!privateServerImports.includes("process_runner.ts")) failures.push("private_server.ts must use process_runner.ts for finite Cargo builds");

  const allowedChildOwners = new Set([
    "cli.ts", "process_runner.ts", "private_server.ts", "recording.ts",
    "runtime.ts", "workspace_inspection.ts",
  ]);
  for (const [name, source] of sources) {
    if (/from\s+["']node:child_process["']/.test(source) && !allowedChildOwners.has(name)) {
      failures.push(`${name} imports child_process without owning an approved child lifecycle`);
    }
  }
  for (const owner of ["process_runner.ts", "private_server.ts", "recording.ts"]) {
    if (!/from\s+["']node:child_process["']/.test(sources.get(owner) || "")) {
      failures.push(`${owner} must remain an explicit request-path/tool child owner`);
    }
  }
  const cli = sources.get("cli.ts") || "";
  if (!/spawn\(process\.execPath,\s*\[path\.join\(scriptDir,\s*["']daemon\.ts["']\)/.test(cli)) {
    failures.push("cli.ts must remain the explicit daemon bootstrap child owner and spawn daemon.ts with Node");
  }
}

function checkBlockingProcesses() {
  const requestPath = [
    "command_service.ts", "driver.ts", "private_server.ts", "process_runner.ts",
    "recording.ts", "fixed_capture.ts", "tailnet_preview.ts", "daemon.ts",
  ];
  for (const name of requestPath) {
    if (/\b(?:spawnSync|execSync|execFileSync)\b/.test(sources.get(name) || "")) {
      failures.push(`${name} contains a blocking child process in the daemon request path`);
    }
  }
  const workspaceInspection = sources.get("workspace_inspection.ts") || "";
  if (!/spawnSync\("git"/.test(workspaceInspection) || !/timeout:\s*2_000/.test(workspaceInspection)) {
    failures.push("workspace_inspection.ts must keep its documented synchronous Git exception explicitly bounded");
  }
  const runtimeSources = [...sources]
    .filter(([name]) => name !== "cli.mjs" && name !== "cli.ts")
    .map(([, source]) => source)
    .join("\n");
  if (/(?:runOrThrow|\.run)\s*\(\s*["']npm["']|spawn\s*\(\s*["']npm["']/.test(runtimeSources)) {
    failures.push("Interact daemon/runtime modules may not install Node dependencies at request time");
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
  if (rootPackage?.engines?.node !== ">=22.18.0" || rootLock?.packages?.[""]?.engines?.node !== ">=22.18.0") {
    failures.push("repository package and lock must require Node 22.18.0 or newer");
  }
  if (rootPackage?.devDependencies?.typescript !== "^5.8" ||
      rootLock?.packages?.[""]?.devDependencies?.typescript !== "^5.8" ||
      rootPackage?.devDependencies?.["@types/node"] !== "^22" ||
      rootLock?.packages?.[""]?.devDependencies?.["@types/node"] !== "^22") {
    failures.push("repository package and lock must own TypeScript 5.8+ and Node 22 typings");
  }
  if (rootPackage?.scripts?.["check:interact-types"] !== "tsc --project scripts/interact/tsconfig.json") {
    failures.push("repository package must expose the Interact no-emit typecheck");
  }
  const tsconfig = readJson(path.join(interactRoot, "tsconfig.json"));
  const compiler = tsconfig?.compilerOptions || {};
  for (const [field, expected] of Object.entries({
    noEmit: true, strict: true, target: "ESNext", module: "NodeNext", moduleResolution: "NodeNext",
    allowImportingTsExtensions: true, erasableSyntaxOnly: true, verbatimModuleSyntax: true,
  })) {
    if (compiler[field] !== expected) failures.push(`Interact TypeScript config must set ${field} to ${JSON.stringify(expected)}`);
  }
  const implementationMjs = readdirSync(interactRoot).filter((name) => name.endsWith(".mjs"));
  if (implementationMjs.join("\0") !== "cli.mjs") failures.push("cli.mjs must be the only JavaScript implementation file in scripts/interact");
  const bootstrap = sources.get("cli.mjs") || "";
  if (!bootstrap.includes('await import("./cli.ts")') || !bootstrap.includes("22.18")) {
    failures.push("cli.mjs must only preflight Node 22.18+ and import cli.ts");
  }
  const testRunner = readFileSync(path.join(repoRoot, "tests", "run-all.sh"), "utf8");
  if (!testRunner.includes("Node 22.18 or newer") ||
      !/NODE_MAJOR[^\n]+-eq 22[^\n]+NODE_MINOR[^\n]+-lt 18/.test(testRunner)) {
    failures.push("tests/run-all.sh must reject Node releases older than 22.18 before direct TypeScript suites");
  }
  for (const [name, source] of sources) {
    if (/\@ts-(?:ignore|nocheck)/.test(source)) failures.push(`${name} contains a blanket TypeScript escape`);
  }
  if (testPackage?.devDependencies?.["puppeteer-core"] != null) {
    failures.push("tests/package.json may not retain a test-only puppeteer-core dependency");
  }
  const driver = sources.get("driver.ts") || "";
  if (!/import\(["']puppeteer-core["']\)/.test(driver) || /testsDir|ensureTestNodeModules|createRequire/.test(driver)) {
    failures.push("driver.ts must import repository-owned puppeteer-core without runtime hydration");
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
    if (name === "session_coordinator.ts") continue;
    if (/semanticTail|operationTail|\benqueue\s*\(/.test(source)) {
      failures.push(`${name} contains a generic semantic queue; session_coordinator.ts is the sole owner`);
    }
  }
  const coordinator = sources.get("session_coordinator.ts") || "";
  if (!/semanticTails/.test(coordinator) || !/drain\(sessionId(?::[^)]*)?\)/.test(coordinator)) {
    failures.push("session_coordinator.ts must own the semantic FIFO and close drain");
  }
}

function checkSignalOwnership() {
  for (const [name, source] of sources) {
    if (name === "daemon.ts") continue;
    if (/process\.(?:once|on|addListener)\s*\(/.test(source)) {
      failures.push(`${name} installs a process listener; daemon.ts is the sole process-signal owner`);
    }
  }
  const daemon = sources.get("daemon.ts") || "";
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    if (!daemon.includes(`"${signal}"`)) failures.push(`daemon.ts does not own ${signal}`);
  }
}

function checkSizeRatchets() {
  for (const [name, maximum] of [
    ["command_service.ts", 1_050],
    ["driver.ts", 1_400],
    ["abort_signal.ts", 60],
    ["process_runner.ts", 210],
    ["private_server.ts", 310],
    ["recording.ts", 600],
    ["fixed_capture.ts", 140],
    ["tailnet_preview.ts", 400],
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
