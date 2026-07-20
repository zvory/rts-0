#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  autoCommitBody,
  buildCodexArgs,
  buildFetchArgs,
  markdownReport,
  normalizeReport,
  parseArgs,
  QUALITY_PASS_ENV,
  renderPrompt,
  resolveHeadBranch,
  statusDescription,
} from "../scripts/adversarial-quality-pass.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

const options = parseArgs([
  "--base",
  "origin/main",
  "--head-branch",
  "zvorygin/example",
  "--context",
  "adversarial-quality-pass",
  "--post-status",
  "--push",
  "--markdown-report-file",
  "/tmp/adversarial-quality-pass.md",
]);
assert.equal(options.baseRef, "origin/main");
assert.equal(options.headBranch, "zvorygin/example");
assert.equal(options.context, "adversarial-quality-pass");
assert.equal(options.postStatus, true);
assert.equal(options.push, true);
assert.equal(options.markdownReportFile, "/tmp/adversarial-quality-pass.md");

assert.throws(() => parseArgs(["--unknown"]), /unknown argument/);

const prompt = renderPrompt({ baseRef: "origin/main", headRef: "HEAD" });
assert.match(prompt, /final autonomous quality pass/);
assert.match(prompt, /Correctness bugs/);
assert.match(prompt, /Architectural issues/);
assert.match(prompt, /provided clean branch worktree/);
assert.match(prompt, /outer helper handles pushing and PR creation/);
assert.match(prompt, /Ignore missing documentation updates/);
assert.match(prompt, /complete, coherent,\nworking state/);
assert.doesNotMatch(prompt, /fail the gate/i);
assert.doesNotMatch(prompt, /close the PR/i);

assert.deepEqual(
  buildCodexArgs({
    repoRoot: "/tmp/repo",
    gitCommonDir: "/tmp/git-common",
    schemaFile: "/tmp/schema.json",
    reportFile: "/tmp/report.json",
    codexModel: "gpt-5.5",
    prompt: "Review.",
  }),
  [
    "exec",
    "--cd",
    "/tmp/repo",
    "--add-dir",
    "/tmp/git-common",
    "--sandbox",
    "workspace-write",
    "-c",
    'approval_policy="never"',
    "--ephemeral",
    "--output-schema",
    "/tmp/schema.json",
    "--output-last-message",
    "/tmp/report.json",
    "--model",
    "gpt-5.5",
    "Review.",
  ],
);

const report = normalizeReport(`\`\`\`json
{
  "verdict": "improved_with_concerns",
  "summary": "Simplified the final branch.",
  "issues_found": ["lazy local patch"],
  "changes_made": ["rewrote helper boundary"],
  "verification": ["node tests/adversarial_quality_pass.mjs"],
  "remaining_concerns": ["watch CI"]
}
\`\`\``);

assert.deepEqual(report, {
  verdict: "improved_with_concerns",
  summary: "Simplified the final branch.",
  issues_found: ["lazy local patch"],
  changes_made: ["rewrote helper boundary"],
  verification: ["node tests/adversarial_quality_pass.mjs"],
  remaining_concerns: ["watch CI"],
});
assert.throws(() => normalizeReport({ verdict: "fail" }), /invalid verdict/);

const markdown = markdownReport(report);
assert.match(markdown, /## Adversarial quality pass/);
assert.match(markdown, /lazy local patch/);
assert.match(markdown, /watch CI/);
assert.equal(statusDescription(report), "improved with concerns; 1 concern(s)");
assert.match(autoCommitBody(report), /Verdict: improved_with_concerns/);
assert.match(autoCommitBody(report), /- rewrote helper boundary/);

assert.equal(path.basename(parseArgs([]).schemaFile), "adversarial-quality-pass.schema.json");

assert.equal(
  resolveHeadBranch({ requestedHeadBranch: "", currentBranch: "zvorygin/example" }),
  "zvorygin/example",
);
assert.equal(
  resolveHeadBranch({ requestedHeadBranch: "zvorygin/example", currentBranch: "zvorygin/example" }),
  "zvorygin/example",
);
assert.throws(
  () => resolveHeadBranch({ requestedHeadBranch: "zvorygin/other", currentBranch: "zvorygin/example" }),
  /head branch mismatch/,
);
assert.throws(
  () => resolveHeadBranch({ requestedHeadBranch: "zvorygin/example", currentBranch: "" }),
  /detached HEAD/,
);

assert.deepEqual(buildFetchArgs({ remote: "origin", baseRef: "origin/main" }), [
  "fetch",
  "origin",
  "+refs/heads/main:refs/remotes/origin/main",
]);
assert.deepEqual(buildFetchArgs({ remote: "origin", baseRef: "main" }), [
  "fetch",
  "origin",
  "+refs/heads/main:refs/remotes/origin/main",
]);
assert.deepEqual(buildFetchArgs({ remote: "origin", baseRef: "upstream/main" }), [
  "fetch",
  "origin",
  "upstream/main",
]);

const nestedAgentPr = spawnSync("bash", ["scripts/agent-pr.sh", "--dry-run"], {
  cwd: repoRoot,
  encoding: "utf8",
  env: testEnv({ [QUALITY_PASS_ENV]: "1" }),
});
assert.equal(nestedAgentPr.status, 2);
assert.match(nestedAgentPr.stderr, /outer helper owns PR lifecycle/);

function testEnv(extra = {}) {
  const env = { ...process.env };
  delete env[QUALITY_PASS_ENV];
  return { ...env, ...extra };
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    ...options,
    env: testEnv(options.env || {}),
  });
  assert.equal(
    result.status,
    0,
    `${command} ${args.join(" ")} failed\nstdout:\n${result.stdout || ""}\nstderr:\n${result.stderr || ""}`,
  );
  return result;
}

