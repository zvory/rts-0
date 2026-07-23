#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const DEFAULT_BASE_REF = "origin/main";
const DEFAULT_CONTEXT = "adversarial-quality-pass";
const DEFAULT_REMOTE = "origin";
const DEFAULT_CODEX_COMMAND = "codex";
const DEFAULT_GH_BIN = "gh";
const VERDICTS = new Set(["passed_unchanged", "improved", "improved_with_concerns"]);
export const QUALITY_PASS_ENV = "RTS_ADVERSARIAL_QUALITY_PASS";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, "..");
const defaultSchemaFile = path.join(scriptDir, "adversarial-quality-pass.schema.json");

export function usage() {
  return `Usage: scripts/adversarial-quality-pass.mjs [options]

Runs the final autonomous quality pass for the current branch. The pass reviews
origin/main..HEAD, may edit or rewrite the branch, commits the final state,
optionally pushes the branch, and optionally posts a GitHub commit status on the
final head SHA.

Options:
  --base REF                  Base ref to review against. Default: ${DEFAULT_BASE_REF}
  --head-branch BRANCH        Branch name to push/status. Default: current branch.
  --context NAME              Commit status context. Default: ${DEFAULT_CONTEXT}
  --repo DIR                  Repository root. Default: current RTS checkout.
  --schema FILE               JSON schema passed to Codex.
  --report-file FILE          JSON report output path. Default: temp file.
  --markdown-report-file FILE Optional Markdown report output path for PR audit trails.
  --codex-command COMMAND     Codex CLI command. Default: codex.
  --codex-model MODEL         Optional model passed to Codex CLI.
  --gh-bin COMMAND            GitHub CLI command. Default: gh.
  --remote NAME               Git remote used for fetch/push. Default: origin.
  --post-status               Post a success commit status on the final head.
  --push                      Push the final head to the branch remote.
  --no-fetch                  Skip fetch of the base branch.
  --dry-run                   Print the prompt and commands without invoking Codex.
  -h, --help                  Show this help.
`;
}

