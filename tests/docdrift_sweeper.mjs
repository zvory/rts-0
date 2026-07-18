#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const script = path.join(repoRoot, "scripts", "docdrift-sweep.mjs");
const dailyScript = path.join(repoRoot, "scripts", "docdrift-daily.sh");
const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-docdrift-sweeper-"));
const repo = path.join(fixtureRoot, "repo");

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    cwd: options.cwd ?? repo,
    env: {
      ...process.env,
      GIT_AUTHOR_DATE: options.date ?? "2026-06-20T12:00:00Z",
      GIT_COMMITTER_DATE: options.date ?? "2026-06-20T12:00:00Z",
      ...(options.env ?? {}),
    },
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
  });
}

function git(args, options = {}) {
  return run("git", args, options).trim();
}

function writeRepoFile(name, text) {
  const absPath = path.join(repo, name);
  fs.mkdirSync(path.dirname(absPath), { recursive: true });
  fs.writeFileSync(absPath, text);
}

function commitFile(name, text, subject, body = "", options = {}) {
  writeRepoFile(name, text);
  git(["add", name]);
  const messageArgs = ["commit", "-m", subject];
  if (body) {
    messageArgs.push("-m", body);
  }
  git(messageArgs, options);
  return git(["rev-parse", "HEAD"]);
}

function sweep(args) {
  return run("node", [script, "--dry-run", "--repo", repo, ...args], {
    cwd: repoRoot,
  });
}

function classify(args) {
  return run("node", [script, "--classify", "--repo", repo, ...args], {
    cwd: repoRoot,
  });
}

function generateDocs(args) {
  return run("node", [script, "--generate-docs", "--repo", repo, ...args], {
    cwd: repoRoot,
  });
}

function fullSweep(args, options = {}) {
  return run("node", [script, "--full", "--repo", repo, ...args], {
    cwd: repoRoot,
    env: options.env,
  });
}

function fullSweepFailure(args, options = {}) {
  try {
    fullSweep(args, options);
    assert.fail("expected full sweep to fail");
  } catch (error) {
    return error.stderr.toString();
  }
}

function writeRunState(runId, state) {
  const dir = path.join(repo, ".docdrift", "runs", runId);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "run-state.json"),
    `${JSON.stringify({ schemaVersion: 1, runId, lifecycle: {}, updatedAt: "2026-07-17T12:00:00Z", ...state }, null, 2)}\n`,
  );
}

function classifyFailure(args) {
  try {
    classify(args);
    assert.fail("expected classifier command to fail");
  } catch (error) {
    return error.stderr.toString();
  }
}

function writeExecutable(name, text) {
  const absPath = path.join(fixtureRoot, name);
  fs.writeFileSync(absPath, text, { mode: 0o755 });
  return absPath;
}

function runDailyWrapper(fakeBin, args, options = {}) {
  const scriptPath = options.dailyScript ?? dailyScript;
  return execFileSync("bash", [scriptPath, ...args], {
    cwd: options.cwd ?? path.dirname(scriptPath),
    env: {
      ...process.env,
      PATH: `${fakeBin}${path.delimiter}${process.env.PATH}`,
      DOC_DRIFT_OBSERVABILITY_DIR: options.observabilityDir,
      DOC_DRIFT_MAX_COMMITS: options.maxCommits ?? "300",
      DOC_DRIFT_RUNNER_WORKTREE: options.runnerWorktree,
      FAKE_NODE_ARGS_FILE: options.argsFile,
      FAKE_NODE_EXIT: String(options.exitCode ?? 0),
      FAKE_NODE_STDOUT: options.stdout ?? "",
      FAKE_NODE_STDERR: options.stderr ?? "",
    },
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
  });
}

fs.mkdirSync(repo, { recursive: true });

