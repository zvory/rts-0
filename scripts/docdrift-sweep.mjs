#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const defaultRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const defaultCheckpointFile = "docs/docdrift-checkpoint.txt";
const defaultTraceMapPath = "docs/doc-map.json";

function usage() {
  console.log(`Usage:
  node scripts/docdrift-sweep.mjs --dry-run [options]

Options:
  --base REF                  Override the reviewed checkpoint ref.
  --checkpoint-file PATH      Checkpoint file used when --base is omitted. Default: ${defaultCheckpointFile}
  --checkpoint-ref REF        Optional checkpoint ref used when --base is omitted.
  --head REF                  Sweep target. Default: origin/main.
  --trace-map PATH            Trace map JSON path. Default: ${defaultTraceMapPath}
  --format markdown|json      Stdout format. Default: markdown.
  --out-dir DIR               Also write docdrift-sweep.md and docdrift-sweep.json.
  --repo DIR                  Repository root. Default: current RTS checkout.
  -h, --help                  Show this help.

Phase 1 is dry-run only. The command reads commit metadata and trace-map routing, but does not
call a model, edit docs, create PRs, or advance the checkpoint.`);
}

export function parseArgs(argv) {
  const options = {
    base: null,
    checkpointFile: defaultCheckpointFile,
    checkpointRef: null,
    dryRun: false,
    format: "markdown",
    head: "origin/main",
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
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "--base" || arg.startsWith("--base=")) {
      options.base = readValue("--base");
    } else if (arg === "--checkpoint-ref" || arg.startsWith("--checkpoint-ref=")) {
      options.checkpointRef = readValue("--checkpoint-ref");
    } else if (arg === "--checkpoint-file" || arg.startsWith("--checkpoint-file=")) {
      options.checkpointFile = readValue("--checkpoint-file");
    } else if (arg === "--head" || arg.startsWith("--head=")) {
      options.head = readValue("--head");
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
  if (!options.help && !options.dryRun) {
    throw new Error("phase 1 supports only --dry-run");
  }

  options.repoRoot = path.resolve(options.repoRoot);
  return options;
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

function writeOutputs(report, outDir) {
  const absOutDir = path.resolve(outDir);
  mkdirSync(absOutDir, { recursive: true });
  writeFileSync(path.join(absOutDir, "docdrift-sweep.json"), `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(path.join(absOutDir, "docdrift-sweep.md"), renderMarkdown(report));
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
    const report = buildReport(options);
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
