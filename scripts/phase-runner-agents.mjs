#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { phaseMarkedDoneText } from "./plan-phase-status.mjs";

export { phaseMarkedDoneText };

const DEFAULT_WORKTREE_ROOT = "/tmp/rts-worktrees";
const DEFAULT_BASE_BRANCH = "main";
const DEFAULT_GH_BIN = "gh";

export function usage() {
  return `Usage:
  scripts/phase-runner-agents.mjs --plan NAME PHASE [PHASE ...] [options]

Examples:
  scripts/phase-runner-agents.mjs --plan faction 4 --pr
  scripts/phase-runner-agents.mjs --plan faction 5.5 --pr
  scripts/phase-runner-agents.mjs --plan faction phase-4 phase-5 --pr --wait
  scripts/phase-runner-agents.mjs --plan lab/room phase-0 --pr --wait
  scripts/phase-runner-agents.mjs --plan faction --from 5 --to 6 --pr --wait
  scripts/phase-runner-agents.mjs --plan ai 2 --model gpt-5.4-mini --pr

Runs executor passes only. Each phase gets a separate worktree and branch under
/tmp/rts-worktrees. Each phase starts from the current local main, then the
runner pushes the completed phase branch, opens or updates an owned PR, and
arms auto-merge. With --wait, the runner waits for that PR to merge and verifies
the phase head is reachable from origin/main before reporting success or
starting the next phase.
Without --wait, the runner stops after opening and arming the first phase PR so
serial follow-up does not start from an assumed merge; treat that as a pending
handoff, not completion.
Phase ids may be numeric, decimal interstitials such as 5.5, or suffixed ids
such as 3a. Use --from/--to to discover all phase files in that interval; for
example --from 5 --to 6 runs phase-5.5 before phase-6 if both files exist.

The runner never creates plans or performs final review. It never merges or
pushes main; GitHub auto-merge and the required PR checks own that lifecycle.
Calling agents should treat the inner executor as a long-running job:
wait for the command to finish, and if polling is unavoidable, poll no more than
once every 5 minutes. Do not tail the executor log during normal progress; the
runner prints the relevant tail on failure.

Options:
  --plan NAME          Plan directory under plans/. Nested subplans such as lab/room are allowed. Required.
  --base BRANCH        Must be main. Kept for compatibility with existing calls.
  --model MODEL        Optional model override for executor passes. Defaults to the caller Codex model when available.
  --from PHASE         Discover phases after PHASE, up to --to. Example: --from 5.
  --to PHASE           Discover phases through PHASE. Requires --from.
  --pr                 Push the phase branch, open/update an owned PR, arm auto-merge, and stop pending merge.
  --wait               With --pr, wait for each phase PR to merge before reporting success or continuing.
  --dry-run            Print worktrees, branches, and prompts without running an executor.
  -h, --help           Show this help.

Environment:
  RTS_WORKTREE_ROOT=/tmp/rts-worktrees
  GH_BIN=gh
  CODEX_MODEL          Optional caller model fallback when --model is not passed.
  CODEX_REASONING_EFFORT
                       Optional caller reasoning-effort fallback with CODEX_MODEL.
  CODEX_THREAD_ID      Used to inherit the active Codex session model and reasoning effort when available.
`;
}

export function parseArgs(argv) {
  const options = {
    baseBranch: DEFAULT_BASE_BRANCH,
    dryRun: false,
    prMode: false,
    waitForPr: false,
    ghBin: process.env.GH_BIN || DEFAULT_GH_BIN,
    model: "",
    planName: "",
    fromPhase: "",
    toPhase: "",
    phases: [],
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--plan":
        options.planName = requireValue(argv, ++index, "--plan");
        break;
      case "--base":
        options.baseBranch = requireValue(argv, ++index, "--base");
        break;
      case "--model":
        options.model = requireValue(argv, ++index, "--model");
        break;
      case "--from":
        options.fromPhase = requireValue(argv, ++index, "--from");
        break;
      case "--to":
        options.toPhase = requireValue(argv, ++index, "--to");
        break;
      case "--dry-run":
        options.dryRun = true;
        break;
      case "--pr":
        options.prMode = true;
        break;
      case "--wait":
        options.waitForPr = true;
        break;
      case "-h":
      case "--help":
        options.help = true;
        break;
      default:
        if (arg.startsWith("--")) {
          throw usageError(`unknown option: ${arg}`);
        }
        options.phases.push(arg);
        break;
    }
  }
  return options;
}