export function parseArgs(argv) {
  const options = {
    baseRef: DEFAULT_BASE_REF,
    codexCommand: DEFAULT_CODEX_COMMAND,
    codexModel: "",
    context: DEFAULT_CONTEXT,
    dryRun: false,
    fetchBase: true,
    ghBin: DEFAULT_GH_BIN,
    headBranch: "",
    help: false,
    markdownReportFile: "",
    postStatus: false,
    push: false,
    remote: DEFAULT_REMOTE,
    reportFile: "",
    repoRoot: defaultRepoRoot,
    schemaFile: defaultSchemaFile,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = (name) => {
      const inline = `${name}=`;
      if (arg.startsWith(inline)) return arg.slice(inline.length);
      index += 1;
      if (index >= argv.length || argv[index].startsWith("--")) {
        throw usageError(`${name} requires a value`);
      }
      return argv[index];
    };

    if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else if (arg === "--base" || arg.startsWith("--base=")) {
      options.baseRef = value("--base");
    } else if (arg === "--head-branch" || arg.startsWith("--head-branch=")) {
      options.headBranch = value("--head-branch");
    } else if (arg === "--context" || arg.startsWith("--context=")) {
      options.context = value("--context");
    } else if (arg === "--repo" || arg.startsWith("--repo=")) {
      options.repoRoot = path.resolve(value("--repo"));
    } else if (arg === "--schema" || arg.startsWith("--schema=")) {
      options.schemaFile = path.resolve(value("--schema"));
    } else if (arg === "--report-file" || arg.startsWith("--report-file=")) {
      options.reportFile = path.resolve(value("--report-file"));
    } else if (arg === "--markdown-report-file" || arg.startsWith("--markdown-report-file=")) {
      options.markdownReportFile = path.resolve(value("--markdown-report-file"));
    } else if (arg === "--codex-command" || arg.startsWith("--codex-command=")) {
      options.codexCommand = value("--codex-command");
    } else if (arg === "--codex-model" || arg.startsWith("--codex-model=")) {
      options.codexModel = value("--codex-model");
    } else if (arg === "--gh-bin" || arg.startsWith("--gh-bin=")) {
      options.ghBin = value("--gh-bin");
    } else if (arg === "--remote" || arg.startsWith("--remote=")) {
      options.remote = value("--remote");
    } else if (arg === "--post-status") {
      options.postStatus = true;
    } else if (arg === "--push") {
      options.push = true;
    } else if (arg === "--no-fetch") {
      options.fetchBase = false;
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else {
      throw usageError(`unknown argument: ${arg}`);
    }
  }

  return options;
}

function usageError(message) {
  const error = new Error(message);
  error.exitCode = 2;
  return error;
}

function cleanString(value) {
  return typeof value === "string" ? value.trim() : "";
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

export function renderPrompt({ baseRef, headRef }) {
  return `You are the final autonomous quality pass for this branch.

Assume no human will review this and no further agent will clean it up. Your job is to leave the
best final system you can.

Use the provided clean branch worktree. The outer helper handles pushing and PR creation after you
return; do not run PR lifecycle helpers yourself.

Review the diff from ${baseRef} to ${headRef}.

AI behavior is outside your authority: do not create, alter, or approve it. Refactor AI code only
when behavior is preserved exactly.

Focus on:
1. Correctness bugs.
2. Architectural issues where the implementer made the locally easiest change instead of the change
   that leaves the overall system simplest.
3. Anything else important enough to improve before merge.

Ignore missing documentation updates and contract-documentation updates unless the omission directly
creates a correctness or architecture problem.

Patch-note generation is outside your authority. The earlier specialist patch-note pass is the sole
owner of every path under patch-notes/. Do not create, edit, delete, stage, or commit those paths.
Do not treat a patch-note concern as authorization to change the branch; leave patch-note content
untouched.

You may rewrite the branch. Prefer the simplest resulting system, not the smallest diff. If a better
path is clear and you can complete it coherently, take it. If the ideal rewrite is too large to finish
well in this pass, make only the improvements that leave the branch in a complete, coherent,
working state.

Commit the final state and run focused verification appropriate to what you changed.

Return JSON with:
{
  "verdict": "passed_unchanged | improved | improved_with_concerns",
  "summary": "...",
  "issues_found": [],
  "changes_made": [],
  "verification": [],
  "remaining_concerns": []
}
`;
}

export function buildCodexArgs({ repoRoot, gitCommonDir = "", schemaFile, reportFile, codexModel, prompt }) {
  const args = [
    "exec",
    "--cd",
    repoRoot,
  ];
  if (gitCommonDir) {
    args.push("--add-dir", gitCommonDir);
  }
  args.push(
    "--sandbox",
    "workspace-write",
    "-c",
    'approval_policy="never"',
    "--ephemeral",
    "--output-schema",
    schemaFile,
    "--output-last-message",
    reportFile,
  );
  if (codexModel) {
    args.push("--model", codexModel);
  }
  args.push(prompt);
  return args;
}

export function buildFetchArgs({ remote, baseRef }) {
  const remotePrefix = `${remote}/`;
  const branch = baseRef.startsWith(remotePrefix)
    ? baseRef.slice(remotePrefix.length)
    : baseRef.includes("/")
      ? ""
      : baseRef;
  if (!branch) {
    return ["fetch", remote, baseRef];
  }
  return ["fetch", remote, `+refs/heads/${branch}:refs/remotes/${remote}/${branch}`];
}

export function normalizeReport(raw) {
  const parsed = typeof raw === "string" ? parseJsonObject(raw) : raw;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("quality pass report must be a JSON object");
  }
  const verdict = cleanString(parsed.verdict);
  if (!VERDICTS.has(verdict)) {
    throw new Error(`quality pass report has invalid verdict: ${verdict || "<missing>"}`);
  }
  return {
    verdict,
    summary: cleanString(parsed.summary),
    issues_found: normalizeStringArray(parsed.issues_found),
    changes_made: normalizeStringArray(parsed.changes_made),
    verification: normalizeStringArray(parsed.verification),
    remaining_concerns: normalizeStringArray(parsed.remaining_concerns),
  };
}

export function resolveHeadBranch({ requestedHeadBranch, currentBranch }) {
  const current = cleanString(currentBranch);
  const requested = cleanString(requestedHeadBranch);
  if (!current) {
    throw new Error("quality pass requires a named current branch; detached HEAD is not supported");
  }
  if (requested && requested !== current) {
    throw new Error(`quality pass head branch mismatch: current branch is '${current}', but --head-branch was '${requested}'`);
  }
  return requested || current;
}

function normalizeStringArray(value) {
  return Array.isArray(value) ? value.map(cleanString).filter(Boolean) : [];
}

function parseJsonObject(raw) {
  const text = String(raw || "").trim();
  try {
    return JSON.parse(text);
  } catch {
    const fenced = /```(?:json)?\s*([\s\S]*?)```/i.exec(text);
    if (fenced) return JSON.parse(fenced[1]);
    const start = text.indexOf("{");
    const end = text.lastIndexOf("}");
    if (start >= 0 && end > start) return JSON.parse(text.slice(start, end + 1));
    throw new Error("quality pass report was not parseable JSON");
  }
}

export function markdownReport(report) {
  const list = (items) => (items.length ? items.map((item) => `- ${item}`).join("\n") : "- None.");
  return [
    "## Adversarial quality pass",
    "",
    `Verdict: ${report.verdict}`,
    "",
    "### Summary",
    "",
    report.summary || "Not recorded.",
    "",
    "### Issues found",
    "",
    list(report.issues_found),
    "",
    "### Changes made",
    "",
    list(report.changes_made),
    "",
    "### Verification",
    "",
    list(report.verification),
    "",
    "### Remaining concerns",
    "",
    list(report.remaining_concerns),
    "",
  ].join("\n");
}

export function statusDescription(report) {
  const prefix = report.verdict.replaceAll("_", " ");
  const suffix = report.remaining_concerns.length ? `; ${report.remaining_concerns.length} concern(s)` : "";
  return `${prefix}${suffix}`.slice(0, 140);
}

export function autoCommitBody(report) {
  const list = (items) => (items.length ? items.map((item) => `- ${item}`).join("\n") : "- None.");
  return [
    `Verdict: ${report.verdict}`,
    "",
    "Summary:",
    report.summary || "Not recorded.",
    "",
    "Issues found:",
    list(report.issues_found),
    "",
    "Changes made:",
    list(report.changes_made),
    "",
    "Verification:",
    list(report.verification),
    "",
    "Remaining concerns:",
    list(report.remaining_concerns),
  ].join("\n");
}

class Runner {
  constructor({ stdout = process.stdout, stderr = process.stderr, env = process.env } = {}) {
    this.stdout = stdout;
    this.stderr = stderr;
    this.env = env;
  }

  log(message) {
    this.stdout.write(`${message}\n`);
  }

  error(message) {
    this.stderr.write(`${message}\n`);
  }

  runCapture(command, args, options = {}) {
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: { ...process.env, ...this.env, ...(options.env || {}) },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    if (result.error) throw result.error;
    if (result.status !== 0) {
      const detail = result.stderr?.trim() || result.stdout?.trim() || `${command} exited ${result.status}`;
      throw new Error(detail);
    }
    return result.stdout.trim();
  }

  runInherit(command, args, options = {}) {
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: { ...process.env, ...this.env, ...(options.env || {}) },
      stdio: "inherit",
    });
    if (result.error) throw result.error;
    if (result.status !== 0) {
      throw new Error(`${command} ${args.join(" ")} exited ${result.status}`);
    }
  }

  git(args, repoRoot) {
    return this.runCapture("git", args, { cwd: repoRoot });
  }

  gitInherit(args, repoRoot) {
    this.runInherit("git", args, { cwd: repoRoot });
  }

  currentBranch(repoRoot) {
    return this.git(["branch", "--show-current"], repoRoot);
  }

  gitCommonDir(repoRoot) {
    return this.git(["rev-parse", "--path-format=absolute", "--git-common-dir"], repoRoot);
  }

  ensureClean(repoRoot) {
    const status = this.git(["status", "--porcelain=v1"], repoRoot);
    if (status) {
      throw new Error(`quality pass requires a clean worktree before starting:\n${status}`);
    }
  }

  assertPatchNotesUnchanged(repoRoot, beforeHead) {
    const trackedChanges = this.git(
      ["diff", "--name-only", "--no-renames", beforeHead, "--", "patch-notes"],
      repoRoot,
    ).split("\n").filter(Boolean);
    const untrackedChanges = this.git(
      ["ls-files", "--others", "--exclude-standard", "--", "patch-notes"],
      repoRoot,
    ).split("\n").filter(Boolean);
    const changedPaths = [...new Set([...trackedChanges, ...untrackedChanges])].sort();
    if (changedPaths.length > 0) {
      throw new Error(
        "adversarial quality pass must not modify specialist-owned patch notes:\n" +
        changedPaths.map((pathname) => `- ${pathname}`).join("\n"),
      );
    }
  }

  commitDirtyFinalState(repoRoot, report) {
    const status = this.git(["status", "--porcelain=v1"], repoRoot);
    if (!status) return false;
    this.gitInherit(["add", "-A"], repoRoot);
    this.gitInherit(["commit", "-m", "Run adversarial quality pass", "-m", autoCommitBody(report)], repoRoot);
    return true;
  }

  formatTouchedRust(repoRoot, baseRef) {
    this.runInherit(path.join(repoRoot, "scripts", "format-touched-rust.sh"), ["--base", baseRef], { cwd: repoRoot });
  }

  postStatus(options, headSha, report) {
    const args = [
      "api",
      "-X",
      "POST",
      `repos/:owner/:repo/statuses/${headSha}`,
      "-f",
      "state=success",
      "-f",
      `context=${options.context}`,
      "-f",
      `description=${statusDescription(report)}`,
    ];
    this.runInherit(options.ghBin, args, { cwd: options.repoRoot });
  }

  run(options) {
    if (options.help) {
      this.stdout.write(usage());
      return;
    }
    const repoRoot = options.repoRoot;
    if (!fs.existsSync(options.schemaFile)) {
      throw new Error(`missing quality pass schema: ${options.schemaFile}`);
    }
    const headBranch = resolveHeadBranch({
      requestedHeadBranch: options.headBranch,
      currentBranch: this.currentBranch(repoRoot),
    });
    const reportFile = options.reportFile || path.join(os.tmpdir(), `rts-adversarial-quality-pass-${process.pid}.json`);
    const gitCommonDir = this.gitCommonDir(repoRoot);
    const prompt = renderPrompt({ baseRef: options.baseRef, headRef: "HEAD" });
    const codexArgs = buildCodexArgs({
      repoRoot,
      gitCommonDir,
      schemaFile: options.schemaFile,
      reportFile,
      codexModel: options.codexModel,
      prompt,
    });

    if (options.dryRun) {
      this.log(`quality-pass: would run ${options.codexCommand} ${codexArgs.map(shellQuote).join(" ")}`);
      if (options.push) {
        this.log(`quality-pass: would push HEAD to ${options.remote}/${headBranch}`);
      }
      if (options.postStatus) {
        this.log(`quality-pass: would post ${options.context} status on final HEAD`);
      }
      if (options.markdownReportFile) {
        this.log(`quality-pass: would write Markdown report to ${options.markdownReportFile}`);
      }
      this.stdout.write(prompt);
      return;
    }

    this.ensureClean(repoRoot);
    if (options.fetchBase) {
      this.gitInherit(buildFetchArgs({ remote: options.remote, baseRef: options.baseRef }), repoRoot);
    }
    const beforeHead = this.git(["rev-parse", "HEAD"], repoRoot);

    this.log(`quality-pass: running Codex final quality pass for ${headBranch}`);
    this.runInherit(options.codexCommand, codexArgs, { cwd: repoRoot, env: { [QUALITY_PASS_ENV]: "1" } });
    this.assertPatchNotesUnchanged(repoRoot, beforeHead);
    if (!fs.existsSync(reportFile)) {
      throw new Error(`quality pass did not write report file: ${reportFile}`);
    }
    const report = normalizeReport(fs.readFileSync(reportFile, "utf8"));
    this.formatTouchedRust(repoRoot, options.baseRef);
    const autoCommitted = this.commitDirtyFinalState(repoRoot, report);
    const finalHead = this.git(["rev-parse", "HEAD"], repoRoot);
    if (autoCommitted) {
      this.log(`quality-pass: committed final dirty state at ${finalHead}`);
    } else if (finalHead !== beforeHead) {
      this.log(`quality-pass: Codex committed final state at ${finalHead}`);
    } else {
      this.log("quality-pass: final state unchanged");
    }
    if (options.markdownReportFile) {
      fs.writeFileSync(options.markdownReportFile, markdownReport(report));
    }

    if (options.push) {
      this.gitInherit(["push", "-u", options.remote, `HEAD:refs/heads/${headBranch}`], repoRoot);
    }
    if (options.postStatus) {
      this.postStatus(options, finalHead, report);
    }
    this.log(`quality-pass: verdict ${report.verdict}`);
    this.log(markdownReport(report));
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    const options = parseArgs(process.argv.slice(2));
    new Runner().run(options);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    if (error.exitCode === 2) {
      process.stderr.write(usage());
      process.exit(2);
    }
    process.exit(error.exitCode || 1);
  }
}
