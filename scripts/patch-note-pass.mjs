#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, "..");
const defaultSchema = path.join(scriptDir, "patch-note-pass.schema.json");
const MAX_DIFF_CHARS = 60000;

export function parseArgs(argv) {
  const options = {
    baseRef: "origin/main", codexCommand: "codex", codexModel: "", dryRun: false,
    headBranch: "", help: false, markdownReportFile: "", repoRoot: defaultRepoRoot, schemaFile: defaultSchema,
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
    else if (arg === "--codex-command") options.codexCommand = value(arg);
    else if (arg === "--codex-model") options.codexModel = value(arg);
    else if (arg === "--head-branch") options.headBranch = value(arg);
    else if (arg === "--markdown-report-file") options.markdownReportFile = path.resolve(value(arg));
    else if (arg === "--repo") options.repoRoot = path.resolve(value(arg));
    else if (arg === "--schema") options.schemaFile = path.resolve(value(arg));
    else if (arg === "--dry-run") options.dryRun = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  return options;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: options.cwd, encoding: "utf8", env: { ...process.env, ...(options.env || {}) } });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr?.trim() || result.stdout?.trim() || `${command} exited ${result.status}`);
  return result.stdout.trim();
}

function git(repoRoot, args) { return run("git", args, { cwd: repoRoot }); }

