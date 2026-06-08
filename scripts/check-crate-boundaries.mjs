#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(new URL("..", import.meta.url).pathname);
const serverManifest = path.join(repoRoot, "server", "Cargo.toml");

const allowedWorkspaceDeps = new Map([
  ["rts-contract", []],
  ["rts-rules", []],
  ["rts-protocol", ["rts-contract"]],
  ["rts-sim", ["rts-contract", "rts-protocol", "rts-rules"]],
  ["rts-ai", ["rts-contract", "rts-protocol", "rts-rules", "rts-sim"]],
  ["rts-server", ["rts-ai", "rts-contract", "rts-protocol", "rts-rules", "rts-sim"]],
]);

const serverOnlyDeps = new Set(["axum", "tokio", "tower-http", "tracing-subscriber"]);
const serverOnlyImportPattern = /\b(axum|tokio|tower_http|tracing_subscriber|rts_server)::/;

function cargoMetadata() {
  return JSON.parse(
    execFileSync(
      "cargo",
      ["metadata", "--no-deps", "--format-version", "1", "--manifest-path", serverManifest],
      { encoding: "utf8" },
    ),
  );
}

function packageRoot(pkg) {
  return path.dirname(pkg.manifest_path);
}

function rustFiles(dir) {
  const out = execFileSync("fd", ["-e", "rs", ".", dir], { encoding: "utf8" });
  return out.split("\n").filter(Boolean);
}

const metadata = cargoMetadata();
const workspaceNames = new Set(metadata.packages.map((pkg) => pkg.name));
const failures = [];

for (const pkg of metadata.packages) {
  const allowed = new Set(allowedWorkspaceDeps.get(pkg.name) ?? []);
  for (const dep of pkg.dependencies) {
    if (!workspaceNames.has(dep.name)) {
      continue;
    }
    if (!allowed.has(dep.name)) {
      failures.push(`${pkg.name} must not depend on workspace package ${dep.name}`);
    }
  }

  if (pkg.name !== "rts-server") {
    for (const dep of pkg.dependencies) {
      if (serverOnlyDeps.has(dep.name)) {
        failures.push(`${pkg.name} must not depend on server-shell crate ${dep.name}`);
      }
    }
  }
}

for (const pkg of metadata.packages) {
  if (pkg.name === "rts-server") {
    continue;
  }
  const root = packageRoot(pkg);
  if (!existsSync(path.join(root, "src"))) {
    continue;
  }
  for (const file of rustFiles(path.join(root, "src"))) {
    const text = readFileSync(file, "utf8");
    const match = text.match(serverOnlyImportPattern);
    if (match) {
      failures.push(`${pkg.name} imports server-only API ${match[1]} in ${path.relative(repoRoot, file)}`);
    }
  }
}

if (failures.length > 0) {
  console.error("crate boundary check failed:");
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}

console.log("crate boundary check passed");
