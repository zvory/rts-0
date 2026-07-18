#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { renderMarkdown, writeFullPrBody, writeOutputs } from "./docdrift-render.mjs";

const defaultRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultCheckpointFile = ".docdrift/checkpoint.txt";
const defaultCheckpointSeedFile = "docs/docdrift-checkpoint.txt";
const defaultTraceMapPath = "docs/doc-map.json";
const defaultClassifierCacheDir = ".docdrift/classifier-cache";
const defaultDocPatchCacheDir = ".docdrift/doc-patch-cache";
const defaultRunRoot = ".docdrift/runs";
const defaultSweepBranch = "zvorygin/docdrift-sweep";
const defaultSweepWorktree = ".docdrift/worktrees/docdrift-sweep";
const runStateSchemaVersion = 1;
const legacySweepBranch = "zvorygin/docdrift-sweep";
const legacyKnownHeadPrefix = "68f6e958";
const classifierPromptVersion = "docdrift-classifier-v1";
const docPatchPromptVersion = "docdrift-doc-patch-v1";
const validClassifierDecisions = new Set(["move_on", "update_docs"]);
const validDocPatchPrefixes = ["docs/design/", "docs/context/"];
const maxDocSectionsPerTarget = 3;
const maxDocSectionChars = 3000;

function usage() {
  console.log(`Usage:
  node scripts/docdrift-sweep.mjs --dry-run [options]
  node scripts/docdrift-sweep.mjs --classify [options]
  node scripts/docdrift-sweep.mjs --generate-docs [options]
  node scripts/docdrift-sweep.mjs --full [--dry-run] [options]

Options:
  --base REF                  Override the reviewed checkpoint ref.
  --classify                  Run the cheap Codex-backed classifier for considered commits.
  --classifier-cache DIR      Cache directory for classifier records. Default: ${defaultClassifierCacheDir}
  --codex-arg ARG             Extra Codex CLI argument for live classify mode. Repeatable.
  --codex-command COMMAND     Codex CLI command for live classify mode. Default: codex
  --codex-model MODEL         Optional model passed to Codex CLI with --model.
  --checkpoint-file PATH      Local checkpoint used when --base is omitted. Default: ${defaultCheckpointFile}
  --checkpoint-seed-file PATH Committed fallback checkpoint. Default: ${defaultCheckpointSeedFile}
  --checkpoint-ref REF        Optional checkpoint ref used when --base is omitted.
  --doc-patch-cache DIR       Cache directory for generated doc patch records. Default: ${defaultDocPatchCacheDir}
  --dry-run                   Build the deterministic Phase 1 report without classification; with --full, preview lifecycle without mutation.
  --fixture NAME_OR_PATH      Fixture response set for --classify/--generate-docs --no-codex.
  --full                      Run the full PR/checkpoint lifecycle.
  --generate-docs             Generate and apply minimal docs patches for update_docs decisions.
  --head REF                  Sweep target. Default: origin/main.
  --pr-title TITLE            PR title for --full. Default: Documentation drift sweep.
  --run-id ID                 Run id for --full reports. Default: current timestamp.
  --run-root DIR              Root directory for --full reports. Default: ${defaultRunRoot}
  --sweep-branch BRANCH       Branch for --full sweep PRs. Default: ${defaultSweepBranch}
  --sweep-worktree DIR        Isolated worktree for --full. Default: ${defaultSweepWorktree}
  --adopt-legacy              Explicitly adopt a verified terminal fixed-branch run with an unknown head.
  --trace-map PATH            Trace map JSON path. Default: ${defaultTraceMapPath}
  --format markdown|json      Stdout format. Default: markdown.
  --max-commits N             Max considered commits for one classify run. Default: 25
  --max-doc-prompt-tokens N   Max estimated doc patch prompt tokens per commit. Default: 8000
  --max-prompt-tokens N       Max estimated prompt tokens per commit. Default: 4000
  --max-total-doc-prompt-tokens N Max estimated doc patch prompt tokens across one run. Default: 40000
  --max-total-prompt-tokens N Max estimated prompt tokens across one classify run. Default: 20000
  --no-codex                  Do not invoke Codex; requires --fixture in classify mode.
  --out-dir DIR               Also write docdrift-sweep.md and docdrift-sweep.json.
  --repo DIR                  Repository root. Default: current RTS checkout.
  -h, --help                  Show this help.

Dry-run mode reads commit metadata and trace-map routing only. Classify mode sends bounded commit
metadata to Codex CLI, caches decision records, and never edits docs. Generate-docs mode feeds only
update_docs decisions and targeted design-doc sections to Codex CLI, applies exact minimal doc
patches in the working tree, and never creates PRs or advances the checkpoint. Full mode creates or
reuses an isolated worktree, generates one docs sweep branch, opens or updates the owned PR, waits
for merge, and advances the local checkpoint only after the processed head is safe.`);
}

export function parseArgs(argv) {
  const options = {
    base: null,
    classifierCacheDir: defaultClassifierCacheDir,
    classify: false,
    codexArgs: [],
    codexCommand: "codex",
    codexModel: null,
    codexTimeoutSeconds: 300,
    checkpointFile: defaultCheckpointFile,
    checkpointSeedFile: defaultCheckpointSeedFile,
    checkpointRef: null,
    docPatchCacheDir: defaultDocPatchCacheDir,
    dryRun: false,
    fixture: null,
    format: "markdown",
    full: false,
    generateDocs: false,
    head: "origin/main",
    maxCommits: 25,
    maxDocPromptTokens: 8000,
    maxPromptTokens: 4000,
    maxTotalDocPromptTokens: 40000,
    maxTotalPromptTokens: 20000,
    noCodex: false,
    outDir: null,
    prTitle: "Documentation drift sweep",
    repoRoot: defaultRepoRoot,
    runId: null,
    runRoot: defaultRunRoot,
    sweepBranch: defaultSweepBranch,
    sweepWorktree: defaultSweepWorktree,
    adoptLegacy: false,
    traceMap: defaultTraceMapPath,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const readValue = (name) => {
      const inlinePrefix = `${name}=`;
      if (arg.startsWith(inlinePrefix)) {
        return arg.slice(inlinePrefix.length);
      }
      index += 1;
      if (index >= argv.length || argv[index].startsWith("-")) {
        throw new Error(`${name} requires a value`);
      }
      return argv[index];
    };

    if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else if (arg === "--classify") {
      options.classify = true;
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--generate-docs") {
      options.generateDocs = true;
    } else if (arg === "--base" || arg.startsWith("--base=")) {
      options.base = readValue("--base");
    } else if (arg === "--classifier-cache" || arg.startsWith("--classifier-cache=")) {
      options.classifierCacheDir = readValue("--classifier-cache");
    } else if (arg === "--codex-arg" || arg.startsWith("--codex-arg=")) {
      options.codexArgs.push(readValue("--codex-arg"));
    } else if (arg === "--codex-command" || arg.startsWith("--codex-command=")) {
      options.codexCommand = readValue("--codex-command");
    } else if (arg === "--codex-model" || arg.startsWith("--codex-model=")) {
      options.codexModel = readValue("--codex-model");
    } else if (arg === "--codex-timeout-seconds" || arg.startsWith("--codex-timeout-seconds=")) {
      options.codexTimeoutSeconds = parsePositiveInteger(readValue("--codex-timeout-seconds"), "--codex-timeout-seconds");
    } else if (arg === "--checkpoint-ref" || arg.startsWith("--checkpoint-ref=")) {
      options.checkpointRef = readValue("--checkpoint-ref");
    } else if (arg === "--checkpoint-file" || arg.startsWith("--checkpoint-file=")) {
      options.checkpointFile = readValue("--checkpoint-file");
    } else if (arg === "--checkpoint-seed-file" || arg.startsWith("--checkpoint-seed-file=")) {
      options.checkpointSeedFile = readValue("--checkpoint-seed-file");
    } else if (arg === "--doc-patch-cache" || arg.startsWith("--doc-patch-cache=")) {
      options.docPatchCacheDir = readValue("--doc-patch-cache");
    } else if (arg === "--fixture" || arg.startsWith("--fixture=")) {
      options.fixture = readValue("--fixture");
    } else if (arg === "--full") {
      options.full = true;
    } else if (arg === "--head" || arg.startsWith("--head=")) {
      options.head = readValue("--head");
    } else if (arg === "--max-commits" || arg.startsWith("--max-commits=")) {
      options.maxCommits = parsePositiveInteger(readValue("--max-commits"), "--max-commits");
    } else if (arg === "--max-doc-prompt-tokens" || arg.startsWith("--max-doc-prompt-tokens=")) {
      options.maxDocPromptTokens = parsePositiveInteger(readValue("--max-doc-prompt-tokens"), "--max-doc-prompt-tokens");
    } else if (arg === "--max-prompt-tokens" || arg.startsWith("--max-prompt-tokens=")) {
      options.maxPromptTokens = parsePositiveInteger(readValue("--max-prompt-tokens"), "--max-prompt-tokens");
    } else if (arg === "--max-total-doc-prompt-tokens" || arg.startsWith("--max-total-doc-prompt-tokens=")) {
      options.maxTotalDocPromptTokens = parsePositiveInteger(
        readValue("--max-total-doc-prompt-tokens"),
        "--max-total-doc-prompt-tokens",
      );
    } else if (arg === "--max-total-prompt-tokens" || arg.startsWith("--max-total-prompt-tokens=")) {
      options.maxTotalPromptTokens = parsePositiveInteger(
        readValue("--max-total-prompt-tokens"),
        "--max-total-prompt-tokens",
      );
    } else if (arg === "--no-codex") {
      options.noCodex = true;
    } else if (arg === "--trace-map" || arg.startsWith("--trace-map=")) {
      options.traceMap = readValue("--trace-map");
    } else if (arg === "--format" || arg.startsWith("--format=")) {
      options.format = readValue("--format");
    } else if (arg === "--out-dir" || arg.startsWith("--out-dir=")) {
      options.outDir = readValue("--out-dir");
    } else if (arg === "--pr-title" || arg.startsWith("--pr-title=")) {
      options.prTitle = readValue("--pr-title");
    } else if (arg === "--repo" || arg.startsWith("--repo=")) {
      options.repoRoot = readValue("--repo");
    } else if (arg === "--run-id" || arg.startsWith("--run-id=")) {
      options.runId = readValue("--run-id");
    } else if (arg === "--run-root" || arg.startsWith("--run-root=")) {
      options.runRoot = readValue("--run-root");
    } else if (arg === "--sweep-branch" || arg.startsWith("--sweep-branch=")) {
      options.sweepBranch = readValue("--sweep-branch");
    } else if (arg === "--sweep-worktree" || arg.startsWith("--sweep-worktree=")) {
      options.sweepWorktree = readValue("--sweep-worktree");
    } else if (arg === "--adopt-legacy") {
      options.adoptLegacy = true;
    } else {
      throw new Error(`unknown option: ${arg}`);
    }
  }

  if (!["markdown", "json"].includes(options.format)) {
    throw new Error("--format must be markdown or json");
  }
  const modeCount = [options.dryRun && !options.full, options.classify, options.generateDocs, options.full].filter(Boolean).length;
  if (!options.help && modeCount !== 1) {
    throw new Error("choose exactly one mode: --dry-run, --classify, --generate-docs, or --full");
  }
  if (
    !options.help &&
    options.noCodex &&
    (options.classify || options.generateDocs || (options.full && !options.dryRun)) &&
    !options.fixture
  ) {
    throw new Error("--no-codex requires --fixture in classify, generate-docs, or mutating full mode");
  }
  if (!options.help && options.fixture && !options.noCodex) {
    throw new Error("--fixture is only valid with --no-codex");
  }
  if (
    !options.help &&
    (options.classify || options.generateDocs || (options.full && !options.dryRun)) &&
    !options.noCodex &&
    !path.basename(options.codexCommand).includes("codex")
  ) {
    throw new Error("--codex-command must point to a Codex CLI command");
  }
  if (!options.help && options.full && !options.sweepBranch.startsWith("zvorygin/")) {
    throw new Error("--sweep-branch must start with zvorygin/");
  }

  options.repoRoot = path.resolve(options.repoRoot);
  return options;
}

