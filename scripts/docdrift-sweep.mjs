#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const defaultRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultCheckpointFile = "docs/docdrift-checkpoint.txt";
const defaultTraceMapPath = "docs/doc-map.json";
const defaultClassifierCacheDir = ".docdrift/classifier-cache";
const defaultDocPatchCacheDir = ".docdrift/doc-patch-cache";
const classifierPromptVersion = "docdrift-classifier-v1";
const docPatchPromptVersion = "docdrift-doc-patch-v1";
const validClassifierDecisions = new Set(["move_on", "update_docs"]);
const validDocPatchPrefixes = ["docs/design/", "docs/context/"];

function usage() {
  console.log(`Usage:
  node scripts/docdrift-sweep.mjs --dry-run [options]
  node scripts/docdrift-sweep.mjs --classify [options]
  node scripts/docdrift-sweep.mjs --generate-docs [options]

Options:
  --base REF                  Override the reviewed checkpoint ref.
  --classify                  Run the cheap Codex-backed classifier for considered commits.
  --classifier-cache DIR      Cache directory for classifier records. Default: ${defaultClassifierCacheDir}
  --codex-arg ARG             Extra Codex CLI argument for live classify mode. Repeatable.
  --codex-command COMMAND     Codex CLI command for live classify mode. Default: codex
  --codex-model MODEL         Optional model passed to Codex CLI with --model.
  --checkpoint-file PATH      Checkpoint file used when --base is omitted. Default: ${defaultCheckpointFile}
  --checkpoint-ref REF        Optional checkpoint ref used when --base is omitted.
  --doc-patch-cache DIR       Cache directory for generated doc patch records. Default: ${defaultDocPatchCacheDir}
  --dry-run                   Build the deterministic Phase 1 report without classification.
  --fixture NAME_OR_PATH      Fixture response set for --classify/--generate-docs --no-codex.
  --generate-docs             Generate and apply minimal docs patches for update_docs decisions.
  --head REF                  Sweep target. Default: origin/main.
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
patches in the working tree, and never creates PRs or advances the checkpoint.`);
}

export function parseArgs(argv) {
  const options = {
    base: null,
    classifierCacheDir: defaultClassifierCacheDir,
    classify: false,
    codexArgs: [],
    codexCommand: "codex",
    codexModel: null,
    checkpointFile: defaultCheckpointFile,
    checkpointRef: null,
    docPatchCacheDir: defaultDocPatchCacheDir,
    dryRun: false,
    fixture: null,
    format: "markdown",
    generateDocs: false,
    head: "origin/main",
    maxCommits: 25,
    maxDocPromptTokens: 8000,
    maxPromptTokens: 4000,
    maxTotalDocPromptTokens: 40000,
    maxTotalPromptTokens: 20000,
    noCodex: false,
    outDir: null,
    repoRoot: defaultRepoRoot,
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
    } else if (arg === "--checkpoint-ref" || arg.startsWith("--checkpoint-ref=")) {
      options.checkpointRef = readValue("--checkpoint-ref");
    } else if (arg === "--checkpoint-file" || arg.startsWith("--checkpoint-file=")) {
      options.checkpointFile = readValue("--checkpoint-file");
    } else if (arg === "--doc-patch-cache" || arg.startsWith("--doc-patch-cache=")) {
      options.docPatchCacheDir = readValue("--doc-patch-cache");
    } else if (arg === "--fixture" || arg.startsWith("--fixture=")) {
      options.fixture = readValue("--fixture");
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
    } else if (arg === "--repo" || arg.startsWith("--repo=")) {
      options.repoRoot = readValue("--repo");
    } else {
      throw new Error(`unknown option: ${arg}`);
    }
  }

  if (!["markdown", "json"].includes(options.format)) {
    throw new Error("--format must be markdown or json");
  }
  const modeCount = [options.dryRun, options.classify, options.generateDocs].filter(Boolean).length;
  if (!options.help && modeCount !== 1) {
    throw new Error("choose exactly one mode: --dry-run, --classify, or --generate-docs");
  }
  if (!options.help && options.noCodex && (options.classify || options.generateDocs) && !options.fixture) {
    throw new Error("--no-codex requires --fixture in classify or generate-docs mode");
  }
  if (!options.help && options.fixture && !options.noCodex) {
    throw new Error("--fixture is only valid with --no-codex");
  }
  if (
    !options.help &&
    (options.classify || options.generateDocs) &&
    !options.noCodex &&
    !path.basename(options.codexCommand).includes("codex")
  ) {
    throw new Error("--codex-command must point to a Codex CLI command");
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
  return execFileSync("git", ["-C", repoRoot, ...args], {
    encoding: "utf8",
    stdio: options.stdio ?? ["ignore", "pipe", "pipe"],
  }).trimEnd();
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
  const args = ["exec", "--sandbox", "read-only", "--ask-for-approval", "never", "--ephemeral"];
  if (options.codexModel) {
    args.push("--model", options.codexModel);
  }
  args.push(...options.codexArgs, "--output-last-message", outputPath, "-");
  return args;
}