function requireValue(argv, index, optionName) {
  const value = argv[index];
  if (!value || value.startsWith("--")) {
    throw usageError(`${optionName} requires a value`);
  }
  return value;
}

function usageError(message) {
  const error = new Error(message);
  error.exitCode = 2;
  return error;
}

export function validateOptions(options) {
  if (!options.planName) {
    throw usageError("missing --plan");
  }
  if (options.prMode !== true) {
    throw usageError("phase-runner is PR-first now; pass --pr, optionally with --wait");
  }
  if (options.waitForPr && !options.prMode) {
    throw usageError("--wait requires --pr");
  }
  if ((options.fromPhase && !options.toPhase) || (!options.fromPhase && options.toPhase)) {
    throw usageError("--from and --to must be used together");
  }
  if (options.fromPhase && options.phases.length !== 0) {
    throw usageError("pass either explicit phases or --from/--to discovery, not both");
  }
  if (!isSafePlanName(options.planName)) {
    throw usageError(`plan name must be a safe plans/ directory name: ${options.planName}`);
  }
  if (options.baseBranch !== DEFAULT_BASE_BRANCH) {
    throw usageError("phase-runner opens PRs against main; --base must be main");
  }
}

export function phaseBaseRefForRun({ dryRun, baseBranch, baseBranchAvailable }) {
  return dryRun && !baseBranchAvailable ? "HEAD" : baseBranch;
}

export function parsePhase(raw) {
  const label = String(raw || "").replace(/^phase-/, "");
  const match = /^([0-9]+)(?:\.([0-9]+))?([a-z])?$/.exec(label);
  if (!match) {
    throw usageError(`invalid phase '${raw}'; use N, N.M, Na, phase-N, phase-N.M, or phase-Na`);
  }
  return {
    id: `phase-${label}`,
    major: Number(match[1]),
    decimal: match[2] == null ? null : Number(`0.${match[2]}`),
    suffix: match[3] || "",
  };
}

export function normalizePhase(raw) {
  return parsePhase(raw).id;
}

export function comparePhase(a, b) {
  return (
    a.major - b.major ||
    ((a.decimal ?? 0) - (b.decimal ?? 0)) ||
    Number(Boolean(a.suffix)) - Number(Boolean(b.suffix)) ||
    a.suffix.localeCompare(b.suffix)
  );
}

export function discoverPhases(planDir, from, to) {
  const fromKey = parsePhase(from);
  const toKey = parsePhase(to);
  if (comparePhase(fromKey, toKey) >= 0) {
    throw usageError(`--from must be before --to: ${from} .. ${to}`);
  }
  const phases = fs
    .readdirSync(planDir)
    .filter((name) => /^phase-[0-9]+(?:\.[0-9]+)?[a-z]?\.md$/.test(name))
    .map((name) => parsePhase(path.basename(name, ".md")))
    .filter((phase) => comparePhase(phase, fromKey) > 0 && comparePhase(phase, toKey) <= 0)
    .sort(comparePhase)
    .map((phase) => phase.id);
  if (phases.length === 0) {
    throw usageError(`no phase files discovered after ${from} through ${to}`);
  }
  return phases;
}

export function phaseMarkedDone(phaseFile) {
  return phaseMarkedDoneText(fs.readFileSync(phaseFile, "utf8"));
}

function isSafePlanName(planName) {
  return String(planName || "")
    .split("/")
    .every((segment) => /^[a-z0-9_.-]+$/.test(segment) && segment !== "." && segment !== "..");
}

function planSlug(planName) {
  return planName.split("/").join("-");
}

export function buildLayout({ worktreeRoot, planName, phaseId, branch }) {
  const planDirSlug = planSlug(planName);
  const worktreePath = path.join(worktreeRoot, `${planDirSlug}-${phaseId}`);
  const logDir = path.join(worktreeRoot, "phase-runner-logs", planDirSlug);
  const handoffDir = path.join(logDir, "handoffs");
  return {
    branch,
    worktreePath,
    logDir,
    handoffDir,
    handoffFile: path.join(handoffDir, `${phaseId}.json`),
    prBodyFile: path.join(logDir, `${phaseId}.pr-body.md`),
    codexLog: path.join(logDir, `${phaseId}.codex.log`),
    timingFile: path.join(logDir, `${phaseId}.timing.json`),
    activeMarkerDir: path.join(worktreeRoot, "phase-runner-active"),
    activeMarker: path.join(worktreeRoot, "phase-runner-active", branch.replaceAll("/", "__")),
  };
}

