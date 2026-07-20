#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, "..");
const defaultConfig = path.join(scriptDir, "agent-pr-passes.json");

export function parseArgs(argv) {
  const options = {
    baseRef: "origin/main",
    configFile: defaultConfig,
    dryRun: false,
    headBranch: "",
    help: false,
    markdownReportFile: "",
    repoRoot: defaultRepoRoot,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = (name) => {
      index += 1;
      if (index >= argv.length || argv[index].startsWith("--")) throw new Error(`${name} requires a value`);
      return argv[index];
    };
    if (arg === "-h" || arg === "--help") options.help = true;
    else if (arg === "--base") options.baseRef = value(arg);
    else if (arg === "--config") options.configFile = path.resolve(value(arg));
    else if (arg === "--head-branch") options.headBranch = value(arg);
    else if (arg === "--markdown-report-file") options.markdownReportFile = path.resolve(value(arg));
    else if (arg === "--repo") options.repoRoot = path.resolve(value(arg));
    else if (arg === "--dry-run") options.dryRun = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  return options;
}

export function loadPasses(configFile) {
  const parsed = JSON.parse(fs.readFileSync(configFile, "utf8"));
  if (parsed?.version !== 1 || !Array.isArray(parsed.passes)) throw new Error("agent PR pass config must have version 1 and passes[]");
  const ids = new Set();
  return parsed.passes.map((entry) => {
    if (!entry || typeof entry.id !== "string" || !entry.id.trim()) throw new Error("agent PR pass requires a non-empty id");
    const id = entry.id.trim();
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(id)) throw new Error(`agent PR pass has invalid id: ${id}`);
    if (ids.has(id)) throw new Error(`duplicate agent PR pass id: ${id}`);
    ids.add(id);
    if (!Array.isArray(entry.command) || entry.command.some((part) => typeof part !== "string" || !part) || entry.command.length === 0) {
      throw new Error(`agent PR pass ${id} requires a non-empty string command[]`);
    }
    if (entry.modelEnv !== undefined && (typeof entry.modelEnv !== "string" || !entry.modelEnv.trim())) {
      throw new Error(`agent PR pass ${id} has invalid modelEnv`);
    }
    return { id, command: entry.command, modelEnv: entry.modelEnv?.trim() || "" };
  });
}

function runCapture(command, args, options) {
  const result = spawnSync(command, args, { cwd: options.cwd, encoding: "utf8", env: options.env });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr?.trim() || result.stdout?.trim() || `${command} exited ${result.status}`);
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
}

function git(repoRoot, args) {
  const result = spawnSync("git", args, { cwd: repoRoot, encoding: "utf8" });
  if (result.status !== 0) throw new Error(result.stderr?.trim() || `git ${args.join(" ")} failed`);
  return result.stdout.trim();
}

export function markdownSummary(records) {
  const sections = ["## Agent PR passes", ""];
  for (const record of records) {
    sections.push(`### ${record.id}`, "", record.report.trim() || "Pass completed without a report.", "");
  }
  return sections.join("\n");
}

export function run(options) {
  if (options.help) {
    process.stdout.write("Usage: node scripts/agent-pr-passes.mjs [--base REF] [--head-branch BRANCH] [--config FILE] [--markdown-report-file FILE] [--repo DIR] [--dry-run]\n");
    return [];
  }
  const branch = git(options.repoRoot, ["branch", "--show-current"]);
  if (!branch || (options.headBranch && branch !== options.headBranch)) throw new Error(`agent PR pass branch mismatch: current=${branch || "<detached>"} requested=${options.headBranch || branch}`);
  if (!options.dryRun) {
    const status = git(options.repoRoot, ["status", "--porcelain=v1"]);
    if (status) throw new Error(`agent PR passes require a clean worktree before starting:\n${status}`);
  }
  const passes = loadPasses(options.configFile);
  const records = [];
  for (const pass of passes) {
    const reportFile = path.join(os.tmpdir(), `rts-agent-pr-pass-${pass.id}-${process.pid}.md`);
    const [command, ...configuredArgs] = pass.command;
    const args = [...configuredArgs, "--base", options.baseRef, "--head-branch", branch, "--markdown-report-file", reportFile, "--repo", options.repoRoot];
    const model = pass.modelEnv ? process.env[pass.modelEnv]?.trim() : "";
    if (model) args.push("--codex-model", model);
    if (options.dryRun) args.push("--dry-run");
    process.stdout.write(`agent-pr-pass: ${pass.id}${model ? ` model=${model}` : " model=default"}\n`);
    try {
      runCapture(command, args, { cwd: options.repoRoot, env: process.env });
      records.push({ id: pass.id, report: fs.existsSync(reportFile) ? fs.readFileSync(reportFile, "utf8") : "Pass completed." });
    } finally {
      fs.rmSync(reportFile, { force: true });
    }
    if (!options.dryRun) {
      const afterBranch = git(options.repoRoot, ["branch", "--show-current"]);
      const status = git(options.repoRoot, ["status", "--porcelain=v1"]);
      if (afterBranch !== branch) throw new Error(`agent PR pass ${pass.id} changed branches to ${afterBranch}`);
      if (status) throw new Error(`agent PR pass ${pass.id} left a dirty worktree:\n${status}`);
    }
  }
  if (options.markdownReportFile) fs.writeFileSync(options.markdownReportFile, markdownSummary(records));
  return records;
}

const invokedAsScript = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedAsScript) {
  try {
    run(parseArgs(process.argv.slice(2)));
  } catch (error) {
    process.stderr.write(`agent-pr-passes: ${error.message}\n`);
    process.exitCode = 1;
  }
}
