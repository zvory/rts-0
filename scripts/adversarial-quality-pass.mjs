#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { collectReviewInputs, excludedRawPaths, renderReviewInputManifest } from "./review-inputs.mjs";

const DEFAULT_BASE_REF = "origin/main";
const DEFAULT_CONTEXT = "adversarial-quality-pass";
const DEFAULT_REMOTE = "origin";
const DEFAULT_CODEX_COMMAND = "codex";
const DEFAULT_GH_BIN = "gh";
const VERDICTS = new Set(["passed_unchanged", "improved", "improved_with_concerns"]);
const REVIEW_MODES = new Set(["full", "incremental", "already-reviewed"]);
export const QUALITY_PASS_ENV = "RTS_ADVERSARIAL_QUALITY_PASS";
export const MAX_PRIOR_FOCUSED_VERIFICATION_CHARS = 2000;
export const PRIOR_FOCUSED_VERIFICATION_NOT_SUPPLIED = "not supplied";

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
  --existing-pr-body-file FILE Existing open PR body used to recover a reviewed-head anchor.
  --existing-pr-base BRANCH  Existing open PR target branch for anchor validation.
  --existing-pr-head SHA     Existing open PR head for local-head validation.
  --expected-base BRANCH     Expected PR target branch. Default: derived from --base.
  --prior-focused-verification TEXT
                             Implementer-supplied focused verification evidence (at most ${MAX_PRIOR_FOCUSED_VERIFICATION_CHARS} characters).
  --review-metadata-file FILE Wrapper-owned selected review mode/base output path.
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
    existingPrBase: "",
    existingPrBodyFile: "",
    existingPrHead: "",
    ghBin: DEFAULT_GH_BIN,
    headBranch: "",
    help: false,
    markdownReportFile: "",
    postStatus: false,
    priorFocusedVerification: "",
    push: false,
    remote: DEFAULT_REMOTE,
    reportFile: "",
    reviewMetadataFile: "",
    repoRoot: defaultRepoRoot,
    schemaFile: defaultSchemaFile,
    expectedBase: "",
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
    } else if (arg === "--existing-pr-body-file" || arg.startsWith("--existing-pr-body-file=")) {
      options.existingPrBodyFile = path.resolve(value("--existing-pr-body-file"));
    } else if (arg === "--existing-pr-base" || arg.startsWith("--existing-pr-base=")) {
      options.existingPrBase = value("--existing-pr-base");
    } else if (arg === "--existing-pr-head" || arg.startsWith("--existing-pr-head=")) {
      options.existingPrHead = value("--existing-pr-head");
    } else if (arg === "--expected-base" || arg.startsWith("--expected-base=")) {
      options.expectedBase = value("--expected-base");
    } else if (arg === "--prior-focused-verification" || arg.startsWith("--prior-focused-verification=")) {
      options.priorFocusedVerification = value("--prior-focused-verification");
    } else if (arg === "--review-metadata-file" || arg.startsWith("--review-metadata-file=")) {
      options.reviewMetadataFile = path.resolve(value("--review-metadata-file"));
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

  options.priorFocusedVerification = normalizePriorFocusedVerification(options.priorFocusedVerification);
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

export function normalizePriorFocusedVerification(value) {
  const verification = cleanString(value);
  if (!verification) return PRIOR_FOCUSED_VERIFICATION_NOT_SUPPLIED;
  if (verification.length > MAX_PRIOR_FOCUSED_VERIFICATION_CHARS) {
    throw usageError(
      `--prior-focused-verification exceeds the ${MAX_PRIOR_FOCUSED_VERIFICATION_CHARS}-character limit`,
    );
  }
  return verification;
}

function expectedBaseFromRef(baseRef) {
  const trimmed = cleanString(baseRef);
  const slash = trimmed.lastIndexOf("/");
  return slash >= 0 ? trimmed.slice(slash + 1) : trimmed;
}

export function reviewedHeadMarker(sha) {
  return `<!-- rts-agent-pr:reviewed-head:v1 sha=${sha} -->`;
}

// GitHub returns commit statuses in reverse chronological order. Only the newest value for the
// required context represents its current result; an earlier success may have been superseded.
export function hasLatestSuccessfulStatus(statuses, context) {
  if (!Array.isArray(statuses)) {
    throw new Error("GitHub status lookup did not return an array");
  }
  return statuses.find((status) => status?.context === context)?.state === "success";
}

export function parseReviewedHeadMarker(body) {
  const wrappers = String(body || "").match(/<!-- rts-agent-pr:v1 -->[\s\S]*?<!-- \/rts-agent-pr -->/g) || [];
  if (wrappers.length !== 1) {
    return { sha: "", reason: "missing wrapper-owned reviewed-head marker" };
  }
  const wrapper = wrappers[0];
  if (!/^Agent-Owned: true$/m.test(wrapper)) {
    return { sha: "", reason: "reviewed-head wrapper is not agent-owned" };
  }
  const markers = wrapper.match(/<!-- rts-agent-pr:reviewed-head:v1[\s\S]*?-->/g) || [];
  if (markers.length === 0) {
    return { sha: "", reason: "missing reviewed-head marker" };
  }
  if (markers.length !== 1) {
    return { sha: "", reason: "duplicate reviewed-head markers" };
  }
  const match = /^<!-- rts-agent-pr:reviewed-head:v1 sha=([0-9a-f]{40}) -->$/.exec(markers[0]);
  if (!match) {
    return { sha: "", reason: "malformed reviewed-head marker" };
  }
  return { sha: match[1], reason: "" };
}

function fullReview({ baseRef, reason }) {
  return {
    mode: "full",
    label: "Full",
    reviewBase: baseRef,
    reviewedHead: "",
    reason,
  };
}

// Keep mode selection small and dependency-injected so the safety fallbacks have direct coverage
// without relying on a live GitHub PR fixture.
export function selectReviewMode({
  baseRef,
  currentHead,
  existingPrBodyFile = "",
  existingPrBase = "",
  existingPrHead = "",
  expectedBase = "",
  getSuccessfulStatus,
  commitExists,
  isAncestor,
  hasMergeCommit,
}) {
  if (!existingPrBodyFile) {
    return fullReview({ baseRef, reason: "new PR has no reviewed-head anchor" });
  }
  if (cleanString(existingPrBase) !== cleanString(expectedBase || expectedBaseFromRef(baseRef))) {
    return fullReview({ baseRef, reason: "existing PR targets an unexpected base branch" });
  }
  if (cleanString(existingPrHead) !== cleanString(currentHead)) {
    return fullReview({ baseRef, reason: "existing PR head does not match local HEAD" });
  }

  const marker = parseReviewedHeadMarker(existingPrBodyFile);
  if (!marker.sha) {
    return fullReview({ baseRef, reason: marker.reason });
  }

  try {
    if (!getSuccessfulStatus(marker.sha)) {
      return fullReview({ baseRef, reason: "reviewed-head lacks a successful adversarial-quality-pass status" });
    }
  } catch {
    return fullReview({ baseRef, reason: "GitHub status lookup failed" });
  }
  if (!commitExists(marker.sha)) {
    return fullReview({ baseRef, reason: "reviewed-head is missing locally" });
  }
  if (marker.sha === currentHead) {
    return {
      mode: "already-reviewed",
      label: "Already Reviewed",
      reviewBase: marker.sha,
      reviewedHead: marker.sha,
      reason: "current HEAD already has the verified reviewed-head anchor",
    };
  }
  if (!isAncestor(marker.sha, currentHead)) {
    return fullReview({ baseRef, reason: "reviewed-head is not an ancestor of HEAD" });
  }
  if (hasMergeCommit(marker.sha, currentHead)) {
    return fullReview({ baseRef, reason: "the correction range contains a merge commit" });
  }
  return {
    mode: "incremental",
    label: "Incremental",
    reviewBase: marker.sha,
    reviewedHead: marker.sha,
    reason: "verified reviewed-head has a simple linear correction range",
  };
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

export function renderPrompt({
  baseRef,
  headRef,
  reviewMode = "full",
  reviewedHead = "",
  reviewInputs = [],
  priorFocusedVerification = "",
}) {
  const exclusions = excludedRawPaths(reviewInputs);
  const priorVerification = normalizePriorFocusedVerification(priorFocusedVerification);
  const incrementalInstruction = reviewMode === "incremental"
    ? `
This is an incremental review. The branch was fully reviewed through ${reviewedHead}. Review only the
new correction range from ${baseRef} to ${headRef}, plus how that correction interacts with the
already reviewed result. Do not reopen unchanged earlier branch work for a second pass.
`
    : "";
  return `You are the final autonomous quality pass for this branch.

Assume no human will review this and no further agent will clean it up. Your job is to leave the
best final system you can.

Use the provided clean branch worktree. The outer helper handles pushing and PR creation after you
return; do not run PR lifecycle helpers yourself.

Review the human-authored parts of the diff from ${baseRef} to ${headRef}. Generated, binary, and
oversized artifacts are deliberately represented only by bounded metadata below. Do not print,
decode, cat, git-show, or git-diff their raw contents. Review their generators, source inputs,
tests, and compact metadata instead.

Review mode: ${reviewMode === "incremental" ? "Incremental" : "Full"}.
Review base: ${baseRef}.
${incrementalInstruction}

Changed-path metadata:
${renderReviewInputManifest(reviewInputs)}

Raw-content exclusions:
${exclusions.length ? exclusions.join("\n") : "<none>"}

Prior focused verification:
${priorVerification}

Treat the prior focused verification as claimed evidence to evaluate against the diff. It does not
prove that a check passed or that it was adequate. Do not repeat an expensive check already supported
by adequate supplied evidence merely to make the report longer.

Verification boundary: this workspace-write sandbox may run offline, deterministic focused tests,
linters, static policy scripts, format checks, and repository inspections. Do not start HTTP or WebSocket listeners,
browsers, Chrome, Interact, Tailnet preview, or other validation that needs unavailable machine or
network access. Known sandbox restrictions are not remaining concerns by themselves. If a material
behavior lacks adequate supplied evidence and cannot be checked offline, record the exact behavior
still unverified and why it matters; do not report generic EPERM, sandbox, or unavailable-tool
concerns.

AI behavior is outside your authority: do not create, alter, or approve it. Refactor AI code only
when behavior is preserved exactly.

Focus on:
1. Correctness bugs.
2. Architectural issues where the implementer made the locally easiest change instead of the change
   that leaves the overall system simplest.
3. Anything else important enough to improve before merge.

Ignore missing documentation updates and contract-documentation updates unless the omission directly
creates a correctness or architecture problem.

You may fix correctness and architecture issues, but do not broaden scope into opportunistic cleanup
or a new validation harness.

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

export function buildCodexArgs({ repoRoot, gitCommonDir = "", schemaFile, reportFile, codexModel }) {
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
  // Keep the prompt and bounded changed-path manifest off the process command line. This avoids
  // operating-system argument limits on large branches and matches the specialist-pass boundary.
  args.push("-");
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

  runSucceeds(command, args, options = {}) {
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: { ...process.env, ...this.env, ...(options.env || {}) },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    if (result.error) throw result.error;
    return result.status === 0;
  }

  runInherit(command, args, options = {}) {
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: { ...process.env, ...this.env, ...(options.env || {}) },
      input: options.input,
      stdio: options.input === undefined ? "inherit" : ["pipe", "inherit", "inherit"],
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

  commitDirtyFinalState(repoRoot, report) {
    const status = this.git(["status", "--porcelain=v1"], repoRoot);
    if (!status) return false;
    this.gitInherit(["add", "-A"], repoRoot);
    this.gitInherit(["commit", "-m", "Run adversarial quality pass", "-m", autoCommitBody(report)], repoRoot);
    return true;
  }

  formatTouchedRust(repoRoot, baseRef) {
    const formatter = path.join(repoRoot, "scripts", "format-touched-rust.sh");
    if (process.platform === "win32") {
      this.runInherit("bash", [formatter, "--base", baseRef], { cwd: repoRoot });
      return;
    }
    this.runInherit(formatter, ["--base", baseRef], { cwd: repoRoot });
  }

  runPreflight(repoRoot, baseRef, { dryRun = false } = {}) {
    const args = [
      path.join(repoRoot, "scripts", "agent-pr-preflight.mjs"),
      "--repo",
      repoRoot,
      "--base",
      baseRef,
    ];
    if (dryRun) args.push("--dry-run");
    this.runInherit(process.execPath, args, { cwd: repoRoot });
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

  hasSuccessfulStatus(options, sha) {
    const raw = this.runCapture(options.ghBin, [
      "api",
      `repos/:owner/:repo/commits/${sha}/statuses?per_page=100`,
    ], { cwd: options.repoRoot });
    const statuses = JSON.parse(raw);
    return hasLatestSuccessfulStatus(statuses, options.context);
  }

  chooseReviewMode(options, repoRoot, currentHead) {
    const existingPrBody = options.existingPrBodyFile
      ? fs.readFileSync(options.existingPrBodyFile, "utf8")
      : "";
    return selectReviewMode({
      baseRef: options.baseRef,
      currentHead,
      existingPrBodyFile: options.existingPrBodyFile ? existingPrBody : "",
      existingPrBase: options.existingPrBase,
      existingPrHead: options.existingPrHead,
      expectedBase: options.expectedBase || expectedBaseFromRef(options.baseRef),
      getSuccessfulStatus: (sha) => this.hasSuccessfulStatus(options, sha),
      commitExists: (sha) => this.runSucceeds("git", ["cat-file", "-e", `${sha}^{commit}`], { cwd: repoRoot }),
      isAncestor: (ancestor, descendant) => this.runSucceeds(
        "git",
        ["merge-base", "--is-ancestor", ancestor, descendant],
        { cwd: repoRoot },
      ),
      hasMergeCommit: (ancestor, descendant) => Boolean(this.git(["rev-list", "--merges", `${ancestor}..${descendant}`], repoRoot)),
    });
  }

  writeReviewMetadata(options, metadata) {
    if (!options.reviewMetadataFile) return;
    fs.writeFileSync(options.reviewMetadataFile, `${JSON.stringify(metadata, null, 2)}\n`);
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
    const buildReviewInvocation = (selection) => {
      const reviewInputs = collectReviewInputs({ repoRoot, baseRef: selection.reviewBase });
      return {
        prompt: renderPrompt({
          baseRef: selection.reviewBase,
          headRef: "HEAD",
          reviewMode: selection.mode,
          reviewedHead: selection.reviewedHead,
          reviewInputs,
          priorFocusedVerification: options.priorFocusedVerification,
        }),
        codexArgs: buildCodexArgs({
          repoRoot,
          gitCommonDir,
          schemaFile: options.schemaFile,
          reportFile,
          codexModel: options.codexModel,
        }),
      };
    };

    if (options.dryRun) {
      const selection = fullReview({ baseRef: options.baseRef, reason: "dry run assumes a new PR" });
      const { prompt, codexArgs } = buildReviewInvocation(selection);
      this.log(`quality-pass: would run ${options.codexCommand} ${codexArgs.map(shellQuote).join(" ")}`);
      this.runPreflight(repoRoot, options.baseRef, { dryRun: true });
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
    const selection = this.chooseReviewMode(options, repoRoot, beforeHead);
    if (!REVIEW_MODES.has(selection.mode)) {
      throw new Error(`quality pass selected unsupported review mode: ${selection.mode}`);
    }
    if (selection.mode === "full") {
      this.log(`quality-pass: using Full review (${selection.reason})`);
    } else if (selection.mode === "already-reviewed") {
      this.log("quality-pass: HEAD is already verified as reviewed; skipping Codex, push, and status");
      this.writeReviewMetadata(options, {
        mode: selection.label,
        reviewBase: selection.reviewBase,
        reviewedHead: beforeHead,
        preservePriorReport: true,
      });
      return;
    } else {
      this.log(`quality-pass: using Incremental review from ${selection.reviewBase}`);
    }
    // Collect after fetching so metadata, exclusions, and the base ref inspected by Codex all
    // describe the same repository state.
    const { prompt, codexArgs } = buildReviewInvocation(selection);

    this.log(`quality-pass: running Codex final quality pass for ${headBranch}`);
    this.runInherit(options.codexCommand, codexArgs, {
      cwd: repoRoot,
      env: { [QUALITY_PASS_ENV]: "1" },
      input: prompt,
    });
    if (!fs.existsSync(reportFile)) {
      throw new Error(`quality pass did not write report file: ${reportFile}`);
    }
    const report = normalizeReport(fs.readFileSync(reportFile, "utf8"));
    this.formatTouchedRust(repoRoot, options.baseRef);
    const autoCommitted = this.commitDirtyFinalState(repoRoot, report);
    const reviewedHead = this.git(["rev-parse", "HEAD"], repoRoot);
    if (autoCommitted) {
      this.log(`quality-pass: committed final dirty state at ${reviewedHead}`);
    } else if (reviewedHead !== beforeHead) {
      this.log(`quality-pass: Codex committed final state at ${reviewedHead}`);
    } else {
      this.log("quality-pass: final state unchanged");
    }
    // Execute the helper from disk so a review-time fix to its implementation is itself what
    // verifies the final committed head before any external mutation.
    this.runPreflight(repoRoot, options.baseRef);
    const finalHead = this.git(["rev-parse", "HEAD"], repoRoot);
    if (options.markdownReportFile) {
      fs.writeFileSync(options.markdownReportFile, markdownReport(report));
    }
    this.writeReviewMetadata(options, {
      mode: selection.label,
      reviewBase: selection.reviewBase,
      reviewedHead: finalHead,
      preservePriorReport: false,
    });

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

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
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