export function renderPrompt({ planName, phaseId, branch }) {
  return `$phase-runner

Execute exactly one planned phase in this RTS repository.

Plan: plans/${planName}/plan.md
Phase: plans/${planName}/${phaseId}.md
Current branch: ${branch}

This is an executor pass only:
- You are already running inside the assigned clean worktree for this phase. This satisfies the
  repository worktree requirement; do not create another worktree or switch to another checkout.
- Do not create or revise the overall plan.
- Do not run a final review pass.
- Do not merge, push, or open a PR; the outer phase runner handles branch push and PR automation after you commit.
- Implement only this phase.
- Stage and commit only files belonging to this phase.
- The phase is not completed until your task changes are committed successfully on ${branch}.
- Mark plans/${planName}/${phaseId}.md done if and only if the phase is committed successfully.
- Run the smallest targeted verification appropriate for the changed files.
- Commit with the normal git commit hook. Do not run the broad full local gate unless the phase
  explicitly requires it; GitHub Actions is the authoritative full gate after the PR opens.
- If the commit hook fails, do not return completed. Inspect the failure, keep working, run focused
  checks, and retry the commit until it succeeds.
- You may commit with --no-verify only for pure documentation changes or when you have conclusively
  confirmed the only failing hook check is unrelated to this phase. Document that evidence in the
  JSON handoff verification or notes.
- Do not run workspace-wide cargo fmt. The owned-PR lifecycle formats only branch-touched Rust
  files after the final quality pass, keeping unrelated formatter drift out of the final diff.
- Prefer plain filesystem renames/moves over git mv inside this sandboxed executor session.
- If the phase is ambiguous, too broad, blocked by failing verification or commit-hook failure you
  cannot repair, or needs human design/product input, stop and report status "blocked".
- Include focused verification, next-step notes, and manual-test notes detailed enough for the
  outer phase runner to write an owned PR body.

Return a compact JSON handoff matching the requested schema.
`;
}

export function verificationSummary(handoff) {
  const verification = Array.isArray(handoff.verification) ? handoff.verification : [];
  const text = verification.filter(Boolean).join("; ");
  return text || "Focused verification not recorded by executor.";
}

export function writePrBody(handoff, bodyFile) {
  const list = (items) => (Array.isArray(items) && items.length ? items.map((item) => `- ${item}`).join("\n") : "- Not recorded.");
  const text = [
    "## Phase runner handoff",
    "",
    `Status: ${handoff.status || "unknown"}`,
    "",
    "### Summary",
    "",
    handoff.summary || "Not recorded.",
    "",
    "### Files changed",
    "",
    list(handoff.files_changed),
    "",
    "### Focused verification",
    "",
    list(handoff.verification),
    "",
    "### Gameplay impact",
    "",
    handoff.gameplay_impact || "Not recorded.",
    "",
    "### Next executor notes",
    "",
    handoff.next_executor_notes || "Not recorded.",
    "",
    "### Manual test notes",
    "",
    handoff.manual_test_notes || "Not recorded.",
    "",
  ].join("\n");
  fs.writeFileSync(bodyFile, text);
}

export function firstPrFromGhList(prJson) {
  const data = typeof prJson === "string" ? JSON.parse(prJson || "[]") : prJson;
  return Array.isArray(data) ? data[0] : data;
}

export function ensurePrReady(prJson, branch) {
  const pr = firstPrFromGhList(prJson);
  if (!pr?.number) {
    throw new Error(`agent-pr did not leave an open PR for ${branch}`);
  }
  if (pr.state !== "OPEN") {
    throw new Error(`PR #${pr.number} is not open (${pr.state}): ${pr.url || ""}`);
  }
  if (!pr.autoMergeRequest) {
    throw new Error(`PR #${pr.number} is missing auto-merge: ${pr.url || ""}`);
  }
  if (pr.mergeStateStatus === "DIRTY") {
    throw new Error(`PR #${pr.number} has merge conflicts: ${pr.url || ""}`);
  }
  return pr;
}

