#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";

const repoRoot = path.resolve(new URL("..", import.meta.url).pathname);

const checks = [
  {
    label: "wiki route and table tests",
    command: "cargo",
    args: ["test", "--manifest-path", "server/Cargo.toml", "-p", "rts-server", "wiki"],
  },
  {
    label: "faction catalog parity",
    command: "node",
    args: ["scripts/check-faction-catalog-parity.mjs"],
  },
];

for (const check of checks) {
  console.log(`\n== ${check.label} ==`);
  const result = spawnSync(check.command, check.args, {
    cwd: repoRoot,
    stdio: "inherit",
  });
  if (result.error) {
    console.error(`${check.label} failed to start: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log("\nwiki regression check passed");