function parsePositiveInteger(value, name) {
  if (!/^\d+$/.test(value)) {
    throw new Error(`${name} must be a positive integer`);
  }
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}

function git(repoRoot, args, options = {}) {
  const output = execFileSync("git", ["-C", repoRoot, ...args], {
    encoding: "utf8",
    stdio: options.stdio ?? ["ignore", "pipe", "pipe"],
  });
  return typeof output === "string" ? output.trimEnd() : "";
}

function resolveCommit(repoRoot, ref, label) {
  try {
    return git(repoRoot, ["rev-parse", "--verify", `${ref}^{commit}`]).trim();
  } catch (error) {
    throw new Error(`could not resolve ${label} ${ref}: ${error.stderr?.toString().trim() || error.message}`);
  }
}

function readCheckpointFile(repoRoot, checkpointFile) {
  const absPath = path.resolve(repoRoot, checkpointFile);
  if (!existsSync(absPath)) {
    throw new Error(`checkpoint file not found: ${repoRelative(repoRoot, absPath)}`);
  }
  const checkpoint = readFileSync(absPath, "utf8")
    .split("\n")
    .map((line) => line.trim())
    .find((line) => line !== "" && !line.startsWith("#"));
  if (!checkpoint) {
    throw new Error(`checkpoint file has no checkpoint value: ${repoRelative(repoRoot, absPath)}`);
  }
  return {
    source: repoRelative(repoRoot, absPath),
    value: checkpoint,
  };
}

function readCheckpoint(repoRoot, checkpointFile, checkpointSeedFile) {
  const absCheckpoint = path.resolve(repoRoot, checkpointFile);
  if (existsSync(absCheckpoint)) {
    return readCheckpointFile(repoRoot, checkpointFile);
  }
  if (checkpointSeedFile) {
    return readCheckpointFile(repoRoot, checkpointSeedFile);
  }
  return readCheckpointFile(repoRoot, checkpointFile);
}

function writeCheckpointAtomic(repoRoot, checkpointFile, sha) {
  const absPath = path.resolve(repoRoot, checkpointFile);
  mkdirSync(path.dirname(absPath), { recursive: true });
  const tempPath = `${absPath}.tmp-${process.pid}`;
  writeFileSync(
    tempPath,
    [
      "# Documentation drift sweeper local checkpoint.",
      "# Updated only after a full sweep processes commits safely.",
      sha,
      "",
    ].join("\n"),
  );
  renameSync(tempPath, absPath);
  return repoRelative(repoRoot, absPath);
}

function repoRelative(repoRoot, pathname) {
  return path.relative(repoRoot, pathname).split(path.sep).join("/");
}

function normalizePath(pathname) {
  return pathname.split(path.sep).join("/");
}

function escapeRegExp(text) {
  return text.replace(/[|\\{}()[\]^$+?.]/g, "\\$&");
}

function globToRegExp(pattern) {
  let source = "";
  for (let index = 0; index < pattern.length; index += 1) {
    const char = pattern[index];
    const next = pattern[index + 1];
    if (char === "*" && next === "*") {
      source += ".*";
      index += 1;
    } else if (char === "*") {
      source += "[^/]*";
    } else {
      source += escapeRegExp(char);
    }
  }
  return new RegExp(`^${source}$`);
}

function pathMatchesPattern(pathname, pattern) {
  const normalizedPath = normalizePath(pathname);
  const normalizedPattern = normalizePath(pattern);
  if (!normalizedPattern.includes("*")) {
    return normalizedPath === normalizedPattern;
  }
  return globToRegExp(normalizedPattern).test(normalizedPath);
}

function loadTraceMap(repoRoot, traceMapPath) {
  const absPath = path.resolve(repoRoot, traceMapPath);
  if (!existsSync(absPath)) {
    throw new Error(`trace map not found: ${repoRelative(repoRoot, absPath)}`);
  }
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(absPath, "utf8"));
  } catch (error) {
    throw new Error(`trace map does not parse: ${error.message}`);
  }
  if (!Array.isArray(parsed.routes)) {
    throw new Error("trace map routes must be an array");
  }
  return {
    path: repoRelative(repoRoot, absPath),
    version: parsed.version ?? null,
    routes: parsed.routes,
  };
}

function changedPathsForCommit(repoRoot, sha) {
  const output = git(repoRoot, ["diff-tree", "--no-commit-id", "--name-only", "-r", "--root", sha]);
  return output.split("\n").map((line) => line.trim()).filter(Boolean);
}

function diffStatForCommit(repoRoot, sha) {
  const output = git(repoRoot, ["diff-tree", "--no-commit-id", "--shortstat", "-r", "--root", sha]);
  return output.trim() || "0 files changed";
}

function commitBody(repoRoot, sha) {
  return git(repoRoot, ["show", "-s", "--format=%b", sha]).trim();
}

function docsTouched(paths) {
  const design = [];
  const context = [];
  for (const pathname of paths) {
    if (pathname.startsWith("docs/design/")) {
      design.push(pathname);
    } else if (pathname.startsWith("docs/context/")) {
      context.push(pathname);
    }
  }
  return {
    design,
    context,
    anyDesign: design.length > 0,
    anyContext: context.length > 0,
  };
}

function isDocsOnlyChurn(paths) {
  if (paths.length === 0) {
    return false;
  }
  return paths.every(
    (pathname) =>
      pathname.startsWith("docs/") ||
      pathname.startsWith("plans/") ||
      pathname === "README.md" ||
      pathname.endsWith(".md"),
  );
}

function traceCandidates(traceMap, paths) {
  const docs = new Set();
  const routes = [];

  traceMap.routes.forEach((route, index) => {
    const sources = Array.isArray(route.source) ? route.source : [];
    const routeDocs = Array.isArray(route.docs) ? route.docs : [];
    const matched = [];
    for (const pathname of paths) {
      const matchedSources = sources.filter((source) => pathMatchesPattern(pathname, source));
      if (matchedSources.length > 0) {
        matched.push({ path: pathname, sources: matchedSources });
      }
    }
    if (matched.length > 0) {
      for (const doc of routeDocs) {
        docs.add(doc);
      }
      routes.push({
        routeIndex: index + 1,
        docs: routeDocs,
        matched,
        notes: typeof route.notes === "string" ? route.notes : "",
      });
    }
  });

  return {
    docs: [...docs].sort(),
    routes,
  };
}

function collectCommit(repoRoot, traceMap, sha) {
  const parents = git(repoRoot, ["show", "-s", "--format=%P", sha]).split(/\s+/).filter(Boolean);
  const paths = changedPathsForCommit(repoRoot, sha);
  const touched = docsTouched(paths);
  const trace = traceCandidates(traceMap, paths);
  let status = "considered";
  let skipReason = null;
  if (parents.length > 1) {
    status = "skipped";
    skipReason = "merge_commit";
  } else if (paths.length === 0) {
    status = "skipped";
    skipReason = "empty_commit";
  } else if (isDocsOnlyChurn(paths)) {
    status = "skipped";
    skipReason = "docs_only_churn";
  }

  return {
    sha,
    shortSha: sha.slice(0, 8),
    subject: git(repoRoot, ["show", "-s", "--format=%s", sha]),
    body: commitBody(repoRoot, sha),
    authorDate: git(repoRoot, ["show", "-s", "--format=%aI", sha]),
    parentCount: parents.length,
    status,
    skipReason,
    changedPaths: paths,
    diffStat: diffStatForCommit(repoRoot, sha),
    docsTouched: touched,
    traceDocs: trace.docs,
    traceRoutes: trace.routes,
  };
}

function stableJson(value) {
  if (Array.isArray(value)) {
    return value.map((item) => stableJson(item));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, stableJson(value[key])]));
  }
  return value;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function estimatePromptTokens(text) {
  return Math.ceil(text.length / 4);
}

function classifierPromptInput(commit, priorCachedDecision) {
  return {
    promptVersion: classifierPromptVersion,
    allowedDecisions: ["move_on", "update_docs"],
    instructions:
      "Classify whether this behavior-changing commit should move on or should be sent to a documentation patch generator. Use only the metadata provided. Return one JSON object.",
    commit: {
      sha: commit.sha,
      subject: commit.subject,
      body: commit.body,
      changedPaths: commit.changedPaths,
      diffStat: commit.diffStat,
      docsTouched: commit.docsTouched,
      traceDocs: commit.traceDocs,
      traceRoutes: commit.traceRoutes.map((route) => ({
        routeIndex: route.routeIndex,
        docs: route.docs,
        matched: route.matched,
        notes: route.notes,
      })),
    },
    priorCachedDecision,
  };
}

function renderClassifierPrompt(input) {
  return [
    "You are the cheap classifier for the RTS documentation drift sweeper.",
    "",
    "Decide if the commit should be ignored for docs drift or sent to the later doc patch generator.",
    "Use best judgment; do not create a manual-review state.",
    "Do not ask for diffs or browse files. Use only the supplied metadata.",
    "",
    "Return only JSON with this shape:",
    '{"decision":"move_on|update_docs","likelyDocs":["docs/design/example.md"],"evidenceNote":"short factual reason"}',
    "",
    "Rules:",
    "- decision must be exactly move_on or update_docs.",
    "- likelyDocs must be strings from traceDocs/docsTouched when available, or an empty array.",
    "- evidenceNote must be one concise sentence grounded in the commit metadata.",
    "",
    "Input JSON:",
    JSON.stringify(input, null, 2),
  ].join("\n");
}

function sanitizeDecision(rawDecision, commit) {
  if (!rawDecision || typeof rawDecision !== "object" || Array.isArray(rawDecision)) {
    throw new Error(`classifier for ${commit.shortSha} did not return a JSON object`);
  }
  if (!validClassifierDecisions.has(rawDecision.decision)) {
    throw new Error(`classifier for ${commit.shortSha} returned invalid decision: ${rawDecision.decision}`);
  }
  if (!Array.isArray(rawDecision.likelyDocs)) {
    throw new Error(`classifier for ${commit.shortSha} returned non-array likelyDocs`);
  }
  const likelyDocs = [...new Set(rawDecision.likelyDocs.filter((doc) => typeof doc === "string" && doc.trim()))]
    .map((doc) => doc.trim())
    .sort();
  const evidenceNote =
    typeof rawDecision.evidenceNote === "string" && rawDecision.evidenceNote.trim()
      ? rawDecision.evidenceNote.trim()
      : "Classifier did not provide an evidence note.";
  return {
    decision: rawDecision.decision,
    likelyDocs,
    evidenceNote,
  };
}

function parseJsonObject(text, label) {
  const trimmed = text.trim();
  try {
    return JSON.parse(trimmed);
  } catch {
    const first = trimmed.indexOf("{");
    const last = trimmed.lastIndexOf("}");
    if (first >= 0 && last > first) {
      return JSON.parse(trimmed.slice(first, last + 1));
    }
    throw new Error(`${label} was not parseable JSON`);
  }
}

function classifierCachePath(repoRoot, cacheDir, commitSha) {
  return path.resolve(repoRoot, cacheDir, classifierPromptVersion, `${commitSha}.json`);
}

function readCachedClassifierRecord(cachePath) {
  if (!existsSync(cachePath)) {
    return null;
  }
  return JSON.parse(readFileSync(cachePath, "utf8"));
}