export function enrichHandoffWithPr(handoff, prJson, phaseHead, waitState) {
  const pr = firstPrFromGhList(prJson);
  if (!pr) {
    return handoff;
  }
  return {
    ...handoff,
    pr_number: pr.number ?? null,
    pr_url: pr.url ?? "",
    head_sha: phaseHead || pr.headRefOid || "",
    auto_merge_state: pr.autoMergeRequest ? "armed" : "missing",
    merge_wait_state: waitState || "not_waited",
  };
}

export function writeJson(file, data) {
  fs.writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
}

function cleanString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : "";
}

function codexHomeForEnv(env) {
  if (cleanString(env.CODEX_HOME)) {
    return env.CODEX_HOME;
  }
  if (cleanString(env.HOME)) {
    return path.join(env.HOME, ".codex");
  }
  return "";
}

function findCodexSessionFiles(root, threadId) {
  const sessionsRoot = path.join(root, "sessions");
  if (!cleanString(threadId) || !fs.existsSync(sessionsRoot)) {
    return [];
  }
  const matches = [];
  const visit = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const entryPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        visit(entryPath);
      } else if (entry.isFile() && entry.name.includes(threadId) && entry.name.endsWith(".jsonl")) {
        matches.push(entryPath);
      }
    }
  };
  visit(sessionsRoot);
  return matches.sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
}

export function readCodexSessionExecutorConfig(sessionFile) {
  let inherited = { model: "", reasoningEffort: "", source: "" };
  for (const line of fs.readFileSync(sessionFile, "utf8").split(/\r?\n/)) {
    if (!line.trim()) {
      continue;
    }
    let item;
    try {
      item = JSON.parse(line);
    } catch {
      continue;
    }
    const payload = item?.payload;
    const model = cleanString(payload?.model) || cleanString(payload?.collaboration_mode?.settings?.model);
    const reasoningEffort = cleanString(payload?.collaboration_mode?.settings?.reasoning_effort) || cleanString(payload?.effort);
    if (model || reasoningEffort) {
      inherited = {
        model: model || inherited.model,
        reasoningEffort: reasoningEffort || inherited.reasoningEffort,
        source: "codex-session",
      };
    }
  }
  return inherited;
}

export function resolveCodexExecutorConfig({ explicitModel = "", env = process.env } = {}) {
  if (cleanString(explicitModel)) {
    return { model: cleanString(explicitModel), reasoningEffort: "", source: "explicit" };
  }

  const envModel = cleanString(env.CODEX_MODEL);
  if (envModel) {
    return {
      model: envModel,
      reasoningEffort: cleanString(env.CODEX_REASONING_EFFORT) || cleanString(env.CODEX_MODEL_REASONING_EFFORT),
      source: "env",
    };
  }

  const codexHome = codexHomeForEnv(env);
  for (const sessionFile of findCodexSessionFiles(codexHome, cleanString(env.CODEX_THREAD_ID))) {
    const inherited = readCodexSessionExecutorConfig(sessionFile);
    if (inherited.model) {
      return inherited;
    }
  }

  return { model: "", reasoningEffort: "", source: "codex-default" };
}

function tomlString(value) {
  return JSON.stringify(value);
}

export function buildCodexExecArgs({ worktreePath, gitCommonDir, schemaFile, handoffFile, executorConfig, prompt }) {
  const args = [
    "exec",
    "--cd",
    worktreePath,
    "--add-dir",
    gitCommonDir,
    "--sandbox",
    "workspace-write",
    "--output-schema",
    schemaFile,
    "--output-last-message",
    handoffFile,
  ];
  if (executorConfig?.model) {
    args.push("--model", executorConfig.model);
  }
  if (executorConfig?.reasoningEffort) {
    args.push("--config", `model_reasoning_effort=${tomlString(executorConfig.reasoningEffort)}`);
  }
  args.push(prompt);
  return args;
}

export class Runner {
  constructor({ env = process.env, stdout = process.stdout, stderr = process.stderr } = {}) {
    this.env = env;
    this.stdout = stdout;
    this.stderr = stderr;
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
    if (result.error) {
      throw result.error;
    }
    if (result.status !== 0) {
      if (options.allowFailure) {
        return "";
      }
      const detail = result.stderr?.trim() || result.stdout?.trim() || `${command} exited ${result.status}`;
      throw new Error(detail);
    }
    return result.stdout;
  }

