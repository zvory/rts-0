#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, "..");
const DEFAULT_BASE_REF = "origin/main";

export function preflightCommands(baseRef) {
  return [
    { command: "git", args: ["diff", "--check", `${baseRef}...HEAD`] },
    { command: "node", args: ["scripts/check-docs-health.mjs"] },
    { command: "node", args: ["scripts/check-source-file-sizes.mjs"] },
    { command: "node", args: ["tests/select-suites.mjs", "--verify"] },
    { command: "node", args: ["scripts/check-faction-assumptions.mjs"] },
    { command: "node", args: ["scripts/check-deploy-assets.mjs"] },
  ];
}

export function renderCommand({ command, args }) {
  return [command, ...args].join(" ");
}

export function runPreflight({
  repoRoot = defaultRepoRoot,
  baseRef = DEFAULT_BASE_REF,
  dryRun = false,
  log = (message) => process.stdout.write(`${message}\n`),
} = {}) {
  for (const check of preflightCommands(baseRef)) {
    const rendered = renderCommand(check);
    log(`agent-pr preflight: ${dryRun ? "would run" : "running"} ${rendered}`);
    if (dryRun) continue;

    const result = spawnSync(check.command, check.args, {
      cwd: repoRoot,
      stdio: "inherit",
    });
    if (result.error) {
      throw new Error(`agent-pr preflight: failed \`${rendered}\`: ${result.error.message}`);
    }
    if (result.status !== 0) {
      throw new Error(`agent-pr preflight: failed \`${rendered}\` (exit ${result.status ?? "unknown"})`);
    }
  }
}

function usage() {
  return `Usage: node scripts/agent-pr-preflight.mjs [options]

Run the deterministic owned-PR preflight checks.

Options:
  --repo DIR                 Repository root. Default: current RTS checkout.
  --base REF                 Diff base ref. Default: ${DEFAULT_BASE_REF}.
  --dry-run                  Print checks without running them.
  -h, --help                 Show this help.
`;
}

function parseArgs(argv) {
  const options = {
    baseRef: DEFAULT_BASE_REF,
    dryRun: false,
    help: false,
    repoRoot: defaultRepoRoot,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = (name) => {
      const inline = `${name}=`;
      if (arg.startsWith(inline)) return arg.slice(inline.length);
      index += 1;
      if (index >= argv.length || argv[index].startsWith("--")) {
        throw new Error(`${name} requires a value`);
      }
      return argv[index];
    };
    if (arg === "--base" || arg.startsWith("--base=")) {
      options.baseRef = value("--base");
    } else if (arg === "--repo" || arg.startsWith("--repo=")) {
      options.repoRoot = path.resolve(value("--repo"));
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return options;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      process.stdout.write(usage());
    } else {
      runPreflight(options);
    }
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}