function writeCachedClassifierRecord(cachePath, record) {
  mkdirSync(path.dirname(cachePath), { recursive: true });
  writeFileSync(cachePath, `${JSON.stringify(record, null, 2)}\n`);
}

function loadClassifierFixture(repoRoot, fixtureNameOrPath) {
  const candidates = [];
  if (fixtureNameOrPath.includes("/") || fixtureNameOrPath.endsWith(".json")) {
    candidates.push(path.resolve(repoRoot, fixtureNameOrPath));
  } else {
    candidates.push(path.resolve(defaultRepoRoot, "tests", "fixtures", "docdrift", `${fixtureNameOrPath}.json`));
    candidates.push(path.resolve(repoRoot, "tests", "fixtures", "docdrift", `${fixtureNameOrPath}.json`));
  }
  const fixturePath = candidates.find((candidate) => existsSync(candidate));
  if (!fixturePath) {
    throw new Error(`classifier fixture not found: ${fixtureNameOrPath}`);
  }
  const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
  if (!Array.isArray(fixture.decisions)) {
    throw new Error(`classifier fixture ${fixtureNameOrPath} must include decisions[]`);
  }
  const pathLabel = fixturePath.startsWith(`${defaultRepoRoot}${path.sep}`)
    ? repoRelative(defaultRepoRoot, fixturePath)
    : repoRelative(repoRoot, fixturePath);
  return {
    path: pathLabel,
    fixture,
  };
}

function fixtureDecisionForCommit(fixture, commit) {
  const matched = fixture.decisions.find((entry) => {
    if (entry.sha && entry.sha !== commit.sha) {
      return false;
    }
    if (entry.subjectIncludes && !commit.subject.includes(entry.subjectIncludes)) {
      return false;
    }
    if (entry.pathIncludes && !commit.changedPaths.some((pathname) => pathname.includes(entry.pathIncludes))) {
      return false;
    }
    return Boolean(entry.sha || entry.subjectIncludes || entry.pathIncludes);
  });
  const fallback = fixture.default ?? {
    decision: "move_on",
    likelyDocs: [],
    evidenceNote: "Fixture default moved this commit on.",
  };
  return matched ?? fallback;
}

function codexInvocationArgs(options, outputPath) {
  const args = ["exec", "--sandbox", "read-only", "-c", 'approval_policy="never"', "--ephemeral", "--json"];
  if (options.codexModel) {
    args.push("--model", options.codexModel);
  }
  args.push(...options.codexArgs, "--output-last-message", outputPath, "-");
  return args;
}

function numericValue(value) {
  return Number.isFinite(value) ? value : null;
}

function normalizeUsageObject(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const inputTokens = numericValue(value.input_tokens ?? value.inputTokens ?? value.prompt_tokens ?? value.promptTokens);
  const cachedInputTokens = numericValue(value.cached_input_tokens ?? value.cachedInputTokens);
  const outputTokens = numericValue(value.output_tokens ?? value.outputTokens ?? value.completion_tokens ?? value.completionTokens);
  const reasoningTokens = numericValue(value.reasoning_tokens ?? value.reasoningTokens);
  const totalTokens = numericValue(
    value.total_tokens ??
      value.totalTokens ??
      (inputTokens !== null || outputTokens !== null || reasoningTokens !== null
        ? (inputTokens ?? 0) + (outputTokens ?? 0) + (reasoningTokens ?? 0)
        : null),
  );
  if (inputTokens === null && cachedInputTokens === null && outputTokens === null && reasoningTokens === null && totalTokens === null) {
    return null;
  }
  return {
    inputTokens,
    cachedInputTokens,
    outputTokens,
    reasoningTokens,
    totalTokens,
  };
}

function findUsageObject(value) {
  const direct = normalizeUsageObject(value);
  if (direct) {
    return direct;
  }
  if (!value || typeof value !== "object") {
    return null;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      const usage = findUsageObject(item);
      if (usage) {
        return usage;
      }
    }
    return null;
  }
  for (const key of ["usage", "token_usage", "tokenUsage"]) {
    const usage = findUsageObject(value[key]);
    if (usage) {
      return usage;
    }
  }
  return null;
}

function codexUsageFromStdout(stdout) {
  let latest = null;
  for (const line of String(stdout ?? "").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("{")) {
      continue;
    }
    try {
      const usage = findUsageObject(JSON.parse(trimmed));
      if (usage) {
        latest = usage;
      }
    } catch {
      // Ignore non-event output; --output-last-message remains the source of the model answer.
    }
  }
  return latest;
}

function codexExecOptions(options, prompt) {
  return { cwd: options.repoRoot, encoding: "utf8", input: prompt, maxBuffer: 1024 * 1024 * 5, stdio: ["pipe", "pipe", "pipe"], timeout: options.codexTimeoutSeconds * 1000 };
}

function codexFailureMessage(error, label, timeoutSeconds) {
  const stderr = error.stderr?.toString().trim();
  return error.code === "ETIMEDOUT" || error.signal === "SIGTERM"
    ? `${label} timed out after ${timeoutSeconds}s`
    : `${label} failed: ${stderr || error.message}`;
}