  runInherit(command, args, options = {}) {
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: { ...process.env, ...this.env, ...(options.env || {}) },
      stdio: "inherit",
    });
    if (result.error) {
      throw result.error;
    }
    if (result.status !== 0) {
      throw new Error(`${command} ${args.join(" ")} exited ${result.status}`);
    }
  }

  runStatus(command, args, options = {}) {
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: { ...process.env, ...this.env, ...(options.env || {}) },
      stdio: "ignore",
    });
    if (result.error) {
      throw result.error;
    }
    return result.status ?? 1;
  }

  commandExists(command) {
    const result = spawnSync("sh", ["-c", `command -v "$1" >/dev/null 2>&1`, "sh", command], {
      env: { ...process.env, ...this.env },
    });
    return result.status === 0;
  }

  git(args, options = {}) {
    return this.runCapture("git", args, options).trim();
  }

  gitInherit(args, options = {}) {
    this.runInherit("git", args, options);
  }

  checkPrerequisites(options, repoRoot) {
    if (!options.dryRun && !this.commandExists("codex")) {
      throw usageError("codex CLI is not available on PATH");
    }
    if (!options.dryRun && !this.commandExists(options.ghBin)) {
      throw usageError(`${options.ghBin} is required to open and inspect PRs`);
    }
    if (!options.dryRun && !this.commandExists("jq")) {
      throw usageError("jq is required by PR helper scripts");
    }
    if (!options.dryRun && this.git(["status", "--porcelain=v1"], { cwd: repoRoot })) {
      throw usageError("current checkout is dirty; start from a clean checkout before running phases");
    }
  }

  syncMain(baseBranch, repoRoot) {
    this.gitInherit(["fetch", "origin", baseBranch], { cwd: repoRoot });
    this.gitInherit(["merge", "--ff-only", `origin/${baseBranch}`], { cwd: repoRoot });
  }

  resolvePhaseBaseRef(options, repoRoot) {
    const baseBranchAvailable =
      this.runStatus("git", ["rev-parse", "--verify", "--quiet", options.baseBranch], { cwd: repoRoot }) === 0;
    const phaseBaseRef = phaseBaseRefForRun({
      dryRun: options.dryRun,
      baseBranch: options.baseBranch,
      baseBranchAvailable,
    });
    if (phaseBaseRef !== options.baseBranch) {
      this.log(`phase-runner: dry run base ${options.baseBranch} is unavailable; previewing from HEAD`);
    }
    return phaseBaseRef;
  }

  getPrJson(branch, options, cwd) {
    return this.runCapture(
      options.ghBin,
      [
        "pr",
        "list",
        "--base",
        options.baseBranch,
        "--head",
        branch,
        "--state",
        "open",
        "--limit",
        "1",
        "--json",
        "number,url,state,headRefOid,headRefName,autoMergeRequest,mergeStateStatus,isDraft",
      ],
      { cwd },
    );
  }

  async runCodexCliExecutor({ worktreePath, gitCommonDir, schemaFile, handoffFile, executorConfig, prompt, codexLog }) {
    const args = buildCodexExecArgs({ worktreePath, gitCommonDir, schemaFile, handoffFile, executorConfig, prompt });
    const logFd = fs.openSync(codexLog, "w");
    try {
      const result = spawnSync("codex", args, {
        env: { ...process.env, ...this.env },
        stdio: ["ignore", logFd, logFd],
      });
      if (result.error) {
        throw result.error;
      }
      if (result.status !== 0) {
        const error = new Error(`Codex failed with status ${result.status}`);
        error.exitCode = result.status;
        throw error;
      }
    } finally {
      fs.closeSync(logFd);
    }
  }

  printLogTail(logFile) {
    try {
      const lines = fs.readFileSync(logFile, "utf8").split(/\r?\n/);
      for (const line of lines.slice(-80)) {
        if (line.length) {
          this.error(line);
        }
      }
    } catch {
      // Best effort, matching the shell runner's `tail ... || true`.
    }
  }

  async run(options) {
    validateOptions(options);
    const repoRoot = this.git(["rev-parse", "--show-toplevel"]);
    const gitCommonDir = this.git(["rev-parse", "--path-format=absolute", "--git-common-dir"], { cwd: repoRoot });
    this.checkPrerequisites(options, repoRoot);

    const planDir = path.join(repoRoot, "plans", options.planName);
    const planFile = path.join(planDir, "plan.md");
    const schemaFile = path.join(repoRoot, "scripts", "phase-runner-result.schema.json");
    if (!fs.existsSync(planFile)) {
      throw usageError(`missing plan entry point: ${planFile}`);
    }
    if (!fs.existsSync(schemaFile)) {
      throw usageError(`missing result schema: ${schemaFile}`);
    }
    const executorConfig = resolveCodexExecutorConfig({ explicitModel: options.model, env: this.env });
    if (executorConfig.source === "explicit") {
      this.log(`phase-runner: using explicit Codex model ${executorConfig.model}`);
    } else if (executorConfig.model) {
      const reasoning = executorConfig.reasoningEffort ? ` (reasoning ${executorConfig.reasoningEffort})` : "";
      this.log(`phase-runner: inheriting caller Codex model ${executorConfig.model}${reasoning}`);
    } else {
      this.log("phase-runner: using Codex CLI configured default model");
    }

    const worktreeRoot = this.env.RTS_WORKTREE_ROOT || DEFAULT_WORKTREE_ROOT;
    fs.mkdirSync(worktreeRoot, { recursive: true });
    const phases = [...options.phases];
    if (options.fromPhase) {
      phases.push(...discoverPhases(planDir, options.fromPhase, options.toPhase));
      this.log(`phase-runner: discovered phases: ${phases.join(" ")}`);
    }
    if (phases.length === 0) {
      throw usageError("missing phase");
    }

    if (!options.dryRun && this.git(["branch", "--show-current"], { cwd: repoRoot }) !== options.baseBranch) {
      throw usageError(`start phase-runner from the local ${options.baseBranch} checkout so each phase starts from main`);
    }
    if (!options.dryRun) {
      this.git(["remote", "get-url", "origin"], { cwd: repoRoot });
    }

    for (const rawPhase of phases) {
      const phaseId = normalizePhase(rawPhase);
      const phaseFile = path.join(planDir, `${phaseId}.md`);
      if (!fs.existsSync(phaseFile)) {
        throw usageError(`missing phase file: ${phaseFile}`);
      }

      const branch = `zvorygin/${options.planName}-${phaseId}`;
      const layout = buildLayout({ worktreeRoot, planName: options.planName, phaseId, branch });
      if (!options.dryRun && this.runStatus("git", ["show-ref", "--verify", "--quiet", `refs/heads/${branch}`], { cwd: repoRoot }) === 0) {
        throw usageError(`branch already exists: ${branch}`);
      }
      if (!options.dryRun && fs.existsSync(layout.worktreePath)) {
        throw usageError(`worktree path already exists: ${layout.worktreePath}`);
      }
      if (!options.dryRun) {
        this.log(`phase-runner: syncing local ${options.baseBranch} from origin/${options.baseBranch} before ${phaseId}`);
        this.syncMain(options.baseBranch, repoRoot);
      }
      const phaseBaseRef = this.resolvePhaseBaseRef(options, repoRoot);
      const phaseBaseCommit = this.git(["rev-parse", phaseBaseRef], { cwd: repoRoot });
      const prompt = renderPrompt({ planName: options.planName, phaseId, branch });
      const phaseStart = Date.now();

      this.log(`phase-runner: creating ${layout.worktreePath} from ${phaseBaseRef} (${phaseBaseCommit}) on ${branch}`);
      if (!options.dryRun) {
        this.gitInherit(["worktree", "add", layout.worktreePath, "-b", branch, phaseBaseRef], { cwd: repoRoot });
        fs.mkdirSync(layout.activeMarkerDir, { recursive: true });
        fs.writeFileSync(
          layout.activeMarker,
          `plan=${options.planName}\nphase=${phaseId}\nbranch=${branch}\nworktree=${layout.worktreePath}\n`,
        );
        fs.mkdirSync(layout.handoffDir, { recursive: true });
        fs.mkdirSync(layout.logDir, { recursive: true });
      }

      if (options.dryRun) {
        this.log(`phase-runner: would run Codex in ${layout.worktreePath}`);
        this.log(`phase-runner: would push ${branch} to origin`);
        this.log(
          `phase-runner: would run scripts/agent-pr.sh --base ${options.baseBranch} --head ${branch} --verification <executor verification>`,
        );
        if (options.waitForPr) {
          this.log("phase-runner: would run scripts/wait-pr.sh <opened-pr> before reporting success or continuing");
          this.log(`phase-runner: would fetch origin/${options.baseBranch} and verify the phase head is reachable from origin/${options.baseBranch}`);
        } else {
          this.log(`phase-runner: would stop with a pending handoff after arming auto-merge for ${branch}`);
        }
        this.stdout.write(prompt);
        if (!options.waitForPr) {
          break;
        }
        continue;
      }

      this.log(`phase-runner: running Codex executor for ${phaseId} (log: ${layout.codexLog})`);
      this.log("phase-runner: inner executor may run for 10-20 minutes; calling agents should wait and poll no more than once every 5 minutes");
      const executorStart = Date.now();
      try {
        await this.runCodexCliExecutor({
          worktreePath: layout.worktreePath,
          gitCommonDir,
          schemaFile,
          handoffFile: layout.handoffFile,
          executorConfig,
          prompt,
          codexLog: layout.codexLog,
        });
      } catch (error) {
        this.error(`phase-runner: Codex failed for ${phaseId}; leaving worktree at ${layout.worktreePath}`);
        this.error(`phase-runner: last 80 log lines from ${layout.codexLog}`);
        this.printLogTail(layout.codexLog);
        throw error;
      }
      const executorSeconds = Math.floor((Date.now() - executorStart) / 1000);

      const handoff = JSON.parse(fs.readFileSync(layout.handoffFile, "utf8"));
      if (handoff.status !== "completed") {
        this.error(`phase-runner: ${phaseId} reported status '${handoff.status || ""}'; leaving worktree for inspection: ${layout.worktreePath}`);
        this.error(`phase-runner: last 80 log lines from ${layout.codexLog}`);
        this.printLogTail(layout.codexLog);
        throw new Error(`${phaseId} did not complete`);
      }
      fs.rmSync(layout.activeMarker, { force: true });

      const dirtyStatus = this.git(["-C", layout.worktreePath, "status", "--porcelain=v1"], { cwd: repoRoot });
      if (dirtyStatus) {
        this.error(`phase-runner: ${phaseId} reported completed but left uncommitted changes; leaving worktree for inspection: ${layout.worktreePath}`);
        this.error(this.git(["-C", layout.worktreePath, "status", "--short"], { cwd: repoRoot }));
        this.error(`phase-runner: last 80 log lines from ${layout.codexLog}`);
        this.printLogTail(layout.codexLog);
        throw new Error(`${phaseId} left uncommitted changes`);
      }
      const commitCount = Number(this.git(["-C", layout.worktreePath, "rev-list", "--count", `${phaseBaseCommit}..HEAD`], { cwd: repoRoot }));
      if (commitCount === 0) {
        this.error(`phase-runner: ${phaseId} reported completed but created no commit over ${phaseBaseCommit}; leaving worktree for inspection: ${layout.worktreePath}`);
        this.error(`phase-runner: last 80 log lines from ${layout.codexLog}`);
        this.printLogTail(layout.codexLog);
        throw new Error(`${phaseId} created no commit`);
      }
      if (!phaseMarkedDone(path.join(layout.worktreePath, "plans", options.planName, `${phaseId}.md`))) {
        throw new Error(`phase-runner: ${phaseId} reported completed but did not mark the phase document done`);
      }

      let phaseHead = this.git(["-C", layout.worktreePath, "rev-parse", "HEAD"], { cwd: repoRoot });
      this.log(`phase-runner: executor committed ${branch} at ${phaseHead}`);
      this.log(`phase-runner: pushing ${branch} to origin`);
      this.gitInherit(["-C", layout.worktreePath, "push", "-u", "origin", branch], { cwd: repoRoot });

      writePrBody(handoff, layout.prBodyFile);
      this.log(`phase-runner: opening/updating owned PR for ${branch}`);
      this.runInherit(
        "scripts/agent-pr.sh",
        [
          "--base",
          options.baseBranch,
          "--head",
          branch,
          "--verification",
          verificationSummary(handoff),
          "--body-file",
          layout.prBodyFile,
        ],
        { cwd: layout.worktreePath, env: { GH_BIN: options.ghBin } },
      );
      const postQualityHead = this.git(["-C", layout.worktreePath, "rev-parse", "HEAD"], { cwd: repoRoot });
      if (postQualityHead !== phaseHead) {
        this.log(`phase-runner: quality pass updated ${branch} from ${phaseHead} to ${postQualityHead}`);
        phaseHead = postQualityHead;
      }

      const prJson = this.getPrJson(branch, options, layout.worktreePath);
      let pr;
      try {
        pr = ensurePrReady(prJson, branch);
      } catch (error) {
        writeJson(layout.handoffFile, enrichHandoffWithPr(handoff, prJson, phaseHead, "blocked"));
        this.error(`phase-runner: ${error.message}`);
        this.error(`phase-runner: PR lifecycle blocked; leaving worktree for repair: ${layout.worktreePath}`);
        throw error;
      }
      this.log(`phase-runner: PR #${pr.number} armed for auto-merge: ${pr.url}`);

      let mergeWaitState = "not_waited";
      if (options.waitForPr) {
        this.log(`phase-runner: waiting for PR #${pr.number} to merge before continuing`);
        try {
          this.runInherit("scripts/wait-pr.sh", [pr.url], { cwd: layout.worktreePath, env: { GH_BIN: options.ghBin } });
        } catch (error) {
          writeJson(layout.handoffFile, enrichHandoffWithPr(handoff, prJson, phaseHead, "blocked"));
          this.error(`phase-runner: PR #${pr.number} did not reach a merged state; leaving worktree for repair: ${layout.worktreePath}`);
          throw error;
        }
        this.gitInherit(["fetch", "origin", options.baseBranch], { cwd: repoRoot });
        try {
          this.git(["merge-base", "--is-ancestor", phaseHead, `origin/${options.baseBranch}`], { cwd: repoRoot });
        } catch (error) {
          writeJson(layout.handoffFile, enrichHandoffWithPr(handoff, prJson, phaseHead, "blocked"));
          this.error(`phase-runner: PR #${pr.number} merged, but ${phaseHead} is not reachable from origin/${options.baseBranch}`);
          throw error;
        }
        this.syncMain(options.baseBranch, repoRoot);
        mergeWaitState = "merged";
        this.log(`phase-runner: PR #${pr.number} merged and ${phaseHead} is reachable from origin/${options.baseBranch}`);
      }

      writeJson(layout.handoffFile, enrichHandoffWithPr(handoff, prJson, phaseHead, mergeWaitState));
      this.log(`phase-runner: ${phaseId} PR lifecycle recorded in ${layout.handoffFile}`);
      const totalSeconds = Math.floor((Date.now() - phaseStart) / 1000);
      writeJson(layout.timingFile, {
        plan: options.planName,
        phase: phaseId,
        branch,
        baseRef: phaseBaseCommit,
        phaseHead,
        pr: {
          number: Number(pr.number),
          url: pr.url,
          autoMergeState: "armed",
          mergeWaitState,
        },
        worktree: layout.worktreePath,
        codexLog: layout.codexLog,
        timingsSeconds: {
          executor: executorSeconds,
          total: totalSeconds,
        },
      });
      this.log(`phase-runner: timing saved to ${layout.timingFile} (${totalSeconds}s total)`);

      if (!options.waitForPr) {
        this.log(`phase-runner: stopped with a pending handoff after opening and arming PR #${pr.number} because --wait was not set`);
        break;
      }
    }

    if (options.dryRun) {
      this.log("phase-runner: dry run finished. No worktrees were created and no PRs were opened.");
    } else if (options.waitForPr) {
      this.log("phase-runner: finished executor passes. Each completed phase PR merged before the next phase started.");
    } else {
      this.log("phase-runner: finished with a pending handoff after arming the first phase PR. Run scripts/wait-pr.sh before claiming completion or starting follow-up work.");
    }
  }
}

function parseJsonHandoff(text) {
  const trimmed = text.trim();
  try {
    return JSON.parse(trimmed);
  } catch {
    const fenced = /```(?:json)?\s*([\s\S]*?)```/i.exec(trimmed);
    if (fenced) {
      return JSON.parse(fenced[1]);
    }
    const start = trimmed.indexOf("{");
    const end = trimmed.lastIndexOf("}");
    if (start >= 0 && end > start) {
      return JSON.parse(trimmed.slice(start, end + 1));
    }
    throw new Error("Agents SDK executor did not return parseable JSON handoff");
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    const options = parseArgs(process.argv.slice(2));
    if (options.help) {
      process.stdout.write(usage());
      process.exit(0);
    }
    await new Runner().run(options);
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    if (error.exitCode === 2) {
      process.stderr.write(usage());
      process.exit(2);
    }
    process.exit(error.exitCode || 1);
  }
}