export function branchSlug(branch) {
  return branch.replace(/^zvorygin\//, "").replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "") || "change";
}

const GAMEPLAY_PATH_PREFIXES = [
  "server/crates/ai/src/",
  "server/crates/rules/src/",
  "server/crates/sim/src/",
  "server/crates/protocol/src/",
  "server/src/",
  "client/src/",
];

export function isGameplayCandidate(pathname) {
  if (pathname.startsWith("tests/") || pathname.startsWith("plans/") || pathname.startsWith("patch-notes/") || pathname.endsWith(".md")) return false;
  return GAMEPLAY_PATH_PREFIXES.some((prefix) => pathname.startsWith(prefix)) ||
    pathname === "client/index.html" ||
    (pathname.startsWith("client/") && pathname.endsWith(".css"));
}

export function normalizeDecision(raw) {
  if (!raw || !["no_patch_note", "write_patch_note"].includes(raw.decision)) throw new Error("patch-note pass returned an invalid decision");
  const singleLine = (value) => String(value || "").replace(/\s+/g, " ").trim();
  const strings = (value, max) => Array.isArray(value) ? value.map(singleLine).filter(Boolean).slice(0, max) : [];
  const decision = {
    decision: raw.decision,
    title: singleLine(raw.title),
    changes: strings(raw.changes, 8),
    playtestWatch: strings(raw.playtest_watch, 4),
    reason: singleLine(raw.reason),
  };
  if (decision.decision === "write_patch_note" && (!decision.title || decision.changes.length === 0)) {
    throw new Error("write_patch_note requires a title and at least one factual change");
  }
  return decision;
}

export function renderFragment({ branch, date, decision }) {
  const lines = [
    "<!-- rts-patch-note:v1 -->",
    `<!-- branch: ${branch} -->`,
    `# ${decision.title}`,
    "",
    `_${date}_`,
    "",
    "## Changes",
    "",
    ...decision.changes.map((item) => `- ${item}`),
  ];
  if (decision.playtestWatch.length) {
    lines.push("", "## Playtest watch", "", ...decision.playtestWatch.map((item) => `- ${item}`));
  }
  lines.push("");
  return lines.join("\n");
}

function renderPrompt({ baseRef, branch, changedPaths, diff, existingFragment, fragmentPath }) {
  return `You are the player-impact and patch-note pass for an RTS pull request.

Classify the complete branch diff from ${baseRef} to HEAD. A patch note is required for player-facing
gameplay changes: unit/building stats, costs, economy, combat behavior, available commands or units,
meaningful gameplay UI affordances, or other changes players should adapt to. Tests, refactors,
developer tools, and fixes with no player-observable gameplay effect do not need a patch note.

If a note is required, state exact factual changes, including old and new values when the diff proves
them. Add concise playtest-watch bullets only where useful. Do not speculate. The outer helper will
render your JSON into ${fragmentPath}; do not edit files or run commands.

Branch: ${branch}
Changed paths:
${changedPaths.join("\n")}

Existing fragment, if any:
${existingFragment || "<none>"}

Bounded diff:
${diff}
`;
}

function existingFragmentPath(repoRoot, baseRef, branch, slug) {
  const changedFragments = git(repoRoot, [
    "diff", "--name-only", "--diff-filter=ACMR", "--no-renames", `${baseRef}...HEAD`, "--", "patch-notes",
  ]).split("\n").filter(Boolean);
  const suffix = `/${slug}.md`;
  const matches = changedFragments
    .filter((candidate) => candidate.startsWith("patch-notes/") && candidate.endsWith(suffix))
    .map((candidate) => path.join(repoRoot, candidate))
    .filter((candidate) => fs.existsSync(candidate));
  if (matches.length > 1) throw new Error(`multiple patch-note fragments exist for ${slug}`);
  const existing = matches[0] || "";
  if (existing) {
    const contents = fs.readFileSync(existing, "utf8");
    if (!contents.startsWith("<!-- rts-patch-note:v1 -->\n") || !contents.includes(`<!-- branch: ${branch} -->`)) {
      throw new Error(`refusing to overwrite unmanaged patch-note fragment ${path.relative(repoRoot, existing)}`);
    }
  }
  return existing;
}

function markdownReport(decision, relativePath = "", removed = false) {
  const result = decision.decision === "write_patch_note"
    ? `Updated \`${relativePath}\`.`
    : removed
      ? `Removed stale fragment \`${relativePath}\`.`
      : "No player-facing gameplay change detected.";
  return [
    `Decision: ${decision.decision}`,
    "",
    result,
    "",
    `Reason: ${decision.reason || "Not recorded."}`,
    "",
  ].join("\n");
}

export function execute(options) {
  if (options.help) {
    process.stdout.write("Usage: node scripts/patch-note-pass.mjs [--base REF] [--head-branch BRANCH] [--codex-model MODEL] [--markdown-report-file FILE] [--repo DIR] [--dry-run]\n");
    return null;
  }
  const branch = git(options.repoRoot, ["branch", "--show-current"]);
  if (!branch || (options.headBranch && branch !== options.headBranch)) throw new Error(`patch-note pass branch mismatch: current=${branch || "<detached>"} requested=${options.headBranch || branch}`);
  if (!options.dryRun) {
    const status = git(options.repoRoot, ["status", "--porcelain=v1"]);
    if (status) throw new Error(`patch-note pass requires a clean worktree:\n${status}`);
  }
  const changedPaths = git(options.repoRoot, ["diff", "--name-only", "--no-renames", `${options.baseRef}...HEAD`]).split("\n").filter(Boolean);
  const candidates = changedPaths.filter(isGameplayCandidate);
  const slug = branchSlug(branch);
  const existing = existingFragmentPath(options.repoRoot, options.baseRef, branch, slug);
  const existingRelativePath = existing ? path.relative(options.repoRoot, existing) : "";
  if (candidates.length === 0) {
    const decision = { decision: "no_patch_note", title: "", changes: [], playtestWatch: [], reason: "No runtime paths with potential player impact changed." };
    if (existing && !options.dryRun) {
      run("git", ["rm", "--", existingRelativePath], { cwd: options.repoRoot });
      run("git", ["commit", "-m", "Remove stale gameplay patch note", "-m", decision.reason], { cwd: options.repoRoot });
      process.stdout.write(`patch-note-pass: removed ${existingRelativePath}\n`);
    } else {
      process.stdout.write(`patch-note-pass: skipped; no gameplay candidate paths${existing ? `; would remove ${existingRelativePath}` : ""}\n`);
    }
    if (options.markdownReportFile) fs.writeFileSync(options.markdownReportFile, markdownReport(decision, existingRelativePath, Boolean(existing) && !options.dryRun));
    return decision;
  }
  const date = existing
    ? path.basename(path.dirname(existing))
    : new Date().toISOString().slice(0, 10);
  const fragmentPath = existing || path.join(options.repoRoot, "patch-notes", date, `${slug}.md`);
  const relativePath = path.relative(options.repoRoot, fragmentPath);
  if (!existing && fs.existsSync(fragmentPath)) {
    throw new Error(`refusing to overwrite base-owned patch-note fragment ${relativePath}`);
  }
  const diff = git(options.repoRoot, ["diff", "--no-ext-diff", "--unified=3", `${options.baseRef}...HEAD`, "--", ...candidates]);
  const boundedDiff = diff.length > MAX_DIFF_CHARS ? `${diff.slice(0, MAX_DIFF_CHARS)}\n[diff truncated]` : diff;
  const prompt = renderPrompt({
    baseRef: options.baseRef, branch, changedPaths, diff: boundedDiff,
    existingFragment: existing ? fs.readFileSync(existing, "utf8") : "", fragmentPath: relativePath,
  });
  if (options.dryRun) {
    process.stdout.write(`patch-note-pass: would classify ${candidates.length} gameplay candidate path(s)${options.codexModel ? ` with ${options.codexModel}` : ""}\n`);
    if (options.markdownReportFile) fs.writeFileSync(options.markdownReportFile, "Dry run: patch-note classification would run.\n");
    return null;
  }
  if (!fs.existsSync(options.schemaFile)) throw new Error(`missing patch-note schema: ${options.schemaFile}`);
  const outputFile = path.join(os.tmpdir(), `rts-patch-note-pass-${process.pid}.json`);
  const args = ["exec", "--cd", options.repoRoot, "--sandbox", "read-only", "-c", 'approval_policy="never"', "--ephemeral", "--output-schema", options.schemaFile, "--output-last-message", outputFile];
  if (options.codexModel) args.push("--model", options.codexModel);
  args.push(prompt);
  try {
    process.stdout.write(`patch-note-pass: classifying ${candidates.length} gameplay candidate path(s)\n`);
    run(options.codexCommand, args, { cwd: options.repoRoot });
    const decision = normalizeDecision(JSON.parse(fs.readFileSync(outputFile, "utf8")));
    if (decision.decision === "write_patch_note") {
      fs.mkdirSync(path.dirname(fragmentPath), { recursive: true });
      fs.writeFileSync(fragmentPath, renderFragment({ branch, date, decision }));
      run("git", ["add", "--", relativePath], { cwd: options.repoRoot });
      if (git(options.repoRoot, ["status", "--porcelain=v1", "--", relativePath])) {
        run("git", ["commit", "-m", existing ? "Update gameplay patch note" : "Add gameplay patch note", "-m", decision.reason || "Record the player-facing impact detected by the agent PR patch-note pass."], { cwd: options.repoRoot });
        process.stdout.write(`patch-note-pass: committed ${relativePath}\n`);
      } else {
        process.stdout.write(`patch-note-pass: ${relativePath} already matches the classified impact\n`);
      }
    } else if (existing) {
      run("git", ["rm", "--", relativePath], { cwd: options.repoRoot });
      run("git", ["commit", "-m", "Remove stale gameplay patch note", "-m", decision.reason || "The final branch diff no longer has player-facing gameplay changes."], { cwd: options.repoRoot });
      process.stdout.write(`patch-note-pass: removed ${relativePath}\n`);
    }
    if (options.markdownReportFile) fs.writeFileSync(options.markdownReportFile, markdownReport(decision, relativePath, decision.decision === "no_patch_note" && Boolean(existing)));
    return decision;
  } finally {
    fs.rmSync(outputFile, { force: true });
  }
}

const invokedAsScript = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedAsScript) {
  try { execute(parseArgs(process.argv.slice(2))); }
  catch (error) { process.stderr.write(`patch-note-pass: ${error.message}\n`); process.exitCode = 1; }
}