function writeExecutable(file, contents) {
  fs.writeFileSync(file, contents, { mode: 0o755 });
}

function copyWorkflowScripts(targetRepo) {
  const targetScripts = path.join(targetRepo, "scripts");
  const targetTests = path.join(targetRepo, "tests");
  fs.mkdirSync(targetScripts, { recursive: true });
  fs.mkdirSync(targetTests, { recursive: true });
  for (const script of [
    "agent-pr.sh",
    "agent-pr-passes.mjs",
    "agent-pr-passes.json",
    "patch-note-pass.mjs",
    "patch-note-pass.schema.json",
    "archive-completed-plans.mjs",
    "plan-phase-status.mjs",
    "adversarial-quality-pass.mjs",
    "adversarial-quality-pass.schema.json",
    "format-touched-rust.sh",
  ]) {
    fs.copyFileSync(path.join(repoRoot, "scripts", script), path.join(targetScripts, script));
  }
  fs.copyFileSync(path.join(repoRoot, "tests", "select-suites.mjs"), path.join(targetTests, "select-suites.mjs"));
  fs.chmodSync(path.join(targetScripts, "agent-pr.sh"), 0o755);
  fs.chmodSync(path.join(targetScripts, "agent-pr-passes.mjs"), 0o755);
  fs.chmodSync(path.join(targetScripts, "patch-note-pass.mjs"), 0o755);
  fs.chmodSync(path.join(targetScripts, "archive-completed-plans.mjs"), 0o755);
  fs.chmodSync(path.join(targetScripts, "adversarial-quality-pass.mjs"), 0o755);
  fs.chmodSync(path.join(targetScripts, "format-touched-rust.sh"), 0o755);
}