try {
  git(["init", "--initial-branch=main"]);
  git(["config", "user.email", "agent@example.invalid"]);
  git(["config", "user.name", "Agent"]);

  writeRepoFile(
    "docs/doc-map.json",
    JSON.stringify(
      {
        version: 1,
        routes: [
          {
            source: ["server/crates/sim/src/game/**"],
            docs: ["docs/context/server-sim.md", "docs/design/server-sim.md", "docs/design/testing.md"],
            notes: "Sim changes route primarily to server sim docs, with a broad testing-doc trace candidate.",
          },
          {
            source: ["tests/**", "scripts/check-*.mjs"],
            docs: ["docs/context/testing.md", "docs/design/testing.md"],
            notes: "Test and check changes route to testing docs.",
          },
        ],
      },
      null,
      2,
    ) + "\n",
  );
  writeRepoFile("docs/context/server-sim.md", "server sim capsule\n");
  writeRepoFile("docs/design/server-sim.md", "server sim design\n");
  writeRepoFile("docs/context/testing.md", "testing capsule\n");
  writeRepoFile("docs/design/testing.md", "testing design\n");
  git(["add", "."]);
  git(["commit", "-m", "Initial docs"]);
  const base = git(["rev-parse", "HEAD"]);
  writeRepoFile(
    "docs/docdrift-checkpoint.txt",
    [
      "# Documentation drift sweeper checkpoint.",
      "# Updated only after a sweep PR merges.",
      base,
      "",
    ].join("\n"),
  );
  git(["add", "docs/docdrift-checkpoint.txt"]);
  git(["commit", "-m", "Add docdrift checkpoint"], { date: "2026-06-20T12:00:30Z" });
  const checkpointCommit = git(["rev-parse", "HEAD"]);

  const simCommit = commitFile(
    "server/crates/sim/src/game/mod.rs",
    "pub struct Game;\n",
    "Change sim API",
    "Adds a new public helper that may need docs.",
    { date: "2026-06-20T12:01:00Z" },
  );
  commitFile(
    "docs/design/testing.md",
    "testing design\nmore detail\n",
    "Clarify testing docs",
    "",
    { date: "2026-06-20T12:02:00Z" },
  );
  git(["commit", "--allow-empty", "-m", "Retrigger main test gate"], { date: "2026-06-20T12:02:30Z" });
  const emptyCommit = git(["rev-parse", "HEAD"]);
  git(["checkout", "-b", "feature-test-route"]);
  const testCommit = commitFile(
    "tests/server_integration.mjs",
    "console.log('test');\n",
    "Add integration coverage",
    "",
    { date: "2026-06-20T12:03:00Z" },
  );
  git(["checkout", "main"]);
  git(["merge", "--no-ff", "feature-test-route", "-m", "Merge feature test route"], {
    date: "2026-06-20T12:04:00Z",
  });
  const head = git(["rev-parse", "HEAD"]);

  const jsonReport = JSON.parse(sweep(["--base", checkpointCommit, "--head", head, "--format", "json"]));
  assert.equal(jsonReport.version, 1);
  assert.equal(jsonReport.mode, "dry-run");
  assert.equal(jsonReport.base.sha, checkpointCommit);
  assert.equal(jsonReport.head.sha, head);
  assert.equal(jsonReport.traceMap.path, "docs/doc-map.json");
  assert.deepEqual(jsonReport.summary, {
    totalCommits: 5,
    consideredCommits: 2,
    skippedMergeCommits: 1,
    skippedEmptyCommits: 1,
    skippedDocsOnlyCommits: 1,
    noCommits: false,
  });

  const simEntry = jsonReport.commits.find((commit) => commit.sha === simCommit);
  assert.ok(simEntry, "expected sim commit in report");
  assert.equal(simEntry.status, "considered");
  assert.equal(simEntry.body, "Adds a new public helper that may need docs.");
  assert.deepEqual(simEntry.traceDocs, ["docs/context/server-sim.md", "docs/design/server-sim.md", "docs/design/testing.md"]);
  assert.equal(simEntry.docsTouched.anyDesign, false);

  const docsOnlyEntry = jsonReport.commits.find((commit) => commit.subject === "Clarify testing docs");
  assert.ok(docsOnlyEntry, "expected docs-only commit in report");
  assert.equal(docsOnlyEntry.status, "skipped");
  assert.equal(docsOnlyEntry.skipReason, "docs_only_churn");
  assert.equal(docsOnlyEntry.docsTouched.anyDesign, true);

  const emptyEntry = jsonReport.commits.find((commit) => commit.sha === emptyCommit);
  assert.ok(emptyEntry, "expected empty commit in report");
  assert.equal(emptyEntry.status, "skipped");
  assert.equal(emptyEntry.skipReason, "empty_commit");

  const testEntry = jsonReport.commits.find((commit) => commit.sha === testCommit);
  assert.ok(testEntry, "expected test commit in report");
  assert.deepEqual(testEntry.traceDocs, ["docs/context/testing.md", "docs/design/testing.md"]);

  const mergeEntry = jsonReport.commits.find((commit) => commit.skipReason === "merge_commit");
  assert.ok(mergeEntry, "expected merge commit skip");
  assert.equal(mergeEntry.parentCount, 2);

  const markdown = sweep(["--base", checkpointCommit, "--head", head]);
  assert.match(markdown, /# Documentation Drift Sweep Dry Run/);
  assert.match(markdown, /Considered commits: 2/);
  assert.match(markdown, /Change sim API/);
  assert.match(markdown, /docs\/design\/server-sim\.md/);
  assert.match(markdown, /docs_only_churn/);
  assert.match(markdown, /empty_commit/);
  assert.match(markdown, /merge_commit/);

  const outDir = path.join(fixtureRoot, "out");
  sweep(["--base", checkpointCommit, "--head", head, "--out-dir", outDir]);
  assert.ok(fs.existsSync(path.join(outDir, "docdrift-sweep.md")));
  assert.ok(fs.existsSync(path.join(outDir, "docdrift-sweep.json")));
  assert.equal(JSON.parse(fs.readFileSync(path.join(outDir, "docdrift-sweep.json"), "utf8")).head.sha, head);

  const classifierCache = ".docdrift/test-classifier-cache";
  const classifierJson = JSON.parse(
    classify([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(classifierJson.mode, "classify");
  assert.equal(classifierJson.classifier.promptVersion, "docdrift-classifier-v1");
  assert.equal(classifierJson.classifier.noCodex, true);
  assert.equal(classifierJson.classifier.fixture, "tests/fixtures/docdrift/classifier-basic.json");
  assert.equal(classifierJson.classifier.summary.totalDecisions, 2);
  assert.equal(classifierJson.classifier.summary.updateDocs, 1);
  assert.equal(classifierJson.classifier.summary.moveOn, 1);
  assert.equal(classifierJson.classifier.summary.cacheHits, 0);

  const simDecision = classifierJson.classifier.decisions.find((decision) => decision.commitSha === simCommit);
  assert.ok(simDecision, "expected sim classifier decision");
  assert.equal(simDecision.decision, "update_docs");
  assert.deepEqual(simDecision.likelyDocs, ["docs/design/server-sim.md"]);
  assert.equal(simDecision.codex.mode, "fixture");
  assert.equal(simDecision.cache.hit, false);

  const testDecision = classifierJson.classifier.decisions.find((decision) => decision.commitSha === testCommit);
  assert.ok(testDecision, "expected test classifier decision");
  assert.equal(testDecision.decision, "move_on");
  assert.deepEqual(testDecision.likelyDocs, ["docs/design/testing.md"]);

  const cachedClassifierJson = JSON.parse(
    classify([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(cachedClassifierJson.classifier.summary.cacheHits, 2);
  assert.ok(cachedClassifierJson.classifier.decisions.every((decision) => decision.cache.hit));

  const classifierOutDir = path.join(fixtureRoot, "classify-out");
  classify([
    "--base",
    checkpointCommit,
    "--head",
    head,
    "--no-codex",
    "--fixture",
    "classifier-basic",
    "--classifier-cache",
    ".docdrift/out-cache",
    "--out-dir",
    classifierOutDir,
  ]);
  assert.ok(fs.existsSync(path.join(classifierOutDir, "docdrift-classify.md")));
  assert.ok(fs.existsSync(path.join(classifierOutDir, "docdrift-classify.json")));

  const docPatchCache = ".docdrift/test-doc-patch-cache";
  const generateJson = JSON.parse(
    generateDocs([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--doc-patch-cache",
      docPatchCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(generateJson.mode, "generate-docs");
  assert.equal(generateJson.docPatch.promptVersion, "docdrift-doc-patch-v1");
  assert.equal(generateJson.docPatch.noCodex, true);
  assert.equal(generateJson.docPatch.fixture, "tests/fixtures/docdrift/classifier-basic.json");
  assert.equal(generateJson.docPatch.summary.updateDocsDecisions, 1);
  assert.equal(generateJson.docPatch.summary.patches, 1);
  assert.equal(generateJson.docPatch.summary.applied, 1);
  assert.deepEqual(generateJson.docPatch.records[0].docTargets, ["docs/design/server-sim.md"]);
  assert.equal(generateJson.docPatch.records[0].docTargetSource, "classifier_likely_docs");
  assert.match(
    fs.readFileSync(path.join(repo, "docs/design/server-sim.md"), "utf8"),
    /Game API helper changes should be reflected here/,
  );

  const idempotentGenerateJson = JSON.parse(
    generateDocs([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--classifier-cache",
      classifierCache,
      "--doc-patch-cache",
      docPatchCache,
      "--format",
      "json",
    ]),
  );
  assert.equal(idempotentGenerateJson.docPatch.summary.applied, 0);
  assert.equal(idempotentGenerateJson.docPatch.summary.alreadyApplied, 1);
  assert.equal(idempotentGenerateJson.docPatch.summary.cacheHits, 1);
  assert.equal(idempotentGenerateJson.docPatch.budget.estimatedPromptTokens, 0);
  assert.equal(idempotentGenerateJson.docPatch.records[0].cache.reason, "already_applied_patch");

  const sequentialRepo = path.join(fixtureRoot, "sequential-repo");
  fs.mkdirSync(sequentialRepo, { recursive: true });
  const seqGit = (args, options = {}) => run("git", args, { cwd: sequentialRepo, ...options }).trim();
  const seqWrite = (name, text) => {
    const absPath = path.join(sequentialRepo, name);
    fs.mkdirSync(path.dirname(absPath), { recursive: true });
    fs.writeFileSync(absPath, text);
  };
  const seqCommit = (name, text, subject, options = {}) => {
    seqWrite(name, text);
    seqGit(["add", name]);
    seqGit(["commit", "-m", subject], options);
    return seqGit(["rev-parse", "HEAD"]);
  };
  run("git", ["init", "--initial-branch=main"], { cwd: sequentialRepo });
  seqGit(["config", "user.email", "agent@example.invalid"]);
  seqGit(["config", "user.name", "Agent"]);
  seqWrite(
    "docs/doc-map.json",
    JSON.stringify(
      {
        version: 1,
        routes: [
          {
            source: ["server/crates/sim/src/game/**"],
            docs: ["docs/design/server-sim.md"],
          },
        ],
      },
      null,
      2,
    ) + "\n",
  );
  seqWrite("docs/design/server-sim.md", "server sim design\n");
  seqGit(["add", "."]);
  seqGit(["commit", "-m", "Initial docs"]);
  const sequentialBase = seqGit(["rev-parse", "HEAD"]);
  seqCommit(
    "server/crates/sim/src/game/mod.rs",
    "pub struct Game;\n",
    "Change sim API",
    { date: "2026-06-20T12:10:00Z" },
  );
  seqCommit(
    "server/crates/sim/src/game/mod.rs",
    "pub struct Game;\npub fn helper() {}\n",
    "Refine sim API",
    { date: "2026-06-20T12:11:00Z" },
  );
  const sequentialHead = seqGit(["rev-parse", "HEAD"]);
  const sequentialFixture = path.join(fixtureRoot, "classifier-sequential.json");
  fs.writeFileSync(
    sequentialFixture,
    JSON.stringify(
      {
        decisions: [
          {
            subjectIncludes: "Change sim API",
            decision: "update_docs",
            likelyDocs: ["docs/design/server-sim.md"],
            evidenceNote: "First sim API commit needs server sim docs.",
          },
          {
            subjectIncludes: "Refine sim API",
            decision: "update_docs",
            likelyDocs: ["docs/design/server-sim.md"],
            evidenceNote: "Second sim API commit needs the same server sim docs.",
          },
        ],
        default: {
          decision: "move_on",
          likelyDocs: [],
          evidenceNote: "No docs update needed.",
        },
      },
      null,
      2,
    ) + "\n",
  );
  const sequentialClassifierCache = ".docdrift/sequential-classifier-cache";
  classify([
    "--repo",
    sequentialRepo,
    "--base",
    sequentialBase,
    "--head",
    sequentialHead,
    "--no-codex",
    "--fixture",
    sequentialFixture,
    "--classifier-cache",
    sequentialClassifierCache,
    "--format",
    "json",
  ]);
  const fakeSequentialCodex = writeExecutable(
    "fake-sequential-doc-codex",
    [
      "#!/usr/bin/env node",
      "const fs = require('node:fs');",
      "const args = process.argv.slice(2);",
      "const outputFlag = args.indexOf('--output-last-message');",
      "if (outputFlag < 0 || outputFlag + 1 >= args.length) process.exit(2);",
      "const prompt = fs.readFileSync(0, 'utf8');",
      "const response = prompt.includes('First generated detail.')",
      "  ? { summary: 'Add second generated detail.', patches: [{ path: 'docs/design/server-sim.md', find: 'First generated detail.\\n', replace: 'First generated detail.\\nSecond generated detail.\\n', rationale: 'The second prompt saw the first generated detail.' }] }",
      "  : { summary: 'Add first generated detail.', patches: [{ path: 'docs/design/server-sim.md', find: 'server sim design\\n', replace: 'server sim design\\n\\nFirst generated detail.\\n', rationale: 'The first prompt adds the initial detail.' }] };",
      "fs.writeFileSync(args[outputFlag + 1], JSON.stringify(response));",
      "console.log(JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 5, output_tokens: 4, total_tokens: 9 } }));",
      "",
    ].join("\n"),
  );
  const sequentialGenerate = JSON.parse(
    generateDocs([
      "--repo",
      sequentialRepo,
      "--base",
      sequentialBase,
      "--head",
      sequentialHead,
      "--codex-command",
      fakeSequentialCodex,
      "--classifier-cache",
      sequentialClassifierCache,
      "--doc-patch-cache",
      ".docdrift/sequential-doc-patch-cache",
      "--format",
      "json",
    ]),
  );
  assert.equal(sequentialGenerate.classifier.summary.cacheHits, 2);
  assert.equal(sequentialGenerate.docPatch.summary.applied, 2);
  assert.equal(sequentialGenerate.docPatch.summary.alreadyApplied, 0);
  assert.equal(sequentialGenerate.docPatch.records[1].summary, "Add second generated detail.");
  const sequentialDoc = fs.readFileSync(path.join(sequentialRepo, "docs/design/server-sim.md"), "utf8");
  assert.match(sequentialDoc, /First generated detail/);
  assert.match(sequentialDoc, /Second generated detail/);

  const partialRepo = path.join(fixtureRoot, "partial-repo");
  fs.mkdirSync(partialRepo, { recursive: true });
  const partialGit = (args, options = {}) => run("git", args, { cwd: partialRepo, ...options }).trim();
  const partialWrite = (name, text) => {
    const absPath = path.join(partialRepo, name);
    fs.mkdirSync(path.dirname(absPath), { recursive: true });
    fs.writeFileSync(absPath, text);
  };
  const partialCommit = (name, text, subject, options = {}) => {
    partialWrite(name, text);
    partialGit(["add", name]);
    partialGit(["commit", "-m", subject], options);
    return partialGit(["rev-parse", "HEAD"]);
  };
  run("git", ["init", "--initial-branch=main"], { cwd: partialRepo });
  partialGit(["config", "user.email", "agent@example.invalid"]);
  partialGit(["config", "user.name", "Agent"]);
  partialWrite(
    "docs/doc-map.json",
    JSON.stringify(
      {
        version: 1,
        routes: [
          {
            source: ["client/src/**"],
            docs: ["docs/design/client-ui.md"],
          },
        ],
      },
      null,
      2,
    ) + "\n",
  );
  partialWrite("docs/design/client-ui.md", "client ui design\n");
  partialGit(["add", "."]);
  partialGit(["commit", "-m", "Initial docs"]);
  const partialBase = partialGit(["rev-parse", "HEAD"]);
  partialCommit("client/src/view.js", "export const first = true;\n", "Add first UI behavior", {
    date: "2026-06-20T12:20:00Z",
  });
  const partialSecondCommit = partialCommit(
    "client/src/view.js",
    "export const first = true;\nexport const second = true;\n",
    "Add second UI behavior",
    { date: "2026-06-20T12:21:00Z" },
  );
  const partialThirdCommit = partialCommit(
    "client/src/view.js",
    "export const first = true;\nexport const second = true;\nexport const third = true;\n",
    "Add third UI behavior",
    { date: "2026-06-20T12:22:00Z" },
  );
  const partialHead = partialGit(["rev-parse", "HEAD"]);
  const partialFixture = path.join(fixtureRoot, "classifier-partial.json");
  fs.writeFileSync(
    partialFixture,
    JSON.stringify(
      {
        decisions: [
          {
            subjectIncludes: "Add first UI behavior",
            decision: "update_docs",
            likelyDocs: ["docs/design/client-ui.md"],
            evidenceNote: "First UI behavior needs client docs.",
          },
          {
            subjectIncludes: "Add second UI behavior",
            decision: "update_docs",
            likelyDocs: ["docs/design/client-ui.md"],
            evidenceNote: "Second UI behavior needs client docs.",
          },
          {
            subjectIncludes: "Add third UI behavior",
            decision: "update_docs",
            likelyDocs: ["docs/design/client-ui.md"],
            evidenceNote: "Third UI behavior needs client docs.",
          },
        ],
        docPatches: [
          {
            subjectIncludes: "Add first UI behavior",
            summary: "Add first UI behavior.",
            patches: [
              {
                path: "docs/design/client-ui.md",
                find: "client ui design\n",
                replace: "client ui design\n\nFirst UI behavior detail.\n",
                rationale: "The first UI behavior changed.",
              },
            ],
          },
          {
            subjectIncludes: "Add second UI behavior",
            summary: "Add second UI behavior.",
            patches: [
              {
                path: "docs/design/client-ui.md",
                find: "missing generated context\n",
                replace: "missing generated context\nSecond UI behavior detail.\n",
                rationale: "The second UI behavior changed.",
              },
            ],
          },
          {
            subjectIncludes: "Add third UI behavior",
            summary: "Add third UI behavior.",
            patches: [
              {
                path: "docs/design/client-ui.md",
                find: "First UI behavior detail.\n",
                replace: "First UI behavior detail.\nThird UI behavior detail.\n",
                rationale: "The third UI behavior changed.",
              },
            ],
          },
        ],
        default: {
          decision: "move_on",
          likelyDocs: [],
          evidenceNote: "No docs update needed.",
        },
        defaultDocPatch: {
          summary: "No patch.",
          patches: [],
        },
      },
      null,
      2,
    ) + "\n",
  );
  const partialGenerate = JSON.parse(
    generateDocs([
      "--repo",
      partialRepo,
      "--base",
      partialBase,
      "--head",
      partialHead,
      "--no-codex",
      "--fixture",
      partialFixture,
      "--classifier-cache",
      ".docdrift/partial-classifier-cache",
      "--doc-patch-cache",
      ".docdrift/partial-doc-patch-cache",
      "--format",
      "json",
    ]),
  );
  assert.equal(partialGenerate.docPatch.partial, false);
  assert.equal(partialGenerate.docPatch.summary.failed, false);
  assert.equal(partialGenerate.docPatch.summary.skipped, 1);
  assert.equal(partialGenerate.docPatch.summary.patchRecords, 2);
  assert.equal(partialGenerate.docPatch.summary.applied, 2);
  assert.equal(partialGenerate.docPatch.skipped.length, 1);
  assert.equal(partialGenerate.docPatch.skipped[0].index, 2);
  assert.equal(partialGenerate.docPatch.skipped[0].commitSha, partialSecondCommit);
  assert.equal(partialGenerate.docPatch.skipped[0].kind, "apply_error");
  assert.match(partialGenerate.docPatch.skipped[0].message, /doc patch find text not found/);
  assert.equal(partialGenerate.docPatch.records[1].commitSha, partialThirdCommit);
  const partialDoc = fs.readFileSync(path.join(partialRepo, "docs/design/client-ui.md"), "utf8");
  assert.match(partialDoc, /First UI behavior detail/);
  assert.doesNotMatch(partialDoc, /Second UI behavior detail/);
  assert.match(partialDoc, /Third UI behavior detail/);

  const partialOutDir = path.join(fixtureRoot, "partial-skip-out");
  generateDocs([
    "--repo",
    partialRepo,
    "--base",
    partialBase,
    "--head",
    partialHead,
    "--no-codex",
    "--fixture",
    partialFixture,
    "--classifier-cache",
    ".docdrift/partial-classifier-cache",
    "--doc-patch-cache",
    ".docdrift/partial-doc-patch-cache",
    "--out-dir",
    partialOutDir,
  ]);
  assert.match(fs.readFileSync(path.join(partialOutDir, "docdrift-generate.md"), "utf8"), /Skipped Decisions/);

  const fakeCodexArgsPath = path.join(fixtureRoot, "fake-codex-args.json");
  const fakeCodex = writeExecutable(
    "fake-codex",
    [
      "#!/usr/bin/env node",
      "const fs = require('node:fs');",
      "const args = process.argv.slice(2);",
      `fs.writeFileSync(${JSON.stringify(fakeCodexArgsPath)}, JSON.stringify(args, null, 2));`,
      "const outputFlag = args.indexOf('--output-last-message');",
      "if (outputFlag < 0 || outputFlag + 1 >= args.length) process.exit(2);",
      "if (args.includes('--ask-for-approval')) process.exit(3);",
      "if (!args.includes('--json')) process.exit(4);",
      "if (!args.includes('-c') || !args.includes('approval_policy=\"never\"')) process.exit(5);",
      "fs.writeFileSync(args[outputFlag + 1], JSON.stringify({ decision: 'move_on', likelyDocs: [], evidenceNote: 'Fake Codex moved this commit on.' }));",
      "console.log(JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 11, output_tokens: 7, total_tokens: 18 } }));",
      "",
    ].join("\n"),
  );
  const liveShapeJson = JSON.parse(
    classify([
      "--base",
      checkpointCommit,
      "--head",
      simCommit,
      "--codex-command",
      fakeCodex,
      "--classifier-cache",
      ".docdrift/fake-codex-cache",
      "--format",
      "json",
    ]),
  );
  assert.equal(liveShapeJson.classifier.summary.totalDecisions, 1);
  assert.equal(liveShapeJson.classifier.decisions[0].codex.mode, "codex_cli");
  assert.deepEqual(liveShapeJson.classifier.decisions[0].codex.usage, {
    inputTokens: 11,
    cachedInputTokens: null,
    outputTokens: 7,
    reasoningTokens: null,
    totalTokens: 18,
  });
  const fakeArgs = JSON.parse(fs.readFileSync(fakeCodexArgsPath, "utf8"));
  assert.ok(!fakeArgs.includes("--ask-for-approval"));
  assert.ok(fakeArgs.includes("--json"));
  assert.ok(fakeArgs.includes("-c"));
  assert.ok(fakeArgs.includes('approval_policy="never"'));

  const slowCodex = writeExecutable(
    "slow-codex",
    [
      "#!/usr/bin/env node",
      "setTimeout(() => {}, 5000);",
      "",
    ].join("\n"),
  );
  assert.match(
    classifyFailure([
      "--base",
      checkpointCommit,
      "--head",
      simCommit,
      "--codex-command",
      slowCodex,
      "--codex-timeout-seconds",
      "1",
      "--classifier-cache",
      ".docdrift/slow-codex-cache",
    ]),
    /Codex classifier timed out after 1s/,
  );

  const generateOutDir = path.join(fixtureRoot, "generate-out");
  generateDocs([
    "--base",
    checkpointCommit,
    "--head",
    head,
    "--no-codex",
    "--fixture",
    "classifier-basic",
    "--classifier-cache",
    classifierCache,
    "--doc-patch-cache",
    docPatchCache,
    "--out-dir",
    generateOutDir,
  ]);
  assert.ok(fs.existsSync(path.join(generateOutDir, "docdrift-generate.md")));
  assert.ok(fs.existsSync(path.join(generateOutDir, "docdrift-generate.json")));

  assert.match(
    classifyFailure([
      "--base",
      checkpointCommit,
      "--head",
      head,
      "--no-codex",
      "--fixture",
      "classifier-basic",
      "--max-commits",
      "1",
    ]),
    /classify budget exceeded: 2 considered commits exceeds --max-commits 1/,
  );

  const checkpointReport = JSON.parse(sweep(["--head", head, "--format", "json"]));
  assert.equal(checkpointReport.base.ref, base);
  assert.equal(checkpointReport.base.source, "docs/docdrift-checkpoint.txt");
  assert.equal(checkpointReport.summary.totalCommits, 6);
  assert.equal(checkpointReport.summary.consideredCommits, 2);

  const emptyReport = JSON.parse(sweep(["--base", head, "--head", head, "--format", "json"]));
  assert.equal(emptyReport.summary.noCommits, true);
  assert.match(sweep(["--base", head, "--head", head]), /No commits to sweep/);

  const fullPreviewOutDir = path.join(fixtureRoot, "full-preview");
  const fullPreview = JSON.parse(
    fullSweep([
      "--dry-run",
      "--base",
      head,
      "--head",
      head,
      "--out-dir",
      fullPreviewOutDir,
      "--run-id",
      "preview",
      "--format",
      "json",
    ]),
  );
  assert.equal(fullPreview.mode, "full");
  assert.equal(fullPreview.dryRun, true);
  assert.equal(fullPreview.sweep.action, "noop_no_commits");
  assert.equal(fullPreview.checkpoint.advanced, false);
  const openPrStep = fullPreview.lifecycle.find((step) => step.name === "open or update owned PR");
  assert.ok(openPrStep);
  assert.match(openPrStep.command, /--label docdrift-sweep/);
  assert.ok(fullPreview.lifecycle.some((step) => step.command.includes("scripts/wait-pr.sh")));
  assert.ok(fs.existsSync(path.join(fullPreviewOutDir, "docdrift-full.json")));
  assert.ok(fs.existsSync(path.join(fullPreviewOutDir, "docdrift-full.md")));

  const bareOrigin = path.join(fixtureRoot, "origin.git");
  run("git", ["init", "--bare", bareOrigin], { cwd: fixtureRoot });
  git(["remote", "add", "origin", bareOrigin]);
  git(["push", "-u", "origin", "main"]);
  const docsOnlyHead = commitFile(
    "docs/design/testing.md",
    "testing design\nmore detail\ncheckpoint-only cleanup\n",
    "Refresh testing docs again",
    "",
    { date: "2026-06-20T12:05:00Z" },
  );
  git(["push", "origin", "main"]);

  const fullNoopOutDir = path.join(fixtureRoot, "full-noop");
  const fullNoop = JSON.parse(
    fullSweep([
      "--base",
      head,
      "--head",
      "origin/main",
      "--out-dir",
      fullNoopOutDir,
      "--run-id",
      "noop",
      "--format",
      "json",
    ]),
  );
  assert.equal(fullNoop.mode, "full");
  assert.equal(fullNoop.dryRun, false);
  assert.equal(fullNoop.sweep.action, "noop_no_considered_commits");
  assert.equal(fullNoop.sweep.prNumber, null);
  assert.equal(fullNoop.checkpoint.advanced, true);
  assert.equal(fullNoop.checkpoint.after.sha, docsOnlyHead);
  assert.match(fs.readFileSync(path.join(repo, ".docdrift/checkpoint.txt"), "utf8"), new RegExp(docsOnlyHead));
  assert.ok(fs.existsSync(path.join(fullNoopOutDir, "docdrift-full.json")));

  const fakeWait = writeExecutable("fake-wait-pr", "#!/usr/bin/env bash\nset -euo pipefail\ngit push origin HEAD:main >/dev/null\n");
  const fakeAgentPr = writeExecutable(
    "fake-agent-pr",
    "#!/usr/bin/env bash\nset -euo pipefail\nprintf 'agent-pr: PR 900 ready: https://example.invalid/pr/900\\n'\n",
  );
  const writeFakeGh = (name, prs) =>
    writeExecutable(name, `#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' '${JSON.stringify(prs)}'\n`);
  const recoveryEnv = (ghCommand) => ({
    DOC_DRIFT_GH_COMMAND: ghCommand,
    DOC_DRIFT_AGENT_PR_COMMAND: fakeAgentPr,
    DOC_DRIFT_WAIT_PR_COMMAND: fakeWait,
  });
  const createRecordedBranch = (runId, headSha, options = {}) => {
    const branch = `zvorygin/docdrift-sweep-${runId}`;
    const worktree = `.docdrift/worktrees/docdrift-sweep-${runId}`;
    git(["branch", branch, headSha]);
    if (options.remote !== false) git(["push", "origin", branch]);
    if (options.worktree !== false) git(["worktree", "add", path.join(repo, worktree), branch]);
    if (options.local === false) git(["branch", "-D", branch]);
    writeRunState(runId, {
      status: "pr_open",
      baseSha: headSha,
      headSha,
      branch,
      worktree,
      generatedHeadSha: headSha,
      pr: { number: options.prNumber ?? 700, url: "https://example.invalid/pr/700", state: "OPEN", headSha },
      checkpointTarget: headSha,
      recoveryAction: "created_fresh_run",
    });
    return { branch, worktree };
  };

  const initializedRunId = "initialized-before-branch";
  const initializedBranch = `zvorygin/docdrift-sweep-${initializedRunId}`;
  const initializedWorktree = `.docdrift/worktrees/docdrift-sweep-${initializedRunId}`;
  writeRunState(initializedRunId, {
    status: "initialized",
    baseSha: simCommit,
    headSha: docsOnlyHead,
    branch: initializedBranch,
    worktree: initializedWorktree,
    generatedHeadSha: null,
    pr: null,
    checkpointTarget: null,
    recoveryAction: "created_fresh_run",
  });
  const initializedGh = writeFakeGh("fake-gh-initialized", []);
  const initializedReport = JSON.parse(
    fullSweep(
      ["--run-id", initializedRunId, "--no-codex", "--fixture", "classifier-basic", "--format", "json"],
      { env: recoveryEnv(initializedGh) },
    ),
  );
  assert.equal(initializedReport.sweep.recoveryAction, "resumed_recorded_pre_pr_run");
  assert.equal(initializedReport.sweep.action, "noop_no_doc_changes");
  assert.equal(git(["rev-parse", initializedBranch]), docsOnlyHead);
  assert.ok(fs.existsSync(path.join(repo, initializedWorktree)));

  const openRun = createRecordedBranch("open-run", docsOnlyHead);
  const openGh = writeFakeGh("fake-gh-open", [
    { number: 700, url: "https://example.invalid/pr/700", state: "OPEN", mergeStateStatus: "CLEAN", headRefOid: docsOnlyHead, headRefName: openRun.branch, mergedAt: null },
  ]);
  const openReport = JSON.parse(fullSweep(["--run-id", "open-run", "--format", "json"], { env: recoveryEnv(openGh) }));
  assert.equal(openReport.sweep.action, "resumed_open_pr_merged");
  assert.equal(openReport.sweep.recoveryAction, "resumed_open_pr");
  assert.equal(git(["rev-parse", openRun.branch]), docsOnlyHead);

  const remoteOnlyRun = createRecordedBranch("remote-only", docsOnlyHead, { worktree: false, local: false, prNumber: 701 });
  const remoteOnlyGh = writeFakeGh("fake-gh-remote-only", [
    { number: 701, url: "https://example.invalid/pr/701", state: "OPEN", mergeStateStatus: "CLEAN", headRefOid: docsOnlyHead, headRefName: remoteOnlyRun.branch, mergedAt: null },
  ]);
  JSON.parse(fullSweep(["--run-id", "remote-only", "--format", "json"], { env: recoveryEnv(remoteOnlyGh) }));
  assert.equal(git(["rev-parse", remoteOnlyRun.branch]), docsOnlyHead);
  assert.ok(fs.existsSync(path.join(repo, remoteOnlyRun.worktree)));

  const localOnlyRun = createRecordedBranch("local-only", docsOnlyHead, { remote: false, worktree: false, prNumber: 708 });
  const localOnlyGh = writeFakeGh("fake-gh-local-only", [
    { number: 708, url: "https://example.invalid/pr/708", state: "OPEN", mergeStateStatus: "CLEAN", headRefOid: docsOnlyHead, headRefName: localOnlyRun.branch, mergedAt: null },
  ]);
  JSON.parse(fullSweep(["--run-id", "local-only", "--format", "json"], { env: recoveryEnv(localOnlyGh) }));
  assert.equal(git(["rev-parse", localOnlyRun.branch]), docsOnlyHead);
  assert.ok(fs.existsSync(path.join(repo, localOnlyRun.worktree)));

  const conflictedRun = createRecordedBranch("conflicted", docsOnlyHead, { prNumber: 702 });
  const conflictedGh = writeFakeGh("fake-gh-conflicted", [
    { number: 702, url: "https://example.invalid/pr/702", state: "OPEN", mergeStateStatus: "DIRTY", headRefOid: docsOnlyHead, headRefName: conflictedRun.branch, mergedAt: null },
  ]);
  assert.match(
    fullSweepFailure(["--run-id", "conflicted"], { env: recoveryEnv(conflictedGh) }),
    /open PR is conflicted.*mergeStateStatus=DIRTY/,
  );
  assert.equal(git(["rev-parse", conflictedRun.branch]), docsOnlyHead);

  const ambiguousRun = createRecordedBranch("ambiguous", docsOnlyHead, { prNumber: 703 });
  const ambiguousGh = writeFakeGh("fake-gh-ambiguous", [
    { number: 703, url: "https://example.invalid/pr/703", state: "OPEN", mergeStateStatus: "CLEAN", headRefOid: docsOnlyHead, headRefName: ambiguousRun.branch, mergedAt: null },
    { number: 704, url: "https://example.invalid/pr/704", state: "CLOSED", mergeStateStatus: "DIRTY", headRefOid: docsOnlyHead, headRefName: ambiguousRun.branch, mergedAt: null },
  ]);
  assert.match(fullSweepFailure(["--run-id", "ambiguous"], { env: recoveryEnv(ambiguousGh) }), /ambiguous PR matches/);
  assert.equal(git(["rev-parse", ambiguousRun.branch]), docsOnlyHead);

  const dirtyRun = createRecordedBranch("dirty", docsOnlyHead, { prNumber: 705 });
  fs.writeFileSync(path.join(repo, dirtyRun.worktree, "dirty.txt"), "do not touch\n");
  const dirtyGh = writeFakeGh("fake-gh-dirty", [
    { number: 705, url: "https://example.invalid/pr/705", state: "OPEN", mergeStateStatus: "CLEAN", headRefOid: docsOnlyHead, headRefName: dirtyRun.branch, mergedAt: null },
  ]);
  assert.match(fullSweepFailure(["--run-id", "dirty"], { env: recoveryEnv(dirtyGh) }), /unsafe sweep worktree.*dirty.txt/);
  assert.equal(fs.readFileSync(path.join(repo, dirtyRun.worktree, "dirty.txt"), "utf8"), "do not touch\n");
  const dirtyState = JSON.parse(fs.readFileSync(path.join(repo, ".docdrift/runs/dirty/run-state.json"), "utf8"));
  assert.equal(dirtyState.lifecycle["fetch origin/main"], "completed");
  assert.equal(dirtyState.recoveryAction, "stopped_for_operator_review");

  const closedUpdateRepo = path.join(fixtureRoot, "closed-update");
  run("git", ["clone", "--branch", "main", bareOrigin, closedUpdateRepo], { cwd: fixtureRoot });
  run("git", ["config", "user.email", "agent@example.invalid"], { cwd: closedUpdateRepo });
  run("git", ["config", "user.name", "Agent"], { cwd: closedUpdateRepo });
  fs.mkdirSync(path.join(closedUpdateRepo, "server/crates/sim/src/game"), { recursive: true });
  fs.writeFileSync(path.join(closedUpdateRepo, "server/crates/sim/src/game/recovery.rs"), "pub fn recovery_probe() {}\n");
  run("git", ["add", "server/crates/sim/src/game/recovery.rs"], { cwd: closedUpdateRepo });
  run("git", ["commit", "-m", "Change sim API"], { cwd: closedUpdateRepo });
  run("git", ["push", "origin", "main"], { cwd: closedUpdateRepo });
  const closedRun = createRecordedBranch("closed-run", docsOnlyHead, { prNumber: 706 });
  const closedGh = writeFakeGh("fake-gh-closed", [
    { number: 706, url: "https://example.invalid/pr/706", state: "CLOSED", mergeStateStatus: "DIRTY", headRefOid: docsOnlyHead, headRefName: closedRun.branch, mergedAt: null },
  ]);
  const closedReport = JSON.parse(fullSweep(["--run-id", "closed-run", "--no-codex", "--fixture", "classifier-basic", "--format", "json"], { env: recoveryEnv(closedGh) }));
  assert.equal(closedReport.sweep.recoveryAction, "created_fresh_run_after_closed_unmerged");
  assert.notEqual(closedReport.sweep.branch, closedRun.branch);
  assert.equal(git(["rev-parse", closedRun.branch]), docsOnlyHead);

  const recoveryUpdateRepo = path.join(fixtureRoot, "recovery-update");
  run("git", ["clone", "--branch", "main", bareOrigin, recoveryUpdateRepo], { cwd: fixtureRoot });
  run("git", ["config", "user.email", "agent@example.invalid"], { cwd: recoveryUpdateRepo });
  run("git", ["config", "user.name", "Agent"], { cwd: recoveryUpdateRepo });
  fs.mkdirSync(path.join(recoveryUpdateRepo, "tests"), { recursive: true });
  fs.writeFileSync(path.join(recoveryUpdateRepo, "tests/recovery_contract.mjs"), "console.log('recovery');\n");
  run("git", ["add", "tests/recovery_contract.mjs"], { cwd: recoveryUpdateRepo });
  run("git", ["commit", "-m", "Add recovery integration coverage"], { cwd: recoveryUpdateRepo });
  run("git", ["push", "origin", "main"], { cwd: recoveryUpdateRepo });
  const mergedRun = createRecordedBranch("merged-run", docsOnlyHead, { prNumber: 707 });
  const mergedGh = writeFakeGh("fake-gh-merged", [
    { number: 707, url: "https://example.invalid/pr/707", state: "MERGED", mergeStateStatus: "UNKNOWN", headRefOid: docsOnlyHead, headRefName: mergedRun.branch, mergedAt: "2026-07-17T12:00:00Z" },
  ]);
  const mergedReport = JSON.parse(fullSweep(["--run-id", "merged-run", "--no-codex", "--fixture", "classifier-basic", "--format", "json"], { env: recoveryEnv(mergedGh) }));
  assert.equal(mergedReport.sweep.recoveryAction, "created_fresh_run_after_merged");
  assert.notEqual(mergedReport.sweep.branch, mergedRun.branch);
  assert.equal(git(["rev-parse", mergedRun.branch]), docsOnlyHead);

  git(["branch", "zvorygin/docdrift-sweep", docsOnlyHead]);
  git(["push", "origin", "zvorygin/docdrift-sweep"]);
  git(["worktree", "add", path.join(repo, ".docdrift/worktrees/docdrift-sweep"), "zvorygin/docdrift-sweep"]);
  const legacyGh = writeFakeGh("fake-gh-legacy", [
    { number: 627, url: "https://example.invalid/pr/627", state: "CLOSED", mergeStateStatus: "DIRTY", headRefOid: docsOnlyHead, headRefName: "zvorygin/docdrift-sweep", mergedAt: null },
  ]);
  const legacyReport = JSON.parse(fullSweep(["--run-id", "after-legacy", "--adopt-legacy", "--no-codex", "--fixture", "classifier-basic", "--format", "json"], { env: recoveryEnv(legacyGh) }));
  assert.match(legacyReport.sweep.recoveryAction, /legacy/);
  assert.equal(git(["rev-parse", "zvorygin/docdrift-sweep"]), docsOnlyHead);
  assert.ok(fs.existsSync(path.join(repo, ".docdrift/runs", `legacy-docdrift-sweep-${docsOnlyHead.slice(0, 12)}`, "run-state.json")));

  const remoteUpdateRepo = path.join(fixtureRoot, "remote-update");
  run("git", ["clone", "--branch", "main", bareOrigin, remoteUpdateRepo], { cwd: fixtureRoot });
  run("git", ["config", "user.email", "agent@example.invalid"], { cwd: remoteUpdateRepo });
  run("git", ["config", "user.name", "Agent"], { cwd: remoteUpdateRepo });
  fs.writeFileSync(path.join(remoteUpdateRepo, "docs/design/testing.md"), "testing design\nremote-only update\n");
  run("git", ["add", "docs/design/testing.md"], { cwd: remoteUpdateRepo });
  run("git", ["commit", "-m", "Remote only testing docs"], {
    cwd: remoteUpdateRepo,
    date: "2026-06-20T12:06:00Z",
  });
  const remoteOnlyHead = run("git", ["rev-parse", "HEAD"], { cwd: remoteUpdateRepo }).trim();
  run("git", ["push", "origin", "main"], { cwd: remoteUpdateRepo });
  assert.equal(git(["rev-parse", "HEAD"]), docsOnlyHead);

  const fixtureDailyScript = path.join(repo, "scripts", "docdrift-daily.sh");
  fs.mkdirSync(path.dirname(fixtureDailyScript), { recursive: true });
  fs.copyFileSync(dailyScript, fixtureDailyScript);
  fs.chmodSync(fixtureDailyScript, 0o755);

  const fakeNodeBin = path.join(fixtureRoot, "fake-node-bin");
  fs.mkdirSync(fakeNodeBin, { recursive: true });
  writeExecutable(
    path.join("fake-node-bin", "node"),
    [
      "#!/usr/bin/env bash",
      "set -euo pipefail",
      "printf '%s\\n' \"$@\" > \"$FAKE_NODE_ARGS_FILE\"",
      "if [ -n \"${FAKE_NODE_STDOUT:-}\" ]; then printf '%s\\n' \"$FAKE_NODE_STDOUT\"; fi",
      "if [ -n \"${FAKE_NODE_STDERR:-}\" ]; then printf '%s\\n' \"$FAKE_NODE_STDERR\" >&2; fi",
      "exit \"${FAKE_NODE_EXIT:-0}\"",
      "",
    ].join("\n"),
  );
  const dailyObservabilityDir = path.join(fixtureRoot, "daily-observability");
  const dailyRunnerWorktree = path.join(fixtureRoot, "daily-runner");
  const dailyArgsFile = path.join(fixtureRoot, "daily-args.txt");
  runDailyWrapper(fakeNodeBin, ["--checkpoint-ref", "docdrift-checkpoint"], {
    dailyScript: fixtureDailyScript,
    observabilityDir: dailyObservabilityDir,
    runnerWorktree: dailyRunnerWorktree,
    argsFile: dailyArgsFile,
    stdout: "daily ok",
  });
  assert.deepEqual(fs.readFileSync(dailyArgsFile, "utf8").trim().split("\n"), [
    path.join(dailyRunnerWorktree, "scripts/docdrift-sweep.mjs"),
    "--full",
    "--repo",
    repo,
    "--head",
    "origin/main",
    "--max-commits",
    "300",
    "--codex-timeout-seconds",
    "300",
    "--checkpoint-ref",
    "docdrift-checkpoint",
  ]);
  assert.equal(run("git", ["-C", dailyRunnerWorktree, "rev-parse", "HEAD"], { cwd: fixtureRoot }).trim(), remoteOnlyHead);
  assert.equal(git(["rev-parse", "HEAD"]), docsOnlyHead);
  assert.equal(fs.existsSync(path.join(dailyObservabilityDir, "last-failure.md")), false);

  try {
    runDailyWrapper(fakeNodeBin, ["--checkpoint-ref", "docdrift-checkpoint"], {
      dailyScript: fixtureDailyScript,
      observabilityDir: dailyObservabilityDir,
      runnerWorktree: dailyRunnerWorktree,
      argsFile: dailyArgsFile,
      exitCode: 42,
      stderr: "classify budget exceeded: 75 considered commits exceeds --max-commits 25",
    });
    assert.fail("expected daily wrapper failure");
  } catch (error) {
    assert.equal(error.status, 42);
  }
  const failureReport = fs.readFileSync(path.join(dailyObservabilityDir, "last-failure.md"), "utf8");
  assert.match(failureReport, /# Documentation Drift Daily Failure/);
  assert.match(failureReport, /Exit code: `42`/);
  assert.match(failureReport, /--max-commits 300/);
  assert.match(failureReport, /classify budget exceeded/);
} finally {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}

console.log("docdrift sweeper tests passed");