function classifyWithCodex(options, prompt) {
  const tempDir = mkdtempSync(path.join(os.tmpdir(), "rts-docdrift-codex-"));
  const outputPath = path.join(tempDir, "last-message.txt");
  const args = codexInvocationArgs(options, outputPath);
  try {
    const stdout = execFileSync(options.codexCommand, args, codexExecOptions(options, prompt));
    if (!existsSync(outputPath)) {
      throw new Error("Codex CLI did not write --output-last-message");
    }
    return {
      rawText: readFileSync(outputPath, "utf8"),
      invocation: {
        command: options.codexCommand,
        args,
        mode: "codex_cli",
        promptVersion: classifierPromptVersion,
        usage: codexUsageFromStdout(stdout),
      },
    };
  } catch (error) {
    throw new Error(codexFailureMessage(error, "Codex classifier", options.codexTimeoutSeconds));
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function buildClassifierRecord({ commit, decision, promptHash, promptTokens, cachePath, cacheHit, invocation }) {
  return {
    promptVersion: classifierPromptVersion,
    commitSha: commit.sha,
    shortSha: commit.shortSha,
    subject: commit.subject,
    decision: decision.decision,
    likelyDocs: decision.likelyDocs,
    evidenceNote: decision.evidenceNote,
    cache: {
      hit: cacheHit,
      path: cachePath,
      promptHash,
    },
    prompt: {
      estimatedTokens: promptTokens,
      inputFields: ["subject", "body", "changedPaths", "diffStat", "docsTouched", "traceDocs", "traceRoutes"],
    },
    codex: invocation,
  };
}

function enforceClassifierBudgets(commits, prompts, options) {
  if (commits.length > options.maxCommits) {
    throw new Error(
      `classify budget exceeded: ${commits.length} considered commits exceeds --max-commits ${options.maxCommits}`,
    );
  }
  const uncachedPrompts = prompts.filter((entry) => !entry.cacheHit);
  const tooLarge = uncachedPrompts.find((entry) => entry.promptTokens > options.maxPromptTokens);
  if (tooLarge) {
    throw new Error(
      `classify budget exceeded: ${tooLarge.commit.shortSha} prompt estimate ${tooLarge.promptTokens} tokens exceeds --max-prompt-tokens ${options.maxPromptTokens}`,
    );
  }
  const totalTokens = uncachedPrompts.reduce((sum, entry) => sum + entry.promptTokens, 0);
  if (totalTokens > options.maxTotalPromptTokens) {
    throw new Error(
      `classify budget exceeded: prompt estimate ${totalTokens} tokens exceeds --max-total-prompt-tokens ${options.maxTotalPromptTokens}`,
    );
  }
}

export function classifyReport(report, options) {
  const considered = report.commits.filter((commit) => commit.status === "considered");
  const cacheRoot = path.resolve(options.repoRoot, options.classifierCacheDir);
  const promptEntries = considered.map((commit) => {
    const cachePath = classifierCachePath(options.repoRoot, options.classifierCacheDir, commit.sha);
    const cached = readCachedClassifierRecord(cachePath);
    const cacheInput = classifierPromptInput(commit, null);
    const promptHash = sha256(JSON.stringify(stableJson(cacheInput)));
    const priorCachedDecision = cached
      ? {
          decision: cached.decision,
          likelyDocs: cached.likelyDocs,
          evidenceNote: cached.evidenceNote,
          promptHash: cached.cache?.promptHash,
        }
      : null;
    const cacheHit = cached?.cache?.promptHash === promptHash;
    const promptInput = cacheHit ? cacheInput : classifierPromptInput(commit, priorCachedDecision);
    const prompt = renderClassifierPrompt(promptInput);
    return {
      commit,
      cachePath,
      cached,
      cacheHit,
      prompt,
      promptHash,
      promptTokens: estimatePromptTokens(prompt),
    };
  });
  enforceClassifierBudgets(considered, promptEntries, options);

  const loadedFixture = options.noCodex ? loadClassifierFixture(options.repoRoot, options.fixture) : null;
  const decisions = [];
  for (const [index, entry] of promptEntries.entries()) {
    const relativeCachePath = repoRelative(options.repoRoot, entry.cachePath);
    if (entry.cached?.cache?.promptHash === entry.promptHash) {
      if (!options.noCodex) {
        console.error(`docdrift: classify ${index + 1}/${promptEntries.length} ${entry.commit.shortSha} cache=hit`);
      }
      decisions.push({
        ...entry.cached,
        cache: {
          ...entry.cached.cache,
          hit: true,
          path: relativeCachePath,
        },
      });
      continue;
    }

    let rawDecision;
    let invocation;
    if (options.noCodex) {
      rawDecision = fixtureDecisionForCommit(loadedFixture.fixture, entry.commit);
      invocation = {
        command: null,
        args: [],
        mode: "fixture",
        fixture: loadedFixture.path,
        promptVersion: classifierPromptVersion,
      };
    } else {
      console.error(
        `docdrift: classify ${index + 1}/${promptEntries.length} ${entry.commit.shortSha} cache=miss prompt_estimate=${entry.promptTokens}`,
      );
      const codexResult = classifyWithCodex(options, entry.prompt);
      rawDecision = parseJsonObject(codexResult.rawText, `Codex output for ${entry.commit.shortSha}`);
      invocation = codexResult.invocation;
    }
    const decision = sanitizeDecision(rawDecision, entry.commit);
    const record = buildClassifierRecord({
      commit: entry.commit,
      decision,
      promptHash: entry.promptHash,
      promptTokens: entry.promptTokens,
      cachePath: relativeCachePath,
      cacheHit: false,
      invocation,
    });
    writeCachedClassifierRecord(entry.cachePath, record);
    decisions.push(record);
  }

  const moveOn = decisions.filter((decision) => decision.decision === "move_on").length;
  const updateDocs = decisions.filter((decision) => decision.decision === "update_docs").length;
  return {
    ...report,
    mode: "classify",
    classifier: {
      promptVersion: classifierPromptVersion,
      cacheDir: repoRelative(options.repoRoot, cacheRoot),
      noCodex: options.noCodex,
      fixture: loadedFixture?.path ?? null,
      budget: {
        maxCommits: options.maxCommits,
        maxPromptTokens: options.maxPromptTokens,
        maxTotalPromptTokens: options.maxTotalPromptTokens,
        estimatedPromptTokens: promptEntries
          .filter((entry) => !entry.cacheHit)
          .reduce((sum, entry) => sum + entry.promptTokens, 0),
      },
      decisions,
      summary: {
        totalDecisions: decisions.length,
        moveOn,
        updateDocs,
        cacheHits: decisions.filter((decision) => decision.cache.hit).length,
      },
    },
  };
}

function designDocTargetsForDecision(decision, commit) {
  const designDocs = (docs) => [...new Set((docs ?? []).filter((doc) => doc.startsWith("docs/design/")))].sort();
  const likelyDesignDocs = designDocs(decision.likelyDocs);
  if (likelyDesignDocs.length > 0) {
    return likelyDesignDocs;
  }
  const touchedDesignDocs = designDocs(commit.docsTouched?.design);
  if (touchedDesignDocs.length > 0) {
    return touchedDesignDocs;
  }
  const traceDesignDocs = designDocs(commit.traceDocs);
  return traceDesignDocs;
}

function designDocTargetSource(decision, commit) {
  if ((decision.likelyDocs ?? []).some((doc) => doc.startsWith("docs/design/"))) {
    return "classifier_likely_docs";
  }
  if ((commit.docsTouched?.design ?? []).length > 0) {
    return "docs_touched";
  }
  return "trace_map";
}

function allCachedPatchReplacementsPresent(repoRoot, rawPatch, commit) {
  const patchResult = sanitizeDocPatchResult(rawPatch, commit);
  return patchResult.patches.every((patch) => {
    const absPath = path.resolve(repoRoot, patch.path);
    return existsSync(absPath) && readFileSync(absPath, "utf8").includes(patch.replace);
  });
}

function commitKeywords(commit, decision) {
  const words = new Set();
  const addWords = (text) => {
    for (const word of String(text ?? "").toLowerCase().match(/[a-z][a-z0-9_-]{3,}/g) ?? []) {
      words.add(word);
    }
  };
  addWords(commit.subject);
  addWords(commit.body);
  addWords(decision.evidenceNote);
  for (const pathname of commit.changedPaths) {
    addWords(path.basename(pathname, path.extname(pathname)));
    addWords(path.dirname(pathname).split("/").slice(-2).join(" "));
  }
  return words;
}

function markdownSections(text) {
  const lines = text.split("\n");
  const sections = [];
  let current = {
    heading: "(preamble)",
    level: 0,
    startLine: 1,
    lines: [],
  };
  const flush = (endLine) => {
    if (current.lines.length > 0) {
      sections.push({
        heading: current.heading,
        level: current.level,
        startLine: current.startLine,
        endLine,
        text: current.lines.join("\n"),
      });
    }
  };
  lines.forEach((line, index) => {
    const heading = /^(#{1,6})\s+(.+?)\s*$/.exec(line);
    if (heading) {
      flush(index);
      current = {
        heading: heading[2],
        level: heading[1].length,
        startLine: index + 1,
        lines: [line],
      };
    } else {
      current.lines.push(line);
    }
  });
  flush(lines.length);
  return sections;
}

function selectDocSections(repoRoot, docPath, keywords) {
  const absPath = path.resolve(repoRoot, docPath);
  if (!existsSync(absPath)) {
    return {
      path: docPath,
      missing: true,
      sections: [],
    };
  }
  const text = readFileSync(absPath, "utf8");
  const sections = markdownSections(text);
  const scored = sections
    .map((section, index) => {
      const haystack = `${section.heading}\n${section.text}`.toLowerCase();
      let score = 0;
      for (const keyword of keywords) {
        if (haystack.includes(keyword)) {
          score += 1;
        }
      }
      if (section.level === 1 || index === 0) {
        score += 0.25;
      }
      return { section, score, index };
    })
    .sort((a, b) => b.score - a.score || a.index - b.index);
  const matched = scored.filter((entry) => entry.score > 0);
  const selectedSource = matched.length > 0 ? matched : scored.slice(0, 1);
  const selected = selectedSource
    .slice(0, maxDocSectionsPerTarget)
    .sort((a, b) => a.index - b.index)
    .map((entry) => ({
      heading: entry.section.heading,
      level: entry.section.level,
      startLine: entry.section.startLine,
      endLine: entry.section.endLine,
      text: entry.section.text.slice(0, maxDocSectionChars),
    }));
  return {
    path: docPath,
    missing: false,
    sectionCount: sections.length,
    sections: selected,
  };
}

function docPatchPromptInput({ commit, decision, docTargets, docSections, priorCachedPatch }) {
  return {
    promptVersion: docPatchPromptVersion,
    instructions:
      "Generate factual, minimal documentation patches for authoritative RTS design docs. Return one JSON object with exact find/replace edits only.",
    rules: [
      "Use only the supplied commit metadata, classifier evidence, trace-map routing, and doc sections.",
      "Patch docs/design/*.md first. Patch docs/context/*.md only if section structure or entry points change.",
      "Do not add speculative strategy claims. Describe concrete behavior only.",
      "If the supplied docs already cover the requested behavior, return an empty patches array instead of restating it.",
      "Do not use OpenAI Agents SDK, direct OpenAI API clients, API keys, or API-billed fallback routes.",
      "Return exact find/replace edits that can be applied idempotently.",
    ],
    commit: {
      sha: commit.sha,
      subject: commit.subject,
      body: commit.body,
      changedPaths: commit.changedPaths,
      diffStat: commit.diffStat,
      traceDocs: commit.traceDocs,
      traceRoutes: commit.traceRoutes,
    },
    decision: {
      decision: decision.decision,
      likelyDocs: decision.likelyDocs,
      evidenceNote: decision.evidenceNote,
    },
    docTargets,
    docSections,
    priorCachedPatch,
  };
}

function renderDocPatchPrompt(input) {
  return [
    "You are the documentation patch generator for the RTS documentation drift sweeper.",
    "",
    "Create only minimal, factual edits for stale authoritative design docs.",
    "Use exact text from the supplied doc sections for every find value.",
    "If no safe minimal patch is justified, return an empty patches array.",
    "",
    "Return only JSON with this shape:",
    '{"summary":"short summary","patches":[{"path":"docs/design/example.md","find":"exact old text","replace":"exact new text","rationale":"short factual reason"}]}',
    "",
    "Input JSON:",
    JSON.stringify(input, null, 2),
  ].join("\n");
}

function loadDocPatchFixture(repoRoot, fixtureNameOrPath) {
  const loaded = loadClassifierFixture(repoRoot, fixtureNameOrPath);
  if (!Array.isArray(loaded.fixture.docPatches)) {
    throw new Error(`doc patch fixture ${fixtureNameOrPath} must include docPatches[]`);
  }
  return loaded;
}

function fixtureDocPatchForCommit(fixture, commit) {
  const matched = fixture.docPatches.find((entry) => {
    if (entry.sha && entry.sha !== commit.sha) {
      return false;
    }
    if (entry.subjectIncludes && !commit.subject.includes(entry.subjectIncludes)) {
      return false;
    }
    if (entry.pathIncludes && !commit.changedPaths.some((pathname) => pathname.includes(entry.pathIncludes))) {
      return false;
    }
    return Boolean(entry.sha || entry.subjectIncludes || entry.pathIncludes);
  });
  return matched ?? fixture.defaultDocPatch ?? { summary: "Fixture returned no doc patch.", patches: [] };
}

function sanitizeDocPatchResult(rawPatch, commit) {
  if (!rawPatch || typeof rawPatch !== "object" || Array.isArray(rawPatch)) {
    throw new Error(`doc patch for ${commit.shortSha} did not return a JSON object`);
  }
  if (!Array.isArray(rawPatch.patches)) {
    throw new Error(`doc patch for ${commit.shortSha} returned non-array patches`);
  }
  const patches = rawPatch.patches.map((patch, index) => {
    if (!patch || typeof patch !== "object" || Array.isArray(patch)) {
      throw new Error(`doc patch ${index + 1} for ${commit.shortSha} is not an object`);
    }
    const pathname = typeof patch.path === "string" ? normalizePath(patch.path.trim()) : "";
    if (!validDocPatchPrefixes.some((prefix) => pathname.startsWith(prefix))) {
      throw new Error(`doc patch ${index + 1} for ${commit.shortSha} targets invalid path: ${pathname || "(empty)"}`);
    }
    const find = typeof patch.find === "string" ? patch.find : "";
    const replace = typeof patch.replace === "string" ? patch.replace : "";
    if (!find || !replace || find === replace) {
      throw new Error(`doc patch ${index + 1} for ${commit.shortSha} must include distinct find and replace text`);
    }
    const rationale =
      typeof patch.rationale === "string" && patch.rationale.trim()
        ? patch.rationale.trim()
        : "Doc patch did not provide a rationale.";
    return { path: pathname, find, replace, rationale };
  });
  return {
    summary:
      typeof rawPatch.summary === "string" && rawPatch.summary.trim()
        ? rawPatch.summary.trim()
        : "Generated documentation patch.",
    patches,
  };
}

function docPatchCachePath(repoRoot, cacheDir, commitSha) {
  return path.resolve(repoRoot, cacheDir, docPatchPromptVersion, `${commitSha}.json`);
}

function countOccurrences(text, needle) {
  let count = 0;
  let offset = 0;
  while (needle && offset < text.length) {
    const next = text.indexOf(needle, offset);
    if (next === -1) {
      break;
    }
    count += 1;
    offset = next + needle.length;
  }
  return count;
}

class DocPatchApplyError extends Error {
  constructor(message, details = {}) {
    super(message);
    this.name = "DocPatchApplyError";
    this.details = details;
  }
}

function applyDocPatches(repoRoot, patches) {
  const results = [];
  const stagedFiles = new Map();
  for (const patch of patches) {
    const absPath = path.resolve(repoRoot, patch.path);
    if (!existsSync(absPath)) {
      throw new DocPatchApplyError(`doc patch target not found: ${patch.path}`, { patch, applications: results });
    }
    const current = stagedFiles.has(patch.path) ? stagedFiles.get(patch.path) : readFileSync(absPath, "utf8");
    if (current.includes(patch.replace)) {
      results.push({ path: patch.path, status: "already_applied", rationale: patch.rationale });
      continue;
    }
    const findCount = countOccurrences(current, patch.find);
    if (findCount === 1) {
      stagedFiles.set(patch.path, current.replace(patch.find, patch.replace));
      results.push({ path: patch.path, status: "applied", rationale: patch.rationale });
      continue;
    }
    if (findCount === 0) {
      throw new DocPatchApplyError(`doc patch find text not found in ${patch.path}`, { patch, applications: results });
    }
    throw new DocPatchApplyError(`doc patch find text matched ${findCount} times in ${patch.path}`, {
      patch,
      applications: results,
    });
  }
  for (const [pathname, text] of stagedFiles) {
    writeFileSync(path.resolve(repoRoot, pathname), text);
  }
  return results;
}

function docPatchSkip({ error, decision, commit, index, total, patchRecords }) {
  return {
    index: index + 1,
    total,
    commitSha: commit.sha,
    shortSha: commit.shortSha,
    subject: commit.subject,
    kind: error instanceof DocPatchApplyError ? "apply_error" : "generation_error",
    message: error.message,
    appliedRecords: patchRecords.length,
    priorAppliedPatches: patchRecords
      .flatMap((record) => record.applications)
      .filter((application) => application.status === "applied").length,
    decision: {
      likelyDocs: decision.likelyDocs ?? [],
      evidenceNote: decision.evidenceNote ?? "",
    },
  };
}

function generateDocPatchWithCodex(options, prompt) {
  const tempDir = mkdtempSync(path.join(os.tmpdir(), "rts-docdrift-doc-patch-"));
  const outputPath = path.join(tempDir, "last-message.txt");
  const args = codexInvocationArgs(options, outputPath);
  try {
    const stdout = execFileSync(options.codexCommand, args, codexExecOptions(options, prompt));
    if (!existsSync(outputPath)) {
      throw new Error("Codex CLI did not write --output-last-message");
    }
    return {
      rawText: readFileSync(outputPath, "utf8"),
      invocation: {
        command: options.codexCommand,
        args,
        mode: "codex_cli",
        promptVersion: docPatchPromptVersion,
        usage: codexUsageFromStdout(stdout),
      },
    };
  } catch (error) {
    throw new Error(codexFailureMessage(error, "Codex doc patch generator", options.codexTimeoutSeconds));
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function buildDocPatchRecord({
  commit,
  decision,
  docTargets,
  docTargetSource,
  docSections,
  patchResult,
  promptHash,
  promptTokens,
  cachePath,
  cacheHit,
  cacheReason,
  invocation,
  applications,
}) {
  return {
    promptVersion: docPatchPromptVersion,
    commitSha: commit.sha,
    shortSha: commit.shortSha,
    subject: commit.subject,
    decision: {
      likelyDocs: decision.likelyDocs,
      evidenceNote: decision.evidenceNote,
    },
    docTargets,
    docTargetSource,
    docSections: docSections.map((doc) => ({
      path: doc.path,
      missing: doc.missing,
      selectedSections: doc.sections.map((section) => ({
        heading: section.heading,
        startLine: section.startLine,
        endLine: section.endLine,
      })),
    })),
    summary: patchResult.summary,
    patches: patchResult.patches.map((patch) => ({
      path: patch.path,
      rationale: patch.rationale,
      findHash: sha256(patch.find),
      replaceHash: sha256(patch.replace),
    })),
    applications,
    cache: {
      hit: cacheHit,
      reason: cacheReason,
      path: cachePath,
      promptHash,
    },
    prompt: {
      estimatedTokens: promptTokens,
      inputFields: ["decision", "commit", "docTargets", "docSections", "repo documentation rules"],
    },
    codex: invocation,
  };
}

function enforceDocPatchBudgets(entries, options) {
  const uncached = entries.filter((entry) => !entry.cacheHit);
  const tooLarge = uncached.find((entry) => entry.promptTokens > options.maxDocPromptTokens);
  if (tooLarge) {
    throw new Error(
      `doc patch budget exceeded: ${tooLarge.commit.shortSha} prompt estimate ${tooLarge.promptTokens} tokens exceeds --max-doc-prompt-tokens ${options.maxDocPromptTokens}`,
    );
  }
  const totalTokens = uncached.reduce((sum, entry) => sum + entry.promptTokens, 0);
  if (totalTokens > options.maxTotalDocPromptTokens) {
    throw new Error(
      `doc patch budget exceeded: prompt estimate ${totalTokens} tokens exceeds --max-total-doc-prompt-tokens ${options.maxTotalDocPromptTokens}`,
    );
  }
}

function buildDocPatchEntry({ options, decision, commit }) {
  const docTargets = designDocTargetsForDecision(decision, commit);
  const docTargetSource = designDocTargetSource(decision, commit);
  const keywords = commitKeywords(commit, decision);
  const docSections = docTargets.map((docPath) => selectDocSections(options.repoRoot, docPath, keywords));
  const cachePath = docPatchCachePath(options.repoRoot, options.docPatchCacheDir, commit.sha);
  const cached = readCachedClassifierRecord(cachePath);
  const cacheInput = docPatchPromptInput({ commit, decision, docTargets, docSections, priorCachedPatch: null });
  const promptHash = sha256(JSON.stringify(stableJson(cacheInput)));
  const promptHashHit = cached?.cache?.promptHash === promptHash;
  const cachedPatchAlreadyApplied = cached?.rawPatch
    ? allCachedPatchReplacementsPresent(options.repoRoot, cached.rawPatch, commit)
    : false;
  const priorCachedPatch = cached
    ? {
        summary: cached.summary,
        patches: cached.patches,
        promptHash: cached.cache?.promptHash,
      }
    : null;
  const cacheHit = promptHashHit || cachedPatchAlreadyApplied;
  const promptInput = cacheHit
    ? cacheInput
    : docPatchPromptInput({ commit, decision, docTargets, docSections, priorCachedPatch });
  const prompt = renderDocPatchPrompt(promptInput);
  return {
    commit,
    decision,
    docTargets,
    docTargetSource,
    docSections,
    cachePath,
    cached,
    cacheHit,
    cacheReason: promptHashHit ? "prompt_hash" : cachedPatchAlreadyApplied ? "already_applied_patch" : null,
    prompt,
    promptHash,
    promptTokens: estimatePromptTokens(prompt),
  };
}

export function generateDocsReport(classifierReport, options) {
  const updateDecisions = classifierReport.classifier.decisions.filter((decision) => decision.decision === "update_docs");
  const commitBySha = new Map(classifierReport.commits.map((commit) => [commit.sha, commit]));
  const cacheRoot = path.resolve(options.repoRoot, options.docPatchCacheDir);

  const loadedFixture = options.noCodex ? loadDocPatchFixture(options.repoRoot, options.fixture) : null;
  const patchRecords = [];
  let estimatedPromptTokens = 0;
  const skipped = [];
  for (const [index, decision] of updateDecisions.entries()) {
    const commit = commitBySha.get(decision.commitSha);
    if (!commit) {
      const missingSha = typeof decision.commitSha === "string" ? decision.commitSha : "(missing)";
      const missingCommit = { sha: missingSha, shortSha: missingSha.slice(0, 8), subject: "(missing commit)" };
      const error = new Error(`classifier decision references missing commit: ${missingSha}`);
      skipped.push(docPatchSkip({ error, decision, commit: missingCommit, index, total: updateDecisions.length, patchRecords }));
      continue;
    }
    try {
      const entry = buildDocPatchEntry({ options, decision, commit });
      if (!entry.cacheHit) {
        enforceDocPatchBudgets([entry], options);
        if (estimatedPromptTokens + entry.promptTokens > options.maxTotalDocPromptTokens) {
          throw new Error(
            `doc patch budget exceeded: prompt estimate ${estimatedPromptTokens + entry.promptTokens} tokens exceeds --max-total-doc-prompt-tokens ${options.maxTotalDocPromptTokens}`,
          );
        }
        estimatedPromptTokens += entry.promptTokens;
      }
      const relativeCachePath = repoRelative(options.repoRoot, entry.cachePath);
      let patchResult;
      let invocation;
      if (entry.cacheHit && entry.cached?.rawPatch) {
        if (!options.noCodex) {
          console.error(
            `docdrift: generate-docs ${index + 1}/${updateDecisions.length} ${entry.commit.shortSha} cache=hit reason=${entry.cacheReason}`,
          );
        }
        patchResult = sanitizeDocPatchResult(entry.cached.rawPatch, entry.commit);
        invocation = {
          ...entry.cached.codex,
          cachedFrom: relativeCachePath,
        };
      } else if (options.noCodex) {
        patchResult = sanitizeDocPatchResult(fixtureDocPatchForCommit(loadedFixture.fixture, entry.commit), entry.commit);
        invocation = {
          command: null,
          args: [],
          mode: "fixture",
          fixture: loadedFixture.path,
          promptVersion: docPatchPromptVersion,
        };
      } else {
        console.error(
          `docdrift: generate-docs ${index + 1}/${updateDecisions.length} ${entry.commit.shortSha} cache=miss prompt_estimate=${entry.promptTokens} targets=${entry.docTargets.join(",") || "none"}`,
        );
        const codexResult = generateDocPatchWithCodex(options, entry.prompt);
        patchResult = sanitizeDocPatchResult(parseJsonObject(codexResult.rawText, `Codex doc patch output for ${entry.commit.shortSha}`), entry.commit);
        invocation = codexResult.invocation;
      }
      const applications = applyDocPatches(options.repoRoot, patchResult.patches);
      const record = buildDocPatchRecord({
        commit: entry.commit,
        decision: entry.decision,
        docTargets: entry.docTargets,
        docTargetSource: entry.docTargetSource,
        docSections: entry.docSections,
        patchResult,
        promptHash: entry.promptHash,
        promptTokens: entry.promptTokens,
        cachePath: relativeCachePath,
        cacheHit: entry.cacheHit,
        cacheReason: entry.cacheReason,
        invocation,
        applications,
      });
      const cacheRecord = {
        ...record,
        rawPatch: patchResult,
        cache: {
          ...record.cache,
          hit: false,
        },
      };
      writeCachedClassifierRecord(entry.cachePath, cacheRecord);
      patchRecords.push(record);
    } catch (error) {
      const skip = docPatchSkip({ error, decision, commit, index, total: updateDecisions.length, patchRecords });
      skipped.push(skip);
      if (!options.noCodex) {
        console.error(`docdrift: generate-docs ${index + 1}/${updateDecisions.length} ${commit.shortSha} skipped: ${error.message}`);
      }
    }
  }

  const applied = patchRecords.flatMap((record) => record.applications).filter((app) => app.status === "applied").length;
  const alreadyApplied = patchRecords
    .flatMap((record) => record.applications)
    .filter((app) => app.status === "already_applied").length;
  return {
    ...classifierReport,
    mode: "generate-docs",
    docPatch: {
      promptVersion: docPatchPromptVersion,
      cacheDir: repoRelative(options.repoRoot, cacheRoot),
      noCodex: options.noCodex,
      fixture: loadedFixture?.path ?? null,
      partial: false,
      failure: null,
      skipped,
      budget: {
        maxDocPromptTokens: options.maxDocPromptTokens,
        maxTotalDocPromptTokens: options.maxTotalDocPromptTokens,
        estimatedPromptTokens,
      },
      records: patchRecords,
      summary: {
        updateDocsDecisions: updateDecisions.length,
        patchRecords: patchRecords.length,
        patches: patchRecords.reduce((sum, record) => sum + record.patches.length, 0),
        applied,
        alreadyApplied,
        cacheHits: patchRecords.filter((record) => record.cache.hit).length,
        skipped: skipped.length,
        failed: false,
      },
    },
  };
}

export function buildReport(options) {
  const repoRoot = options.repoRoot;
  const traceMap = loadTraceMap(repoRoot, options.traceMap);
  let baseRef = options.base;
  let baseSource = "--base";
  if (!baseRef && options.checkpointRef) {
    baseRef = options.checkpointRef;
    baseSource = "--checkpoint-ref";
  } else if (!baseRef) {
    const checkpoint = readCheckpoint(repoRoot, options.checkpointFile, options.checkpointSeedFile);
    baseRef = checkpoint.value;
    baseSource = checkpoint.source;
  }
  const baseSha = resolveCommit(repoRoot, baseRef, baseSource);
  const headSha = resolveCommit(repoRoot, options.head, "--head");
  const revListOutput = git(repoRoot, ["rev-list", "--reverse", `${baseSha}..${headSha}`]);
  const shas = revListOutput.split("\n").map((line) => line.trim()).filter(Boolean);
  const commits = shas.map((sha) => collectCommit(repoRoot, traceMap, sha));
  const skippedMerge = commits.filter((commit) => commit.skipReason === "merge_commit").length;
  const skippedEmpty = commits.filter((commit) => commit.skipReason === "empty_commit").length;
  const skippedDocsOnly = commits.filter((commit) => commit.skipReason === "docs_only_churn").length;
  const considered = commits.filter((commit) => commit.status === "considered").length;

  return {
    version: 1,
    mode: "dry-run",
    base: { ref: baseRef, source: baseSource, sha: baseSha },
    head: { ref: options.head, sha: headSha },
    traceMap: {
      path: traceMap.path,
      version: traceMap.version,
      routeCount: traceMap.routes.length,
    },
    summary: {
      totalCommits: commits.length,
      consideredCommits: considered,
      skippedMergeCommits: skippedMerge,
      skippedEmptyCommits: skippedEmpty,
      skippedDocsOnlyCommits: skippedDocsOnly,
      noCommits: commits.length === 0,
    },
    commits,
  };
}

function timestampRunId() {
  return new Date().toISOString().replace(/[:.]/g, "-");
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_./:=@+-]+$/.test(value)) {
    return value;
  }
  return `'${value.replace(/'/g, "'\\''")}'`;
}

function commandLine(command, args) {
  return [command, ...args].map(shellQuote).join(" ");
}

function fullRunDir(options, runId) {
  return path.resolve(options.repoRoot, options.outDir ?? path.join(options.runRoot, runId));
}

function recordLifecycle(lifecycle, step) {
  lifecycle.push({
    status: step.status ?? "completed",
    name: step.name,
    command: step.command ?? null,
    note: step.note ?? "",
  });
}

function runLifecycleCommand(lifecycle, cwd, name, command, args, options = {}) {
  recordLifecycle(lifecycle, {
    name,
    command: commandLine(command, args),
    status: "running",
    note: options.note,
  });
  options.onChange?.();
  try {
    const output = execFileSync(command, args, {
      cwd,
      encoding: "utf8",
      stdio: options.stdio ?? ["ignore", "pipe", "pipe"],
      maxBuffer: 1024 * 1024 * 10,
    });
    lifecycle[lifecycle.length - 1].status = "completed";
    options.onChange?.();
    return output.trim();
  } catch (error) {
    lifecycle[lifecycle.length - 1].status = "failed";
    const stderr = error.stderr?.toString().trim();
    lifecycle[lifecycle.length - 1].note = stderr || error.message;
    options.onChange?.();
    throw error;
  }
}

function runStatePath(repoRoot, runRoot, runId) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(runId)) {
    throw new Error(`invalid --run-id: ${runId}`);
  }
  return path.resolve(repoRoot, runRoot, runId, "run-state.json");
}

function writeJsonAtomic(filePath, value) {
  mkdirSync(path.dirname(filePath), { recursive: true });
  const tempPath = `${filePath}.tmp-${process.pid}`;
  writeFileSync(tempPath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  renameSync(tempPath, filePath);
}

function persistRunState(options, state, lifecycle = null) {
  state.updatedAt = new Date().toISOString();
  if (lifecycle) state.lifecycle = Object.fromEntries(lifecycle.map((step) => [step.name, step.status]));
  writeJsonAtomic(runStatePath(options.repoRoot, options.runRoot, state.runId), state);
}

function readRunState(options, runId) {
  const statePath = runStatePath(options.repoRoot, options.runRoot, runId);
  if (!existsSync(statePath)) return null;
  const state = JSON.parse(readFileSync(statePath, "utf8"));
  if (state.schemaVersion !== runStateSchemaVersion || state.runId !== runId) {
    throw new Error(`unsupported or mismatched run state: ${repoRelative(options.repoRoot, statePath)}`);
  }
  return state;
}

function isTerminalRun(state) {
  return ["merged", "closed", "checkpointed", "no_changes"].includes(state.status);
}

function recordedNonterminalRuns(options) {
  const root = path.resolve(options.repoRoot, options.runRoot);
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => readRunState(options, entry.name))
    .filter((state) => state && !isTerminalRun(state));
}

function refSha(repoRoot, ref) {
  try {
    return git(repoRoot, ["rev-parse", "--verify", `${ref}^{commit}`]).trim();
  } catch {
    return null;
  }
}

function safeRunName(runId) {
  return runId.toLowerCase().replace(/[^a-z0-9-]+/g, "-").replace(/^-+|-+$/g, "");
}

function namesForRun(options, runId) {
  const suffix = safeRunName(runId);
  return {
    branch: options.sweepBranch === defaultSweepBranch ? `zvorygin/docdrift-sweep-${suffix}` : options.sweepBranch,
    worktree:
      options.sweepWorktree === defaultSweepWorktree
        ? `.docdrift/worktrees/docdrift-sweep-${suffix}`
        : options.sweepWorktree,
  };
}

function hasGitOperation(worktreePath) {
  const gitDir = git(worktreePath, ["rev-parse", "--git-dir"]);
  const absoluteGitDir = path.resolve(worktreePath, gitDir);
  return ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply"].some((name) =>
    existsSync(path.join(absoluteGitDir, name)),
  );
}