const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-agent-pr-quality-report-"));
try {
  const originPath = path.join(tempRoot, "origin.git");
  const workPath = path.join(tempRoot, "work");
  const binPath = path.join(tempRoot, "bin");
  const capturedBody = path.join(tempRoot, "pr-body.md");
  const docsOnlyBody = path.join(tempRoot, "docs-only-pr-body.md");
  const codexCalledMarker = path.join(tempRoot, "codex-called.txt");
  const docsOnlyCodexCalledMarker = path.join(tempRoot, "docs-only-codex-called.txt");
  const docsOnlyStatusCapture = path.join(tempRoot, "docs-only-gh-api.txt");
  fs.mkdirSync(binPath, { recursive: true });

  writeExecutable(
    path.join(binPath, "codex"),
    `#!/usr/bin/env bash
set -euo pipefail
report_file=""
is_patch_note=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    report_file="$2"
    shift
  elif [ "$1" = "--output-schema" ]; then
    if [[ "$2" == */patch-note-pass.schema.json ]]; then
      is_patch_note=1
    fi
    shift
  fi
  shift
done
if [ -z "$report_file" ]; then
  echo "missing report file" >&2
  exit 1
fi
if [ "$is_patch_note" = "1" ]; then
  cat >"$report_file" <<'JSON'
{
  "decision": "no_patch_note",
  "title": "",
  "changes": [],
  "playtest_watch": [],
  "reason": "The fixture source edit has no player-facing gameplay effect."
}
JSON
  exit 0
fi
if [ -n "\${CODEX_CALLED_MARKER:-}" ]; then
  printf 'codex called\\n' >>"$CODEX_CALLED_MARKER"
fi
if [ "\${RTS_ADVERSARIAL_QUALITY_PASS:-}" != "1" ]; then
  echo "missing quality pass environment" >&2
  exit 1
fi
if [ "\${CODEX_MUTATE_AGENT_PR:-}" = "1" ]; then
  printf '\\n# fixture codex mutation\\n' >> scripts/agent-pr.sh
fi
cat >"$report_file" <<'JSON'
{
  "verdict": "improved",
  "summary": "Captured report body.",
  "issues_found": ["PR body lacked a durable audit trail"],
  "changes_made": ["embedded the quality-pass report"],
  "verification": ["fake codex verification"],
  "remaining_concerns": []
}
JSON
`,
  );
  writeExecutable(
    path.join(binPath, "gh"),
    `#!/usr/bin/env bash
set -euo pipefail
if [ "$1" = "api" ]; then
  if [ -n "\${AGENT_GH_API_CAPTURE:-}" ]; then
    printf '%s\\n' "$*" >>"$AGENT_GH_API_CAPTURE"
  fi
  exit 0
fi
if [ "$1" = "label" ] && [ "\${2:-}" = "create" ]; then
  exit 0
fi
if [ "$1" = "pr" ] && [ "\${2:-}" = "list" ]; then
  exit 0
fi
if [ "$1" = "pr" ] && [ "\${2:-}" = "create" ]; then
  body_file=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--body-file" ]; then
      body_file="$2"
      shift
    fi
    shift
  done
  if [ -z "$body_file" ]; then
    echo "missing PR body file" >&2
    exit 1
  fi
  cat "$body_file" >"$AGENT_PR_BODY_CAPTURE"
  printf 'https://github.example/zvory/rts-0/pull/123\\n'
  exit 0
fi
if [ "$1" = "pr" ] && [ "\${2:-}" = "merge" ]; then
  exit 0
fi
echo "unexpected gh invocation: $*" >&2
exit 1
`,
  );
  writeExecutable(
    path.join(binPath, "rustfmt"),
    `#!/usr/bin/env bash
set -euo pipefail
for argument in "$@"; do
  case "$argument" in
    *.rs) perl -0pi -e 's/fn main\\(\\)\\{\\}/fn main() {}/g' "$argument" ;;
  esac
done
`,
  );

  run("git", ["init", "--bare", originPath]);
  fs.mkdirSync(workPath);
  run("git", ["init"], { cwd: workPath });
  run("git", ["checkout", "-b", "main"], { cwd: workPath });
  run("git", ["config", "user.email", "qa@example.invalid"], { cwd: workPath });
  run("git", ["config", "user.name", "Quality Pass Test"], { cwd: workPath });
  copyWorkflowScripts(workPath);
  fs.writeFileSync(path.join(workPath, "README.md"), "initial\n");
  fs.mkdirSync(path.join(workPath, "plans", "fixture"), { recursive: true });
  fs.writeFileSync(path.join(workPath, "plans", "fixture", "plan.md"), "# Fixture plan\n");
  fs.writeFileSync(path.join(workPath, "plans", "fixture", "phase-1.md"), "Status: Not started.\n");
  run("git", ["add", "-A"], { cwd: workPath });
  run("git", ["commit", "-m", "Initial"], { cwd: workPath });
  run("git", ["remote", "add", "origin", originPath], { cwd: workPath });
  run("git", ["push", "-u", "origin", "main"], { cwd: workPath });
  run("git", ["checkout", "-b", "zvorygin/quality-report-body"], { cwd: workPath });
  fs.appendFileSync(path.join(workPath, "README.md"), "implementation branch docs change\n");
  fs.writeFileSync(path.join(workPath, "--implementation.rs"), "branch change\n");
  fs.mkdirSync(path.join(workPath, "server", "src"), { recursive: true });
  fs.writeFileSync(path.join(workPath, "server", "src", "branch.rs"), "fn main(){}\n");
  run("git", ["add", "--", "README.md", "--implementation.rs", "server/src/branch.rs"], { cwd: workPath });
  run("git", ["commit", "-m", "Change branch"], { cwd: workPath });

  run("scripts/agent-pr.sh", ["--owner", "tester", "--title", "Quality report body", "--verification", "workflow fixture"], {
    cwd: workPath,
    env: {
      AGENT_PR_BODY_CAPTURE: capturedBody,
      CODEX_CALLED_MARKER: codexCalledMarker,
      CODEX_MUTATE_AGENT_PR: "1",
      GH_BIN: path.join(binPath, "gh"),
      PATH: `${binPath}:${process.env.PATH}`,
    },
  });

  const body = fs.readFileSync(capturedBody, "utf8");
  assert.match(body, /<!-- rts-agent-pr:v1 -->/);
  assert.match(body, /^Focused-Verification: workflow fixture$/m);
  assert.match(body, /## Adversarial quality pass/);
  assert.match(body, /## Agent PR passes/);
  assert.match(body, /Decision: no_patch_note/);
  assert.match(body, /Verdict: improved/);
  assert.match(body, /Captured report body\./);
  assert.match(body, /- embedded the quality-pass report/);
  assert.match(fs.readFileSync(codexCalledMarker, "utf8"), /codex called/);
  assert.equal(fs.readFileSync(path.join(workPath, "server", "src", "branch.rs"), "utf8"), "fn main() {}\n");
  assert.match(run("git", ["log", "-1", "--format=%s"], { cwd: workPath }).stdout, /Run adversarial quality pass/);

  run("git", ["checkout", "main"], { cwd: workPath });
  run("git", ["checkout", "-b", "zvorygin/docs-only-quality-skip"], { cwd: workPath });
  fs.appendFileSync(path.join(workPath, "README.md"), "docs-only branch change\n");
  fs.writeFileSync(path.join(workPath, "plans", "fixture", "phase-1.md"), "Status: Done. Manual QA remains.\n");
  run("git", ["add", "README.md", "plans/fixture/phase-1.md"], { cwd: workPath });
  run("git", ["commit", "-m", "Document branch"], { cwd: workPath });

  const docsOnlyHeadMismatch = spawnSync(
    "scripts/agent-pr.sh",
    ["--owner", "tester", "--head", "zvorygin/other", "--verification", "mismatch fixture"],
    {
      cwd: workPath,
      encoding: "utf8",
      env: testEnv({
        AGENT_GH_API_CAPTURE: docsOnlyStatusCapture,
        CODEX_CALLED_MARKER: docsOnlyCodexCalledMarker,
        GH_BIN: path.join(binPath, "gh"),
        PATH: `${binPath}:${process.env.PATH}`,
      }),
    },
  );
  assert.equal(docsOnlyHeadMismatch.status, 2);
  assert.match(docsOnlyHeadMismatch.stderr, /head branch mismatch/);
  assert.equal(fs.existsSync(docsOnlyCodexCalledMarker), false, "mismatched --head should not invoke Codex");
  assert.equal(fs.existsSync(docsOnlyStatusCapture), false, "mismatched --head should not post status");

  run("scripts/agent-pr.sh", ["--owner", "tester", "--title", "Docs-only quality skip", "--verification", "docs fixture"], {
    cwd: workPath,
    env: {
      AGENT_GH_API_CAPTURE: docsOnlyStatusCapture,
      AGENT_PR_BODY_CAPTURE: docsOnlyBody,
      CODEX_CALLED_MARKER: docsOnlyCodexCalledMarker,
      GH_BIN: path.join(binPath, "gh"),
      PATH: `${binPath}:${process.env.PATH}`,
    },
  });

  assert.equal(fs.existsSync(docsOnlyCodexCalledMarker), false, "docs-only PR should not invoke Codex");
  const docsBody = fs.readFileSync(docsOnlyBody, "utf8");
  assert.match(docsBody, /<!-- rts-agent-pr:v1 -->/);
  assert.match(docsBody, /^Focused-Verification: docs fixture$/m);
  assert.match(docsBody, /## Adversarial quality pass/);
  assert.match(docsBody, /Verdict: skipped_docs_only/);
  assert.match(docsBody, /changes only Markdown documentation files/);
  assert.match(docsBody, /classified this branch as `docs_only=true`/);
  const docsOnlyStatus = fs.readFileSync(docsOnlyStatusCapture, "utf8");
  assert.match(docsOnlyStatus, /statuses\//);
  assert.match(docsOnlyStatus, /description=skipped for docs-only changes/);
  assert.equal(fs.existsSync(path.join(workPath, "plans", "fixture")), false);
  assert.equal(fs.existsSync(path.join(workPath, "plans", "archive", "fixture", "phase-1.md")), true);
  assert.match(run("git", ["log", "-1", "--format=%s"], { cwd: workPath }).stdout, /Archive completed plan: fixture/);
} finally {
  fs.rmSync(tempRoot, { recursive: true, force: true });
}

console.log("adversarial quality pass tests passed");