function classifyWithCodex(options, prompt) {
  const tempDir = mkdtempSync(path.join(os.tmpdir(), "rts-docdrift-codex-"));
  const outputPath = path.join(tempDir, "last-message.txt");
  const args = codexInvocationArgs(options, outputPath);
  try {
    execFileSync(options.codexCommand, args, {
      cwd: options.repoRoot,
      encoding: "utf8",
      input: prompt,
      maxBuffer: 1024 * 1024 * 5,
      stdio: ["pipe", "pipe", "pipe"],
    });
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
      },
    };
  } catch (error) {
    const stderr = error.stderr?.toString().trim();
    throw new Error(`Codex classifier failed: ${stderr || error.message}`);
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
  for (const entry of promptEntries) {
    const relativeCachePath = repoRelative(options.repoRoot, entry.cachePath);
    if (entry.cached?.cache?.promptHash === entry.promptHash) {
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
  const docs = new Set();
  for (const doc of decision.likelyDocs ?? []) {
    if (doc.startsWith("docs/design/")) {
      docs.add(doc);
    }
  }
  for (const doc of commit.traceDocs ?? []) {
    if (doc.startsWith("docs/design/")) {
      docs.add(doc);
    }
  }
  for (const doc of commit.docsTouched?.design ?? []) {
    docs.add(doc);
  }
  return [...docs].sort();
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
  const selected = scored
    .filter((entry, index) => entry.score > 0 || index < 2)
    .slice(0, 4)
    .sort((a, b) => a.index - b.index)
    .map((entry) => ({
      heading: entry.section.heading,
      level: entry.section.level,
      startLine: entry.section.startLine,
      endLine: entry.section.endLine,
      text: entry.section.text.slice(0, 5000),
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

function applyDocPatches(repoRoot, patches) {
  const results = [];
  for (const patch of patches) {
    const absPath = path.resolve(repoRoot, patch.path);
    if (!existsSync(absPath)) {
      throw new Error(`doc patch target not found: ${patch.path}`);
    }
    const current = readFileSync(absPath, "utf8");
    if (current.includes(patch.replace)) {
      results.push({ path: patch.path, status: "already_applied", rationale: patch.rationale });
      continue;
    }
    const findCount = countOccurrences(current, patch.find);
    if (findCount === 1) {
      writeFileSync(absPath, current.replace(patch.find, patch.replace));
      results.push({ path: patch.path, status: "applied", rationale: patch.rationale });
      continue;
    }
    if (findCount === 0) {
      throw new Error(`doc patch find text not found in ${patch.path}`);
    }
    throw new Error(`doc patch find text matched ${findCount} times in ${patch.path}`);
  }
  return results;
}

function generateDocPatchWithCodex(options, prompt) {
  const tempDir = mkdtempSync(path.join(os.tmpdir(), "rts-docdrift-doc-patch-"));
  const outputPath = path.join(tempDir, "last-message.txt");
  const args = codexInvocationArgs(options, outputPath);
  try {
    execFileSync(options.codexCommand, args, {
      cwd: options.repoRoot,
      encoding: "utf8",
      input: prompt,
      maxBuffer: 1024 * 1024 * 5,
      stdio: ["pipe", "pipe", "pipe"],
    });
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
      },
    };
  } catch (error) {
    const stderr = error.stderr?.toString().trim();
    throw new Error(`Codex doc patch generator failed: ${stderr || error.message}`);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function buildDocPatchRecord({ commit, decision, docTargets, docSections, patchResult, promptHash, promptTokens, cachePath, cacheHit, invocation, applications }) {
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

export function generateDocsReport(classifierReport, options) {
  const updateDecisions = classifierReport.classifier.decisions.filter((decision) => decision.decision === "update_docs");
  const commitBySha = new Map(classifierReport.commits.map((commit) => [commit.sha, commit]));
  const cacheRoot = path.resolve(options.repoRoot, options.docPatchCacheDir);
  const entries = updateDecisions.map((decision) => {
    const commit = commitBySha.get(decision.commitSha);
    if (!commit) {
      throw new Error(`classifier decision references missing commit: ${decision.commitSha}`);
    }
    const docTargets = designDocTargetsForDecision(decision, commit);
    const keywords = commitKeywords(commit, decision);
    const docSections = docTargets.map((docPath) => selectDocSections(options.repoRoot, docPath, keywords));
    const cachePath = docPatchCachePath(options.repoRoot, options.docPatchCacheDir, commit.sha);
    const cached = readCachedClassifierRecord(cachePath);
    const cacheInput = docPatchPromptInput({ commit, decision, docTargets, docSections, priorCachedPatch: null });
    const promptHash = sha256(JSON.stringify(stableJson(cacheInput)));
    const priorCachedPatch = cached
      ? {
          summary: cached.summary,
          patches: cached.patches,
          promptHash: cached.cache?.promptHash,
        }
      : null;
    const cacheHit = cached?.cache?.promptHash === promptHash;
    const promptInput = cacheHit
      ? cacheInput
      : docPatchPromptInput({ commit, decision, docTargets, docSections, priorCachedPatch });
    const prompt = renderDocPatchPrompt(promptInput);
    return {
      commit,
      decision,
      docTargets,
      docSections,
      cachePath,
      cached,
      cacheHit,
      prompt,
      promptHash,
      promptTokens: estimatePromptTokens(prompt),
    };
  });
  enforceDocPatchBudgets(entries, options);

  const loadedFixture = options.noCodex ? loadDocPatchFixture(options.repoRoot, options.fixture) : null;
  const patchRecords = [];
  for (const entry of entries) {
    const relativeCachePath = repoRelative(options.repoRoot, entry.cachePath);
    let patchResult;
    let invocation;
    if (entry.cacheHit && entry.cached?.rawPatch) {
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
      const codexResult = generateDocPatchWithCodex(options, entry.prompt);
      patchResult = sanitizeDocPatchResult(parseJsonObject(codexResult.rawText, `Codex doc patch output for ${entry.commit.shortSha}`), entry.commit);
      invocation = codexResult.invocation;
    }
    const applications = applyDocPatches(options.repoRoot, patchResult.patches);
    const record = buildDocPatchRecord({
      commit: entry.commit,
      decision: entry.decision,
      docTargets: entry.docTargets,
      docSections: entry.docSections,
      patchResult,
      promptHash: entry.promptHash,
      promptTokens: entry.promptTokens,
      cachePath: relativeCachePath,
      cacheHit: entry.cacheHit,
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
      budget: {
        maxDocPromptTokens: options.maxDocPromptTokens,
        maxTotalDocPromptTokens: options.maxTotalDocPromptTokens,
        estimatedPromptTokens: entries.filter((entry) => !entry.cacheHit).reduce((sum, entry) => sum + entry.promptTokens, 0),
      },
      records: patchRecords,
      summary: {
        updateDocsDecisions: updateDecisions.length,
        patchRecords: patchRecords.length,
        patches: patchRecords.reduce((sum, record) => sum + record.patches.length, 0),
        applied,
        alreadyApplied,
        cacheHits: patchRecords.filter((record) => record.cache.hit).length,
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
    const checkpoint = readCheckpointFile(repoRoot, options.checkpointFile);
    baseRef = checkpoint.value;
    baseSource = checkpoint.source;
  }
  const baseSha = resolveCommit(repoRoot, baseRef, baseSource);
  const headSha = resolveCommit(repoRoot, options.head, "--head");
  const revListOutput = git(repoRoot, ["rev-list", "--reverse", `${baseSha}..${headSha}`]);
  const shas = revListOutput.split("\n").map((line) => line.trim()).filter(Boolean);
  const commits = shas.map((sha) => collectCommit(repoRoot, traceMap, sha));
  const skippedMerge = commits.filter((commit) => commit.skipReason === "merge_commit").length;
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
      skippedDocsOnlyCommits: skippedDocsOnly,
      noCommits: commits.length === 0,
    },
    commits,
  };
}

function markdownList(items, emptyText) {
  if (items.length === 0) {
    return `- ${emptyText}`;
  }
  return items.map((item) => `- ${item}`).join("\n");
}

export function renderMarkdown(report) {
  if (report.mode === "generate-docs") {
    return renderGenerateDocsMarkdown(report);
  }
  if (report.mode === "classify") {
    return renderClassifierMarkdown(report);
  }

  const lines = [
    "# Documentation Drift Sweep Dry Run",
    "",
    `Base: ${report.base.ref} (${report.base.sha.slice(0, 12)})`,
    `Head: ${report.head.ref} (${report.head.sha.slice(0, 12)})`,
    `Trace map: ${report.traceMap.path} (version ${report.traceMap.version ?? "unknown"}, ${report.traceMap.routeCount} routes)`,
    "",
    "## Summary",
    "",
    `- Total commits: ${report.summary.totalCommits}`,
    `- Considered commits: ${report.summary.consideredCommits}`,
    `- Skipped merge commits: ${report.summary.skippedMergeCommits}`,
    `- Skipped docs-only churn commits: ${report.summary.skippedDocsOnlyCommits}`,
    "",
  ];

  if (report.summary.noCommits) {
    lines.push("No commits to sweep between the checkpoint and head.", "");
    return `${lines.join("\n")}\n`;
  }

  const considered = report.commits.filter((commit) => commit.status === "considered");
  lines.push("## Considered Commits", "");
  if (considered.length === 0) {
    lines.push("No non-merge, non-docs-only commits need doc drift classification.", "");
  }
  for (const commit of considered) {
    lines.push(
      `### ${commit.shortSha} - ${commit.subject}`,
      "",
      `- Author date: ${commit.authorDate}`,
      `- Diff stat: ${commit.diffStat}`,
      `- Design docs touched: ${commit.docsTouched.anyDesign ? commit.docsTouched.design.join(", ") : "none"}`,
      `- Context docs touched: ${commit.docsTouched.anyContext ? commit.docsTouched.context.join(", ") : "none"}`,
      `- Trace-map candidate docs: ${commit.traceDocs.length > 0 ? commit.traceDocs.join(", ") : "none"}`,
      "",
      "Changed paths:",
      markdownList(commit.changedPaths, "none"),
    );
    if (commit.body) {
      lines.push("", "Commit body:", "", commit.body.split("\n").map((line) => `> ${line}`).join("\n"));
    }
    lines.push("");
  }

  const skipped = report.commits.filter((commit) => commit.status === "skipped");
  lines.push("## Skipped Commits", "");
  if (skipped.length === 0) {
    lines.push("No commits were skipped.", "");
  }
  for (const commit of skipped) {
    lines.push(`- ${commit.shortSha} ${commit.subject} (${commit.skipReason})`);
  }
  lines.push("");

  return `${lines.join("\n")}\n`;
}

function renderClassifierMarkdown(report) {
  const lines = [
    "# Documentation Drift Classifier Report",
    "",
    `Base: ${report.base.ref} (${report.base.sha.slice(0, 12)})`,
    `Head: ${report.head.ref} (${report.head.sha.slice(0, 12)})`,
    `Trace map: ${report.traceMap.path} (version ${report.traceMap.version ?? "unknown"}, ${report.traceMap.routeCount} routes)`,
    `Classifier prompt: ${report.classifier.promptVersion}`,
    `Classifier cache: ${report.classifier.cacheDir}`,
    "",
    "## Summary",
    "",
    `- Total commits: ${report.summary.totalCommits}`,
    `- Considered commits: ${report.summary.consideredCommits}`,
    `- Decisions: ${report.classifier.summary.totalDecisions}`,
    `- Move on: ${report.classifier.summary.moveOn}`,
    `- Update docs: ${report.classifier.summary.updateDocs}`,
    `- Cache hits: ${report.classifier.summary.cacheHits}`,
    `- Estimated prompt tokens: ${report.classifier.budget.estimatedPromptTokens}`,
    "",
  ];

  if (report.classifier.summary.totalDecisions === 0) {
    lines.push("No non-merge, non-docs-only commits need doc drift classification.", "");
    return `${lines.join("\n")}\n`;
  }

  lines.push("## Decisions", "");
  for (const decision of report.classifier.decisions) {
    lines.push(
      `### ${decision.shortSha} - ${decision.subject}`,
      "",
      `- Decision: ${decision.decision}`,
      `- Likely docs: ${decision.likelyDocs.length > 0 ? decision.likelyDocs.join(", ") : "none"}`,
      `- Evidence: ${decision.evidenceNote}`,
      `- Cache: ${decision.cache.hit ? "hit" : "miss"} (${decision.cache.path})`,
      `- Invocation mode: ${decision.codex.mode}`,
      "",
    );
  }

  const skipped = report.commits.filter((commit) => commit.status === "skipped");
  lines.push("## Skipped Commits", "");
  if (skipped.length === 0) {
    lines.push("No commits were skipped.", "");
  }
  for (const commit of skipped) {
    lines.push(`- ${commit.shortSha} ${commit.subject} (${commit.skipReason})`);
  }
  lines.push("");
  return `${lines.join("\n")}\n`;
}

function renderGenerateDocsMarkdown(report) {
  const lines = [
    "# Documentation Drift Generated Docs Report",
    "",
    `Base: ${report.base.ref} (${report.base.sha.slice(0, 12)})`,
    `Head: ${report.head.ref} (${report.head.sha.slice(0, 12)})`,
    `Trace map: ${report.traceMap.path} (version ${report.traceMap.version ?? "unknown"}, ${report.traceMap.routeCount} routes)`,
    `Classifier prompt: ${report.classifier.promptVersion}`,
    `Doc patch prompt: ${report.docPatch.promptVersion}`,
    `Classifier cache: ${report.classifier.cacheDir}`,
    `Doc patch cache: ${report.docPatch.cacheDir}`,
    "",
    "## Summary",
    "",
    `- Total commits: ${report.summary.totalCommits}`,
    `- Considered commits: ${report.summary.consideredCommits}`,
    `- Update-docs decisions: ${report.docPatch.summary.updateDocsDecisions}`,
    `- Patch records: ${report.docPatch.summary.patchRecords}`,
    `- Patches: ${report.docPatch.summary.patches}`,
    `- Applied patches: ${report.docPatch.summary.applied}`,
    `- Already applied patches: ${report.docPatch.summary.alreadyApplied}`,
    `- Doc patch cache hits: ${report.docPatch.summary.cacheHits}`,
    `- Estimated doc patch prompt tokens: ${report.docPatch.budget.estimatedPromptTokens}`,
    "",
  ];

  if (report.docPatch.summary.patchRecords === 0) {
    lines.push("No update_docs decisions produced documentation patches.", "");
    return `${lines.join("\n")}\n`;
  }

  lines.push("## Generated Patches", "");
  for (const record of report.docPatch.records) {
    lines.push(
      `### ${record.shortSha} - ${record.subject}`,
      "",
      `- Summary: ${record.summary}`,
      `- Evidence: ${record.decision.evidenceNote}`,
      `- Target docs: ${record.docTargets.length > 0 ? record.docTargets.join(", ") : "none"}`,
      `- Cache: ${record.cache.hit ? "hit" : "miss"} (${record.cache.path})`,
      `- Invocation mode: ${record.codex.mode}`,
      "",
    );
    if (record.applications.length === 0) {
      lines.push("Applications:", "- none", "");
    } else {
      lines.push("Applications:");
      for (const application of record.applications) {
        lines.push(`- ${application.path}: ${application.status} - ${application.rationale}`);
      }
      lines.push("");
    }
  }
  return `${lines.join("\n")}\n`;
}

function writeOutputs(report, outDir) {
  const absOutDir = path.resolve(outDir);
  mkdirSync(absOutDir, { recursive: true });
  const stem =
    report.mode === "generate-docs" ? "docdrift-generate" : report.mode === "classify" ? "docdrift-classify" : "docdrift-sweep";
  writeFileSync(path.join(absOutDir, `${stem}.json`), `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(path.join(absOutDir, `${stem}.md`), renderMarkdown(report));
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
    let report = buildReport(options);
    if (options.classify || options.generateDocs) {
      report = classifyReport(report, options);
    }
    if (options.generateDocs) {
      report = generateDocsReport(report, options);
    }
    if (options.outDir) {
      writeOutputs(report, options.outDir);
    }
    if (options.format === "json") {
      console.log(JSON.stringify(report, null, 2));
    } else {
      process.stdout.write(renderMarkdown(report));
    }
  } catch (error) {
    console.error(`docdrift sweep failed: ${error.message}`);
    process.exit(1);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}