function assertCleanWorktree(worktreePath, branch) {
  const actualBranch = git(worktreePath, ["branch", "--show-current"]);
  const dirt = statusShort(worktreePath);
  if (actualBranch !== branch || dirt || hasGitOperation(worktreePath)) {
    throw new Error(
      `unsafe sweep worktree branch=${actualBranch || "(detached)"} expected=${branch} status=${JSON.stringify(dirt || "clean")} operation=${hasGitOperation(worktreePath) ? "in-progress" : "none"}`,
    );
  }
}

function lookupOwnedPr(options, branch) {
  const ghCommand = process.env.DOC_DRIFT_GH_COMMAND || "gh";
  const output = execFileSync(ghCommand, [
    "pr", "list", "--head", branch, "--state", "all", "--json",
    "number,url,state,mergeStateStatus,headRefOid,headRefName,mergedAt",
  ], { cwd: options.repoRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  const matches = JSON.parse(output).filter((pr) => pr.headRefName === branch);
  if (matches.length > 1) {
    throw new Error(`ambiguous PR matches for ${branch}: ${matches.map((pr) => `#${pr.number}`).join(", ")}`);
  }
  return matches[0] ?? null;
}

function newRunState(options, runId, baseSha, headSha) {
  const names = namesForRun(options, runId);
  return {
    schemaVersion: runStateSchemaVersion,
    runId,
    status: "initialized",
    baseSha,
    headSha,
    branch: names.branch,
    worktree: names.worktree,
    generatedHeadSha: null,
    pr: null,
    checkpointTarget: null,
    recoveryAction: "created_fresh_run",
    lifecycle: {},
    updatedAt: new Date().toISOString(),
  };
}

function validateRecordedRefs(options, state, pr) {
  const local = refSha(options.repoRoot, `refs/heads/${state.branch}`);
  const remote = refSha(options.repoRoot, `refs/remotes/origin/${state.branch}`);
  const expected = state.generatedHeadSha ?? state.headSha;
  let localCanFastForward = false;
  if (local && remote === expected && local !== remote) {
    try {
      git(options.repoRoot, ["merge-base", "--is-ancestor", local, remote], { stdio: "ignore" });
      localCanFastForward = true;
    } catch {}
  }
  if ((local && local !== expected && !localCanFastForward) || (remote && remote !== expected) || (pr?.headRefOid && pr.headRefOid !== expected)) {
    throw new Error(`run/head mismatch branch=${state.branch} expected=${expected} local=${local ?? "missing"} remote=${remote ?? "missing"} pr=${pr?.headRefOid ?? "missing"}`);
  }
  return { local, remote, expected, localCanFastForward };
}

function branchExists(repoRoot, branch) {
  try {
    git(repoRoot, ["show-ref", "--verify", "--quiet", `refs/heads/${branch}`], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function statusShort(repoRoot, pathspecs = []) {
  const args = ["status", "--short"];
  if (pathspecs.length > 0) {
    args.push("--", ...pathspecs);
  }
  return git(repoRoot, args);
}

function ensureSweepWorktree(options, lifecycle, state) {
  const worktreePath = path.resolve(options.repoRoot, state.worktree);
  mkdirSync(path.dirname(worktreePath), { recursive: true });
  const onChange = () => persistRunState(options, state, lifecycle);

  if (existsSync(worktreePath)) {
    const topLevel = git(worktreePath, ["rev-parse", "--show-toplevel"]);
    if (realpathSync(topLevel) !== realpathSync(worktreePath)) {
      throw new Error(`sweep worktree path is not its own checkout: ${worktreePath}`);
    }
    const branch = git(worktreePath, ["branch", "--show-current"]);
    assertCleanWorktree(worktreePath, state.branch);
    const actualHead = git(worktreePath, ["rev-parse", "HEAD"]);
    const expected = state.generatedHeadSha ?? state.headSha;
    if (actualHead !== expected) {
      const remote = refSha(options.repoRoot, `refs/remotes/origin/${state.branch}`);
      if (remote !== expected) throw new Error(`worktree head mismatch branch=${state.branch} expected=${expected} actual=${actualHead}`);
      runLifecycleCommand(lifecycle, worktreePath, "fast-forward recorded sweep worktree", "git", ["merge", "--ff-only", expected], { onChange });
    }
    return worktreePath;
  }

  if (branchExists(options.repoRoot, state.branch)) {
    runLifecycleCommand(lifecycle, options.repoRoot, "reuse sweep branch worktree", "git", [
      "worktree",
      "add",
      worktreePath,
      state.branch,
    ], { onChange });
    assertCleanWorktree(worktreePath, state.branch);
    const actualHead = git(worktreePath, ["rev-parse", "HEAD"]);
    const expected = state.generatedHeadSha ?? state.headSha;
    if (actualHead !== expected) {
      const remote = refSha(options.repoRoot, `refs/remotes/origin/${state.branch}`);
      if (remote !== expected) throw new Error(`local-only branch mismatch branch=${state.branch} expected=${expected} local=${actualHead}`);
      runLifecycleCommand(lifecycle, worktreePath, "fast-forward recorded sweep worktree", "git", ["merge", "--ff-only", expected], { onChange });
    }
    return worktreePath;
  }

  const remote = refSha(options.repoRoot, `refs/remotes/origin/${state.branch}`);
  if (remote) {
    const expected = state.generatedHeadSha ?? state.headSha;
    if (remote !== expected) throw new Error(`remote-only branch mismatch branch=${state.branch} expected=${expected} remote=${remote}`);
    runLifecycleCommand(lifecycle, options.repoRoot, "recreate local sweep branch", "git", ["branch", state.branch, remote], { onChange });
    runLifecycleCommand(lifecycle, options.repoRoot, "recreate sweep worktree", "git", ["worktree", "add", worktreePath, state.branch], { onChange });
    return worktreePath;
  }

  runLifecycleCommand(lifecycle, options.repoRoot, "create sweep worktree", "git", [
    "worktree",
    "add",
    worktreePath,
    "-b",
    state.branch,
    state.headSha,
  ], { onChange });
  state.status = "branch_ready";
  persistRunState(options, state, lifecycle);
  return worktreePath;
}

function recordRecoveryFailure(options, error) {
  if (!options.full) return;
  let state = options.runId ? readRunState(options, options.runId) : null;
  if (!state) {
    const candidates = recordedNonterminalRuns(options);
    if (candidates.length === 1) state = candidates[0];
  }
  if (!state) return;
  state.recoveryAction = "stopped_for_operator_review";
  state.lastError = error.message;
  persistRunState(options, state);
  const dir = fullRunDir(options, state.runId);
  writeJsonAtomic(path.join(dir, "docdrift-recovery-failure.json"), {
    version: 1,
    runId: state.runId,
    branch: state.branch,
    worktree: state.worktree,
    generatedHeadSha: state.generatedHeadSha,
    pr: state.pr,
    checkpointTarget: state.checkpointTarget,
    recoveryAction: state.recoveryAction,
    error: error.message,
    updatedAt: state.updatedAt,
  });
}

function parseAgentPrOutput(output) {
  const match = /agent-pr: PR (\d+) ready: (\S+)/.exec(output);
  if (!match) {
    throw new Error(`could not parse agent-pr output: ${output}`);
  }
  return {
    number: Number.parseInt(match[1], 10),
    url: match[2],
  };
}

function adoptLegacyRun(options) {
  const local = refSha(options.repoRoot, `refs/heads/${legacySweepBranch}`);
  const remote = refSha(options.repoRoot, `refs/remotes/origin/${legacySweepBranch}`);
  const worktreePath = path.resolve(options.repoRoot, defaultSweepWorktree);
  if (!local && !remote && !existsSync(worktreePath)) return null;
  if (!local || !remote || !existsSync(worktreePath)) {
    throw new Error(`legacy adoption requires local, remote, and worktree heads: branch=${legacySweepBranch} local=${local ?? "missing"} remote=${remote ?? "missing"} worktree=${existsSync(worktreePath) ? "present" : "missing"}`);
  }
  assertCleanWorktree(worktreePath, legacySweepBranch);
  const worktreeHead = git(worktreePath, ["rev-parse", "HEAD"]);
  if (local !== remote || local !== worktreeHead) {
    throw new Error(`legacy head mismatch branch=${legacySweepBranch} local=${local} remote=${remote} worktree=${worktreeHead}`);
  }
  const pr = lookupOwnedPr(options, legacySweepBranch);
  if (!pr || !["CLOSED", "MERGED"].includes(pr.state) || pr.headRefOid !== local) {
    throw new Error(`legacy adoption requires exactly one terminal PR with exact head: branch=${legacySweepBranch} head=${local} pr=${pr ? `#${pr.number}/${pr.state}/${pr.headRefOid}` : "missing"}`);
  }
  if (!local.startsWith(legacyKnownHeadPrefix) && !options.adoptLegacy) {
    throw new Error(`legacy head ${local} is not the known ${legacyKnownHeadPrefix} fixture; rerun with --adopt-legacy after operator review`);
  }
  const runId = `legacy-docdrift-sweep-${local.slice(0, 12)}`;
  const existing = readRunState(options, runId);
  if (existing) return existing;
  const state = {
    schemaVersion: runStateSchemaVersion,
    runId,
    status: pr.state === "MERGED" ? "merged" : "closed",
    baseSha: null,
    headSha: local,
    branch: legacySweepBranch,
    worktree: defaultSweepWorktree,
    generatedHeadSha: local,
    pr: {
      number: pr.number,
      url: pr.url,
      state: pr.state,
      mergeStateStatus: pr.mergeStateStatus ?? null,
      headSha: pr.headRefOid,
    },
    checkpointTarget: null,
    recoveryAction: "adopted_terminal_legacy_run",
    lifecycle: {},
    updatedAt: new Date().toISOString(),
  };
  persistRunState(options, state);
  return state;
}

function selectRecordedRun(options) {
  if (options.runId) return readRunState(options, options.runId);
  const candidates = recordedNonterminalRuns(options);
  if (candidates.length > 1) {
    throw new Error(`multiple nonterminal docdrift runs: ${candidates.map((state) => `${state.runId}:${state.branch}`).join(", ")}; pass --run-id`);
  }
  return candidates[0] ?? null;
}

function assertFreshRunTargets(options, state) {
  const worktreePath = path.resolve(options.repoRoot, state.worktree);
  const local = refSha(options.repoRoot, `refs/heads/${state.branch}`);
  const remote = refSha(options.repoRoot, `refs/remotes/origin/${state.branch}`);
  if (local || remote || existsSync(worktreePath)) {
    throw new Error(`new run target collision run=${state.runId} branch=${state.branch} local=${local ?? "missing"} remote=${remote ?? "missing"} worktree=${existsSync(worktreePath) ? worktreePath : "missing"}`);
  }
}

function checkpointAdvanceTarget(report, sweepHeadSha) {
  const partialTarget = partialCheckpointTarget(report);
  if (partialTarget) {
    return partialTarget;
  }
  if (report.docPatch?.partial) {
    return null;
  }
  return sweepHeadSha || report.head.sha;
}

function partialCheckpointTarget(report) {
  const failedSha = report.docPatch?.failure?.commitSha;
  if (!failedSha) {
    return null;
  }
  const failedIndex = report.commits.findIndex((commit) => commit.sha === failedSha);
  if (failedIndex <= 0) {
    return null;
  }
  return report.commits[failedIndex - 1].sha;
}

function fullReport({ options, runId, runDir, lifecycle, collectionReport, generatedReport, action, checkpointAfter, pr, sweepHeadSha, state = null }) {
  const sourceReport = generatedReport ?? collectionReport;
  return {
    version: 1,
    mode: "full",
    dryRun: options.dryRun,
    run: {
      id: runId,
      outDir: repoRelative(options.repoRoot, runDir),
    },
    base: sourceReport.base,
    head: sourceReport.head,
    traceMap: sourceReport.traceMap,
    summary: sourceReport.summary,
    classifier: generatedReport?.classifier ?? null,
    docPatch: generatedReport?.docPatch ?? null,
    lifecycle,
    sweep: {
      action,
      recoveryAction: state?.recoveryAction ?? (options.dryRun ? "preview_only" : null),
      branch: state?.branch ?? options.sweepBranch,
      worktree: repoRelative(options.repoRoot, path.resolve(options.repoRoot, state?.worktree ?? options.sweepWorktree)),
      headSha: sweepHeadSha ?? null,
      prNumber: pr?.number ?? null,
      prUrl: pr?.url ?? null,
    },
    checkpoint: {
      file: options.checkpointFile,
      seedFile: options.checkpointSeedFile,
      advanced: Boolean(checkpointAfter),
      after: checkpointAfter,
    },
  };
}

function plannedFullLifecycle(options) {
  const worktreePath = path.resolve(options.repoRoot, options.sweepWorktree);
  return [
    {
      status: "planned",
      name: "fetch origin/main",
      command: commandLine("git", ["fetch", "origin", "main"]),
      note: "Refresh the target before collecting commits.",
    },
    {
      status: "planned",
      name: "create or reuse sweep worktree",
      command: commandLine("git", ["worktree", "add", worktreePath, "-b", options.sweepBranch, options.head]),
      note: "Actual runs reuse the existing clean sweep worktree when possible.",
    },
    {
      status: "planned",
      name: "generate docs",
      command: commandLine("node", ["scripts/docdrift-sweep.mjs", "--generate-docs", "--base", "<checkpoint>", "--head", options.head]),
      note: "Runs classifier and doc patch generation inside the isolated worktree.",
    },
    {
      status: "planned",
      name: "commit docs changes",
      command: commandLine("git", ["add", "docs/design", "docs/context"]),
      note: "Skipped when no docs changed.",
    },
    {
      status: "planned",
      name: "push sweep branch",
      command: commandLine("git", ["push", "-u", "origin", options.sweepBranch]),
      note: "Skipped when no docs changed.",
    },
    {
      status: "planned",
      name: "open or update owned PR",
      command: commandLine("scripts/agent-pr.sh", ["--title", options.prTitle, "--verification", "docdrift full sweep", "--label", "docdrift-sweep"]),
      note: "Arms auto-merge through the standard PR helper.",
    },
    {
      status: "planned",
      name: "wait for merge",
      command: commandLine("scripts/wait-pr.sh", ["<pr>"]),
      note: "Checkpoint advances only after this proves reachability from origin/main.",
    },
  ];
}

export function runFullSweep(options) {
  let runId = options.runId ?? timestampRunId();
  const runDir = fullRunDir(options, runId);
  mkdirSync(runDir, { recursive: true });

  if (options.dryRun) {
    const collectionReport = buildReport(options);
    const report = fullReport({
      options,
      runId,
      runDir,
      lifecycle: plannedFullLifecycle(options),
      collectionReport,
      generatedReport: null,
      action: collectionReport.summary.noCommits ? "noop_no_commits" : "preview_only",
      checkpointAfter: null,
      pr: null,
      sweepHeadSha: null,
    });
    writeOutputs(report, runDir);
    return report;
  }

  const lifecycle = [];
  runLifecycleCommand(lifecycle, options.repoRoot, "fetch origin/main", "git", ["fetch", "origin", "main"]);
  let state = selectRecordedRun(options);
  let recoveryAction = null;
  let terminalRunId = null;
  if (!state) {
    const legacy = adoptLegacyRun(options);
    if (legacy) recoveryAction = legacy.status === "merged" ? "created_fresh_run_after_merged_legacy" : "created_fresh_run_after_closed_legacy";
  }

  if (state) {
    const pr = lookupOwnedPr(options, state.branch);
    const refs = validateRecordedRefs(options, state, pr);
    if (!refs.local && !refs.remote && (state.status !== "initialized" || state.generatedHeadSha || state.pr)) {
      throw new Error(`recorded branch is missing locally and remotely: run=${state.runId} branch=${state.branch} expected=${refs.expected}`);
    }
    if (pr) {
      state.pr = {
        number: pr.number,
        url: pr.url,
        state: pr.state,
        mergeStateStatus: pr.mergeStateStatus ?? null,
        headSha: pr.headRefOid,
      };
    } else if (state.pr) {
      throw new Error(`recorded PR #${state.pr.number} is no longer the unique PR match for ${state.branch}`);
    }
    if (pr?.state === "OPEN" && ["DIRTY", "CONFLICTING"].includes(pr.mergeStateStatus)) {
      throw new Error(`open PR is conflicted: branch=${state.branch} head=${refs.expected} pr=#${pr.number} mergeStateStatus=${pr.mergeStateStatus}`);
    }
    if (pr?.state === "CLOSED") {
      state.status = "closed";
      state.recoveryAction = "preserved_closed_unmerged_run";
      persistRunState(options, state, lifecycle);
      recoveryAction = "created_fresh_run_after_closed_unmerged";
      terminalRunId = state.runId;
      state = null;
    } else if (pr?.state === "MERGED") {
      state.status = "merged";
      state.recoveryAction = "completed_merged_run";
      if (state.checkpointTarget) {
        writeCheckpointAtomic(options.repoRoot, options.checkpointFile, state.checkpointTarget);
        state.status = "checkpointed";
      }
      persistRunState(options, state, lifecycle);
      recoveryAction = "created_fresh_run_after_merged";
      terminalRunId = state.runId;
      state = null;
    } else if (pr?.state === "OPEN") {
      state.recoveryAction = "resumed_open_pr";
      persistRunState(options, state, lifecycle);
      const worktreePath = ensureSweepWorktree(options, lifecycle, state);
      const waitCommand = process.env.DOC_DRIFT_WAIT_PR_COMMAND || path.join(worktreePath, "scripts", "wait-pr.sh");
      runLifecycleCommand(lifecycle, worktreePath, "wait for PR merge", waitCommand, [String(pr.number)], {
        onChange: () => persistRunState(options, state, lifecycle),
      });
      const checkpointPath = state.checkpointTarget
        ? writeCheckpointAtomic(options.repoRoot, options.checkpointFile, state.checkpointTarget)
        : null;
      state.status = state.checkpointTarget ? "checkpointed" : "merged";
      state.pr.state = "MERGED";
      persistRunState(options, state, lifecycle);
      const resumedOptions = { ...options, base: state.baseSha, head: state.headSha };
      const collectionReport = buildReport(resumedOptions);
      const generatedPath = path.join(fullRunDir(options, state.runId), "docdrift-generate.json");
      const generatedReport = existsSync(generatedPath) ? JSON.parse(readFileSync(generatedPath, "utf8")) : null;
      const report = fullReport({
        options,
        runId: state.runId,
        runDir: fullRunDir(options, state.runId),
        lifecycle,
        collectionReport,
        generatedReport,
        action: "resumed_open_pr_merged",
        checkpointAfter: checkpointPath ? { path: checkpointPath, sha: state.checkpointTarget } : null,
        pr: state.pr,
        sweepHeadSha: state.generatedHeadSha,
        state,
      });
      writeOutputs(report, fullRunDir(options, state.runId));
      return report;
    } else {
      state.recoveryAction = "resumed_recorded_pre_pr_run";
      persistRunState(options, state, lifecycle);
    }
  }

  let collectionOptions = options;
  if (state) collectionOptions = { ...options, base: state.baseSha, head: state.headSha };
  const collectionReport = buildReport(collectionOptions);

  if (!state) {
    runId = terminalRunId ? `${terminalRunId}-next-${timestampRunId()}` : options.runId ?? timestampRunId();
    state = newRunState(options, runId, collectionReport.base.sha, collectionReport.head.sha);
    state.recoveryAction = recoveryAction ?? "created_fresh_run";
    assertFreshRunTargets(options, state);
    persistRunState(options, state, lifecycle);
  }
  const activeRunDir = fullRunDir(options, state.runId);
  mkdirSync(activeRunDir, { recursive: true });

  if (collectionReport.summary.noCommits) {
    state.status = "no_changes";
    state.recoveryAction = recoveryAction ?? state.recoveryAction;
    persistRunState(options, state, lifecycle);
    const report = fullReport({
      options,
      runId: state.runId,
      runDir: activeRunDir,
      lifecycle,
      collectionReport,
      generatedReport: null,
      action: "noop_no_commits",
      checkpointAfter: null,
      pr: null,
      sweepHeadSha: null,
      state,
    });
    writeOutputs(report, activeRunDir);
    return report;
  }

  if (collectionReport.summary.consideredCommits === 0) {
    const checkpointPath = writeCheckpointAtomic(options.repoRoot, options.checkpointFile, collectionReport.head.sha);
    state.status = "checkpointed";
    state.checkpointTarget = collectionReport.head.sha;
    persistRunState(options, state, lifecycle);
    const report = fullReport({
      options,
      runId: state.runId,
      runDir: activeRunDir,
      lifecycle,
      collectionReport,
      generatedReport: null,
      action: "noop_no_considered_commits",
      checkpointAfter: { path: checkpointPath, sha: collectionReport.head.sha },
      pr: null,
      sweepHeadSha: null,
      state,
    });
    writeOutputs(report, activeRunDir);
    return report;
  }

  validateRecordedRefs(options, state, null);
  const worktreePath = ensureSweepWorktree(options, lifecycle, state);
  const sweepOptions = {
    ...options,
    base: collectionReport.base.sha,
    checkpointRef: null,
    dryRun: false,
    full: false,
    generateDocs: true,
    outDir: null,
    repoRoot: worktreePath,
  };
  let generatedReport;
  const generatedPath = path.join(activeRunDir, "docdrift-generate.json");
  if (state.generatedHeadSha && existsSync(generatedPath)) {
    generatedReport = JSON.parse(readFileSync(generatedPath, "utf8"));
  } else {
    generatedReport = classifyReport(buildReport(sweepOptions), sweepOptions);
    generatedReport = generateDocsReport(generatedReport, sweepOptions);
    writeOutputs(generatedReport, activeRunDir);
    state.status = "generated";
    persistRunState(options, state, lifecycle);
  }

  if (state.generatedHeadSha && ["committed", "pushed"].includes(state.status)) {
    const onStateChange = () => persistRunState(options, state, lifecycle);
    if (state.status === "committed") {
      runLifecycleCommand(lifecycle, worktreePath, "push sweep branch", "git", ["push", "-u", "origin", state.branch], { onChange: onStateChange });
      state.status = "pushed";
      persistRunState(options, state, lifecycle);
    }
    const existingPr = lookupOwnedPr(options, state.branch);
    if (existingPr) throw new Error(`recorded pre-PR run unexpectedly matches PR #${existingPr.number}; rerun for exact PR reconciliation`);
    const bodyPath = writeFullPrBody(
      path.join(activeRunDir, "docdrift-pr-body.md"),
      generatedReport,
      repoRelative(options.repoRoot, generatedPath),
    );
    const agentPrCommand = process.env.DOC_DRIFT_AGENT_PR_COMMAND || path.join(worktreePath, "scripts", "agent-pr.sh");
    const agentOutput = runLifecycleCommand(lifecycle, worktreePath, "open or update owned PR", agentPrCommand, [
      "--title", options.prTitle,
      "--verification", `node scripts/docdrift-sweep.mjs --generate-docs report ${repoRelative(options.repoRoot, generatedPath)}`,
      "--label", "docdrift-sweep", "--body-file", bodyPath,
    ], { onChange: onStateChange });
    const pr = parseAgentPrOutput(agentOutput);
    state.pr = { ...pr, state: "OPEN", mergeStateStatus: null, headSha: state.generatedHeadSha };
    state.status = "pr_open";
    persistRunState(options, state, lifecycle);
    const waitCommand = process.env.DOC_DRIFT_WAIT_PR_COMMAND || path.join(worktreePath, "scripts", "wait-pr.sh");
    runLifecycleCommand(lifecycle, worktreePath, "wait for PR merge", waitCommand, [String(pr.number)], { onChange: onStateChange });
    const checkpointSha = state.checkpointTarget;
    const checkpointPath = checkpointSha ? writeCheckpointAtomic(options.repoRoot, options.checkpointFile, checkpointSha) : null;
    state.status = checkpointSha ? "checkpointed" : "merged";
    state.pr.state = "MERGED";
    persistRunState(options, state, lifecycle);
    const report = fullReport({
      options, runId: state.runId, runDir: activeRunDir, lifecycle, collectionReport, generatedReport,
      action: "resumed_pre_pr_run_merged",
      checkpointAfter: checkpointPath ? { path: checkpointPath, sha: checkpointSha } : null,
      pr: state.pr, sweepHeadSha: state.generatedHeadSha, state,
    });
    writeOutputs(report, activeRunDir);
    return report;
  }

  const docsDirt = statusShort(worktreePath, ["docs/design", "docs/context"]);
  if (!docsDirt) {
    const checkpointSha = checkpointAdvanceTarget(generatedReport, null);
    const checkpointPath = checkpointSha ? writeCheckpointAtomic(options.repoRoot, options.checkpointFile, checkpointSha) : null;
    state.status = checkpointSha ? "checkpointed" : "no_changes";
    state.checkpointTarget = checkpointSha;
    persistRunState(options, state, lifecycle);
    const report = fullReport({
      options,
      runId: state.runId,
      runDir: activeRunDir,
      lifecycle,
      collectionReport,
      generatedReport,
      action: generatedReport.docPatch?.partial
        ? checkpointSha
          ? "partial_failure_no_doc_changes_checkpoint_advanced"
          : "partial_failure_no_doc_changes_checkpoint_unchanged"
        : "noop_no_doc_changes",
      checkpointAfter: checkpointSha ? { path: checkpointPath, sha: checkpointSha } : null,
      pr: null,
      sweepHeadSha: null,
      state,
    });
    writeOutputs(report, activeRunDir);
    return report;
  }

  const onStateChange = () => persistRunState(options, state, lifecycle);
  runLifecycleCommand(lifecycle, worktreePath, "stage docs changes", "git", ["add", "docs/design", "docs/context"], { onChange: onStateChange });
  runLifecycleCommand(lifecycle, worktreePath, "commit docs changes", "git", [
    "commit",
    "-m",
    generatedReport.docPatch?.partial ? "Sweep documentation drift partial prefix" : "Sweep documentation drift",
    "-m",
    [
      `Generated docs updates for ${generatedReport.base.sha.slice(0, 12)}..${(checkpointAdvanceTarget(generatedReport, null) ?? generatedReport.head.sha).slice(0, 12)}.`,
      generatedReport.docPatch?.partial
        ? `Partial sweep stopped at ${generatedReport.docPatch.failure.shortSha}: ${generatedReport.docPatch.failure.message}`
        : null,
      generatedReport.docPatch?.summary?.skipped
        ? `Skipped doc-patch decisions: ${generatedReport.docPatch.summary.skipped}`
        : null,
      `Report: ${repoRelative(options.repoRoot, activeRunDir)}/docdrift-generate.json`,
    ]
      .filter(Boolean)
      .join("\n\n"),
  ], { onChange: onStateChange });
  const sweepHeadSha = git(worktreePath, ["rev-parse", "HEAD"]);
  state.generatedHeadSha = sweepHeadSha;
  state.checkpointTarget = checkpointAdvanceTarget(generatedReport, sweepHeadSha);
  state.status = "committed";
  persistRunState(options, state, lifecycle);
  runLifecycleCommand(lifecycle, worktreePath, "push sweep branch", "git", ["push", "-u", "origin", state.branch], { onChange: onStateChange });
  state.status = "pushed";
  persistRunState(options, state, lifecycle);

  const bodyPath = writeFullPrBody(
    path.join(activeRunDir, "docdrift-pr-body.md"),
    generatedReport,
    repoRelative(options.repoRoot, path.join(activeRunDir, "docdrift-generate.json")),
  );
  const agentPrCommand = process.env.DOC_DRIFT_AGENT_PR_COMMAND || path.join(worktreePath, "scripts", "agent-pr.sh");
  const agentOutput = runLifecycleCommand(lifecycle, worktreePath, "open or update owned PR", agentPrCommand, [
    "--title",
    options.prTitle,
    "--verification",
    `node scripts/docdrift-sweep.mjs --generate-docs report ${repoRelative(options.repoRoot, activeRunDir)}/docdrift-generate.json`,
    "--label", "docdrift-sweep", "--body-file", bodyPath,
  ], { onChange: onStateChange });
  const pr = parseAgentPrOutput(agentOutput);
  state.pr = { ...pr, state: "OPEN", mergeStateStatus: null, headSha: sweepHeadSha };
  state.status = "pr_open";
  persistRunState(options, state, lifecycle);
  const waitCommand = process.env.DOC_DRIFT_WAIT_PR_COMMAND || path.join(worktreePath, "scripts", "wait-pr.sh");
  runLifecycleCommand(lifecycle, worktreePath, "wait for PR merge", waitCommand, [String(pr.number)], { onChange: onStateChange });

  const checkpointSha = checkpointAdvanceTarget(generatedReport, sweepHeadSha);
  const checkpointPath = checkpointSha ? writeCheckpointAtomic(options.repoRoot, options.checkpointFile, checkpointSha) : null;
  state.status = checkpointSha ? "checkpointed" : "merged";
  state.pr.state = "MERGED";
  state.checkpointTarget = checkpointSha;
  persistRunState(options, state, lifecycle);
  const report = fullReport({
    options,
    runId: state.runId,
    runDir: activeRunDir,
    lifecycle,
    collectionReport,
    generatedReport,
    action: generatedReport.docPatch?.partial
      ? checkpointSha
        ? "partial_pr_merged_checkpoint_advanced"
        : "partial_pr_merged_checkpoint_unchanged"
      : "pr_merged_checkpoint_advanced",
    checkpointAfter: checkpointSha ? { path: checkpointPath, sha: checkpointSha } : null,
    pr,
    sweepHeadSha,
    state,
  });
  writeOutputs(report, activeRunDir);
  return report;
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    usage();
    process.exit(2);
  }

  if (options.help) {
    usage();
    return;
  }

  try {
    let report;
    if (options.full) {
      report = runFullSweep(options);
    } else {
      report = buildReport(options);
      if (options.classify || options.generateDocs) {
        report = classifyReport(report, options);
      }
      if (options.generateDocs) {
        report = generateDocsReport(report, options);
      }
      if (options.outDir) {
        writeOutputs(report, options.outDir);
      }
    }
    if (options.format === "json") {
      console.log(JSON.stringify(report, null, 2));
    } else {
      process.stdout.write(renderMarkdown(report));
    }
    if (report.docPatch?.partial) {
      process.exitCode = 1;
    }
  } catch (error) {
    try {
      recordRecoveryFailure(options, error);
    } catch (recordError) {
      console.error(`docdrift recovery failure report failed: ${recordError.message}`);
    }
    console.error(`docdrift sweep failed: ${error.message}`);
    process.exit(1);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}
