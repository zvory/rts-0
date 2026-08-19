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
  hasLatestSuccessfulStatus,
  markdownReport,
  normalizeReport,
  parseArgs,
  MAX_PRIOR_FOCUSED_VERIFICATION_CHARS,
  QUALITY_PASS_ENV,
  parseReviewedHeadMarker,
  renderPrompt,
  reviewedHeadMarker,
  resolveHeadBranch,
  selectReviewMode,
  statusDescription,
} from "../scripts/adversarial-quality-pass.mjs";
import { preflightCommands } from "../scripts/agent-pr-preflight.mjs";

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
  "--prior-focused-verification",
  "node tests/agent_pr.mjs passed",
]);
assert.equal(options.baseRef, "origin/main");
assert.equal(options.headBranch, "zvorygin/example");
assert.equal(options.context, "adversarial-quality-pass");
assert.equal(options.postStatus, true);
assert.equal(options.push, true);
assert.equal(options.markdownReportFile, "/tmp/adversarial-quality-pass.md");
assert.equal(options.priorFocusedVerification, "node tests/agent_pr.mjs passed");

assert.throws(() => parseArgs(["--unknown"]), /unknown argument/);
assert.throws(
  () => parseArgs(["--prior-focused-verification", "x".repeat(MAX_PRIOR_FOCUSED_VERIFICATION_CHARS + 1)]),
  /exceeds the 2000-character limit/,
);

assert.deepEqual(
  preflightCommands("origin/main").map(({ command, args }) => [command, ...args]),
  [
    ["git", "diff", "--check", "origin/main...HEAD"],
    ["node", "scripts/check-docs-health.mjs"],
    ["node", "scripts/check-source-file-sizes.mjs"],
    ["node", "tests/select-suites.mjs", "--verify"],
    ["node", "scripts/check-faction-assumptions.mjs"],
    ["node", "scripts/check-deploy-assets.mjs"],
  ],
);

const prompt = renderPrompt({
  baseRef: "origin/main",
  headRef: "HEAD",
  priorFocusedVerification: "node tests/adversarial_quality_pass.mjs passed",
  reviewInputs: [{
    path: "client/assets/snapshot-streams/fixed-roster-hellhole.rtsstream",
    classification: "binary",
    oldBytes: 100,
    newBytes: 101,
    oldBlob: "old-stream",
    newBlob: "new-stream",
    added: null,
    deleted: null,
  }],
});
assert.match(prompt, /final autonomous quality pass/);
assert.match(prompt, /Correctness bugs/);
assert.match(prompt, /Architectural issues/);
assert.match(prompt, /provided clean branch worktree/);
assert.match(prompt, /outer helper handles pushing and PR creation/);
assert.match(prompt, /represented only by bounded metadata/);
assert.match(prompt, /Do not print,\ndecode, cat, git-show, or git-diff their raw contents/);
assert.match(prompt, /fixed-roster-hellhole\.rtsstream \| class=binary/);
assert.match(prompt, /AI behavior is outside your authority: do not create, alter, or approve it/);
assert.match(prompt, /Refactor AI code only\nwhen behavior is preserved exactly/);
assert.match(prompt, /Bundled map assets under server\/assets\/maps\/ are immutable review inputs/);
assert.match(prompt, /never create, modify, delete, rename, format, regenerate, stage, or\ncommit files there/);
assert.match(prompt, /leave every bundled\nmap file byte-for-byte unchanged/);
assert.match(prompt, /Ignore missing documentation updates/);
assert.match(prompt, /complete, coherent,\nworking state/);
assert.doesNotMatch(prompt, /fail the gate/i);
assert.doesNotMatch(prompt, /close the PR/i);
assert.match(prompt, /Review mode: Full\./);
assert.match(prompt, /Prior focused verification:\nnode tests\/adversarial_quality_pass\.mjs passed/);
assert.equal((prompt.match(/node tests\/adversarial_quality_pass\.mjs passed/g) ?? []).length, 1);
assert.match(prompt, /claimed evidence to evaluate against the diff/);
assert.match(prompt, /offline, deterministic focused tests,\nlinters, static policy scripts, format checks, and repository inspections/);
assert.match(prompt, /Do not start HTTP or WebSocket listeners,\nbrowsers, Chrome, Interact, Tailnet preview/);
assert.match(prompt, /exact behavior\nstill unverified and why it matters/);
assert.match(prompt, /do not report generic EPERM, sandbox, or unavailable-tool\nconcerns/);
assert.match(prompt, /do not broaden scope into opportunistic cleanup\nor a new validation harness/);

const defaultVerificationPrompt = renderPrompt({
  baseRef: "origin/main",
  headRef: "HEAD",
});
assert.match(defaultVerificationPrompt, /Prior focused verification:\nnot supplied/);
assert.equal((defaultVerificationPrompt.match(/Prior focused verification:/g) ?? []).length, 1);

const reviewedAnchor = "a".repeat(40);
const correctionHead = "b".repeat(40);
const markerBody = `<!-- rts-agent-pr:v1 -->
Agent-Owned: true
${reviewedHeadMarker(reviewedAnchor)}
<!-- /rts-agent-pr -->
## Adversarial quality pass
`;
assert.deepEqual(parseReviewedHeadMarker(markerBody), { sha: reviewedAnchor, reason: "" });
assert.equal(
  parseReviewedHeadMarker(markerBody.replace("<!-- /rts-agent-pr -->", `${reviewedHeadMarker(reviewedAnchor)}\n<!-- /rts-agent-pr -->`)).reason,
  "duplicate reviewed-head markers",
);
assert.equal(
  parseReviewedHeadMarker("<!-- rts-agent-pr:v1 -->\nAgent-Owned: true\n<!-- rts-agent-pr:reviewed-head:v1 sha=BAD -->\n<!-- /rts-agent-pr -->").reason,
  "malformed reviewed-head marker",
);
assert.equal(
  parseReviewedHeadMarker(`<!-- rts-agent-pr:v1 -->\nAgent-Owned: true\n<!-- /rts-agent-pr -->\n${reviewedHeadMarker(reviewedAnchor)}`).reason,
  "missing reviewed-head marker",
);

const selectMode = (overrides = {}) => selectReviewMode({
  baseRef: "origin/main",
  currentHead: correctionHead,
  existingPrBodyFile: markerBody,
  existingPrBase: "main",
  existingPrHead: correctionHead,
  expectedBase: "main",
  getSuccessfulStatus: () => true,
  commitExists: () => true,
  isAncestor: () => true,
  hasMergeCommit: () => false,
  ...overrides,
});
assert.deepEqual(selectMode(), {
  mode: "incremental",
  label: "Incremental",
  reviewBase: reviewedAnchor,
  reviewedHead: reviewedAnchor,
  reason: "verified reviewed-head has a simple linear correction range",
});
assert.equal(selectMode({ currentHead: reviewedAnchor, existingPrHead: reviewedAnchor }).mode, "already-reviewed");
assert.equal(selectMode({ existingPrBodyFile: "" }).mode, "full");
assert.equal(selectMode({ existingPrBodyFile: "malformed marker" }).mode, "full");
assert.equal(
  selectMode({
    existingPrBodyFile: `<!-- rts-agent-pr:v1 -->\nAgent-Owned: true\n<!-- /rts-agent-pr -->\n${reviewedHeadMarker(reviewedAnchor)}`,
  }).mode,
  "full",
);
assert.equal(selectMode({ getSuccessfulStatus: () => false }).mode, "full");
assert.equal(selectMode({ getSuccessfulStatus: () => { throw new Error("offline"); } }).reason, "GitHub status lookup failed");
assert.equal(selectMode({ commitExists: () => false }).mode, "full");
assert.equal(selectMode({ isAncestor: () => false }).mode, "full");
assert.equal(selectMode({ hasMergeCommit: () => true }).mode, "full");
assert.equal(selectMode({ existingPrBase: "release" }).mode, "full");
assert.equal(selectMode({ existingPrHead: "c".repeat(40) }).mode, "full");
assert.equal(hasLatestSuccessfulStatus([
  { context: "adversarial-quality-pass", state: "success" },
], "adversarial-quality-pass"), true);
assert.equal(hasLatestSuccessfulStatus([
  { context: "adversarial-quality-pass", state: "failure" },
  { context: "adversarial-quality-pass", state: "success" },
], "adversarial-quality-pass"), false);
assert.equal(hasLatestSuccessfulStatus([], "adversarial-quality-pass"), false);
assert.throws(() => hasLatestSuccessfulStatus({}, "adversarial-quality-pass"), /did not return an array/);

const incrementalPrompt = renderPrompt({
  baseRef: reviewedAnchor,
  headRef: "HEAD",
  reviewMode: "incremental",
  reviewedHead: reviewedAnchor,
  reviewInputs: [],
});
assert.match(incrementalPrompt, /Review mode: Incremental\./);
assert.match(incrementalPrompt, /Do not reopen unchanged earlier branch work/);

assert.deepEqual(
  buildCodexArgs({
    repoRoot: "/tmp/repo",
    gitCommonDir: "/tmp/git-common",
    schemaFile: "/tmp/schema.json",
    reportFile: "/tmp/report.json",
    codexModel: "gpt-5.5",
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
    "-",
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
    "review-inputs.mjs",
    "archive-completed-plans.mjs",
    "plan-phase-status.mjs",
    "agent-pr-preflight.mjs",
    "adversarial-quality-pass.mjs",
    "adversarial-quality-pass.schema.json",
    "format-touched-rust.sh",
  ]) {
    fs.copyFileSync(path.join(repoRoot, "scripts", script), path.join(targetScripts, script));
  }
  for (const check of [
    "check-docs-health.mjs",
    "check-source-file-sizes.mjs",
    "check-faction-assumptions.mjs",
    "check-deploy-assets.mjs",
  ]) {
    writeExecutable(
      path.join(targetScripts, check),
      `#!/usr/bin/env node
import fs from "node:fs";
if (process.argv[1].endsWith("check-source-file-sizes.mjs") && fs.existsSync("source-size-invalid.js")) {
  console.error("fixture source-size policy violation");
  process.exit(1);
}
console.log("fixture ${check} passed");
`,
    );
  }
  writeExecutable(
    path.join(targetTests, "select-suites.mjs"),
    `#!/usr/bin/env node
const args = process.argv.slice(2);
if (args.includes("--ci-policy")) {
  const files = args.slice(args.indexOf("--ci-policy") + 1);
  const docsOnly = files.length > 0 && files.every((file) => file.endsWith(".md"));
  console.log("ci_class=" + (docsOnly ? "docs_only" : "full"));
  console.log("docs_only=" + docsOnly);
  process.exit(0);
}
if (args.includes("--verify")) console.log("fixture selector verification passed");
`,
  );
  fs.chmodSync(path.join(targetScripts, "agent-pr.sh"), 0o755);
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
  const qualityStatusCapture = path.join(tempRoot, "quality-gh-api.txt");
  const qualityPromptCapture = path.join(tempRoot, "quality-prompt.txt");
  const incrementalBody = path.join(tempRoot, "incremental-pr-body.md");
  const incrementalPromptCapture = path.join(tempRoot, "incremental-prompt.txt");
  const incrementalStatusCapture = path.join(tempRoot, "incremental-gh-api.txt");
  const noChangeBody = path.join(tempRoot, "no-change-pr-body.md");
  const noChangeCodexCalledMarker = path.join(tempRoot, "no-change-codex-called.txt");
  const noChangeStatusCapture = path.join(tempRoot, "no-change-gh-api.txt");
  const extraBodyFile = path.join(tempRoot, "extra-pr-body.md");
  const preReviewCodexCalledMarker = path.join(tempRoot, "pre-review-codex-called.txt");
  const preReviewStatusCapture = path.join(tempRoot, "pre-review-gh-api.txt");
  const finalHeadCodexCalledMarker = path.join(tempRoot, "final-head-codex-called.txt");
  const finalHeadStatusCapture = path.join(tempRoot, "final-head-gh-api.txt");
  const finalHeadBody = path.join(tempRoot, "final-head-pr-body.md");
  const rewrittenPreflightCodexCalledMarker = path.join(tempRoot, "rewritten-preflight-codex-called.txt");
  const rewrittenPreflightStatusCapture = path.join(tempRoot, "rewritten-preflight-gh-api.txt");
  const rewrittenPreflightBody = path.join(tempRoot, "rewritten-preflight-pr-body.md");
  const protectedMapStatusCapture = path.join(tempRoot, "protected-map-gh-api.txt");
  const protectedMapBody = path.join(tempRoot, "protected-map-pr-body.md");
  const protectedMapCodexCalledMarker = path.join(tempRoot, "protected-map-codex-called.txt");
  fs.mkdirSync(binPath, { recursive: true });
  fs.writeFileSync(extraBodyFile, "## Fixture caller notes\n\nKeep this once.\n");

  writeExecutable(
    path.join(binPath, "codex"),
    `#!/usr/bin/env bash
set -euo pipefail
report_file=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    report_file="$2"
    shift
  elif [ "$1" = "--output-schema" ]; then shift; fi
  shift
done
if [ -z "$report_file" ]; then
  echo "missing report file" >&2
  exit 1
fi
quality_prompt="$(cat)"
if [[ "$quality_prompt" != *"final autonomous quality pass"* ]] ||
   [[ "$quality_prompt" != *"Changed-path metadata:"* ]]; then
  echo "missing quality pass prompt on stdin" >&2
  exit 1
fi
if [ -n "\${CODEX_CALLED_MARKER:-}" ]; then
  printf 'codex called\\n' >>"$CODEX_CALLED_MARKER"
fi
if [ -n "\${AGENT_CODEX_PROMPT_CAPTURE:-}" ]; then
  printf '%s' "$quality_prompt" >"$AGENT_CODEX_PROMPT_CAPTURE"
fi
if [ "\${RTS_ADVERSARIAL_QUALITY_PASS:-}" != "1" ]; then
  echo "missing quality pass environment" >&2
  exit 1
fi
if [ "\${CODEX_MUTATE_AGENT_PR:-}" = "1" ]; then
  printf '\\n# fixture codex mutation\\n' >> scripts/agent-pr.sh
fi
if [ "\${CODEX_CREATE_INVALID_FINAL_HEAD:-}" = "1" ]; then
  printf 'fixture source-size violation\\n' > source-size-invalid.js
fi
if [ "\${CODEX_REWRITE_PREFLIGHT_TO_REJECT:-}" = "1" ]; then
  cat > scripts/agent-pr-preflight.mjs <<'PREFLIGHT'
#!/usr/bin/env node
console.error("fixture rewritten preflight rejection");
process.exit(1);
PREFLIGHT
fi
if [ "\${CODEX_MUTATE_PROTECTED_MAP:-}" = "1" ]; then
  mkdir -p server/assets/maps
  printf '{"fixture":true}\n' > server/assets/maps/quality-pass-mutation.json
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
  if [[ "$*" == *"/commits/"*"/statuses?per_page=100"* ]]; then
    if [ "\${AGENT_GH_STATUS_FAILURE:-}" = "1" ]; then
      echo "fixture GitHub status lookup failed" >&2
      exit 1
    fi
    if [ -n "\${AGENT_GH_STATUS_SHA:-}" ] && [[ "$*" == *"/commits/\${AGENT_GH_STATUS_SHA:-}/statuses?per_page=100"* ]]; then
      printf '[{"context":"adversarial-quality-pass","state":"%s"}]\\n' "\${AGENT_GH_STATUS_STATE:-success}"
    else
      printf '[]\\n'
    fi
    exit 0
  fi
  if [[ "$*" == *"/statuses/"* ]] && [ -n "\${AGENT_GH_API_CAPTURE:-}" ]; then
    printf '%s\\n' "$*" >>"$AGENT_GH_API_CAPTURE"
  fi
  exit 0
fi
if [ "$1" = "label" ] && [ "\${2:-}" = "create" ]; then
  exit 0
fi
if [ "$1" = "pr" ] && [ "\${2:-}" = "list" ]; then
  if [ -n "\${AGENT_GH_PR_BODY_FILE:-}" ]; then
    jq -n \
      --rawfile body "\${AGENT_GH_PR_BODY_FILE:-}" \
      --arg base "\${AGENT_GH_PR_BASE:-main}" \
      --arg head "$(git rev-parse HEAD)" \
      '{number: 123, url: "https://github.example/zvory/rts-0/pull/123", body: $body, baseRefName: $base, headRefOid: $head}'
  fi
  exit 0
fi
if [ "$1" = "pr" ] && { [ "\${2:-}" = "create" ] || [ "\${2:-}" = "edit" ]; }; then
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
  if [ "\${2:-}" = "create" ]; then
    printf 'https://github.example/zvory/rts-0/pull/123\\n'
  fi
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

  run("git", ["checkout", "-b", "zvorygin/pre-review-preflight-reject"], { cwd: workPath });
  fs.writeFileSync(path.join(workPath, "source-size-invalid.js"), "fixture source-size violation\n");
  run("git", ["add", "source-size-invalid.js"], { cwd: workPath });
  run("git", ["commit", "-m", "Invalid preflight branch"], { cwd: workPath });
  const preReviewFailure = spawnSync(
    "scripts/agent-pr.sh",
    ["--owner", "tester", "--verification", "pre-review fixture"],
    {
      cwd: workPath,
      encoding: "utf8",
      env: testEnv({
        AGENT_GH_API_CAPTURE: preReviewStatusCapture,
        CODEX_CALLED_MARKER: preReviewCodexCalledMarker,
        GH_BIN: path.join(binPath, "gh"),
        PATH: `${binPath}:${process.env.PATH}`,
      }),
    },
  );
  assert.notEqual(preReviewFailure.status, 0);
  assert.match(`${preReviewFailure.stdout}\n${preReviewFailure.stderr}`, /failed `node scripts\/check-source-file-sizes\.mjs`/);
  assert.equal(fs.existsSync(preReviewCodexCalledMarker), false, "pre-review failure must not invoke Codex");
  assert.equal(fs.existsSync(preReviewStatusCapture), false, "pre-review failure must not post status");
  assert.equal(
    run("git", ["ls-remote", "--heads", "origin", "zvorygin/pre-review-preflight-reject"], { cwd: workPath }).stdout,
    "",
    "pre-review failure must not push the branch",
  );

  const oversizedVerificationTmp = fs.mkdtempSync(path.join(tempRoot, "oversized-verification-tmp-"));
  const oversizedVerification = spawnSync(
    "scripts/agent-pr.sh",
    ["--verification", "x".repeat(MAX_PRIOR_FOCUSED_VERIFICATION_CHARS + 1)],
    {
      cwd: workPath,
      encoding: "utf8",
      env: testEnv({
        CODEX_CALLED_MARKER: preReviewCodexCalledMarker,
        GH_BIN: path.join(binPath, "gh"),
        PATH: `${binPath}:${process.env.PATH}`,
        TMPDIR: oversizedVerificationTmp,
      }),
    },
  );
  assert.equal(oversizedVerification.status, 2);
  assert.match(oversizedVerification.stderr, /--verification exceeds the 2000-character limit/);
  assert.equal(fs.existsSync(preReviewCodexCalledMarker), false, "oversized verification must not invoke Codex");
  assert.deepEqual(fs.readdirSync(oversizedVerificationTmp), [], "oversized verification must clean up its stable helper copy");

  run("git", ["checkout", "main"], { cwd: workPath });
  run("git", ["checkout", "-b", "zvorygin/quality-report-body"], { cwd: workPath });
  fs.appendFileSync(path.join(workPath, "README.md"), "implementation branch docs change\n");
  fs.writeFileSync(path.join(workPath, "--implementation.rs"), "branch change\n");
  fs.mkdirSync(path.join(workPath, "server", "src"), { recursive: true });
  fs.writeFileSync(path.join(workPath, "server", "src", "branch.rs"), "fn main(){}\n");
  run("git", ["add", "--", "README.md", "--implementation.rs", "server/src/branch.rs"], { cwd: workPath });
  run("git", ["commit", "-m", "Change branch"], { cwd: workPath });

  const qualityPassRun = run("scripts/agent-pr.sh", ["--owner", "tester", "--title", "Quality report body", "--verification", "workflow fixture", "--body-file", extraBodyFile], {
    cwd: workPath,
    env: {
      AGENT_GH_API_CAPTURE: qualityStatusCapture,
      AGENT_PR_BODY_CAPTURE: capturedBody,
      CODEX_CALLED_MARKER: codexCalledMarker,
      AGENT_CODEX_PROMPT_CAPTURE: qualityPromptCapture,
      CODEX_MUTATE_AGENT_PR: "1",
      GH_BIN: path.join(binPath, "gh"),
      PATH: `${binPath}:${process.env.PATH}`,
    },
  });

  const preflightMarker = "agent-pr preflight: running git diff --check origin/main...HEAD";
  const firstPreflight = qualityPassRun.stdout.indexOf(preflightMarker);
  const reviewStart = qualityPassRun.stdout.indexOf("quality-pass: running Codex final quality pass");
  const finalPreflight = qualityPassRun.stdout.indexOf(preflightMarker, firstPreflight + 1);
  assert(firstPreflight >= 0 && reviewStart > firstPreflight && finalPreflight > reviewStart);
  assert.equal(qualityPassRun.stdout.split(preflightMarker).length - 1, 2);

  const body = fs.readFileSync(capturedBody, "utf8");
  assert.match(body, /<!-- rts-agent-pr:v1 -->/);
  assert.match(body, /^Focused-Verification: workflow fixture$/m);
  assert.match(body, /## Adversarial quality pass/);
  assert.match(body, /Verdict: improved/);
  assert.match(body, /Captured report body\./);
  assert.match(body, /- embedded the quality-pass report/);
  assert.match(body, /<!-- rts-agent-pr:quality-report:v1 -->/);
  assert.match(body, /<!-- \/rts-agent-pr:quality-report -->/);
  assert.match(fs.readFileSync(codexCalledMarker, "utf8"), /codex called/);
  const capturedQualityPrompt = fs.readFileSync(qualityPromptCapture, "utf8");
  assert.match(capturedQualityPrompt, /Prior focused verification:\nworkflow fixture/);
  assert.equal((capturedQualityPrompt.match(/workflow fixture/g) ?? []).length, 1);
  assert.equal((fs.readFileSync(qualityStatusCapture, "utf8").match(/statuses\//g) ?? []).length, 1);
  assert.equal(fs.readFileSync(path.join(workPath, "server", "src", "branch.rs"), "utf8"), "fn main() {}\n");
  assert.match(run("git", ["log", "-1", "--format=%s"], { cwd: workPath }).stdout, /Run adversarial quality pass/);
  const fullReviewedHead = run("git", ["rev-parse", "HEAD"], { cwd: workPath }).stdout.trim();
  assert.match(body, new RegExp(`Review-Mode: Full\\nReview-Base: origin/main\\n${reviewedHeadMarker(fullReviewedHead)}`));

  fs.writeFileSync(path.join(workPath, "ci-fix.js"), "export const ciFix = true;\n");
  run("git", ["add", "ci-fix.js"], { cwd: workPath });
  run("git", ["commit", "-m", "Apply deterministic CI fix"], { cwd: workPath });
  run("git", ["push", "origin", "HEAD:refs/heads/zvorygin/quality-report-body"], { cwd: workPath });
  const incrementalRun = run("scripts/agent-pr.sh", ["--owner", "tester", "--verification", "incremental fixture", "--body-file", extraBodyFile], {
    cwd: workPath,
    env: {
      AGENT_CODEX_PROMPT_CAPTURE: incrementalPromptCapture,
      AGENT_GH_API_CAPTURE: incrementalStatusCapture,
      AGENT_GH_PR_BODY_FILE: capturedBody,
      AGENT_GH_STATUS_SHA: fullReviewedHead,
      AGENT_PR_BODY_CAPTURE: incrementalBody,
      CODEX_CALLED_MARKER: codexCalledMarker,
      GH_BIN: path.join(binPath, "gh"),
      PATH: `${binPath}:${process.env.PATH}`,
    },
  });
  const incrementalHead = run("git", ["rev-parse", "HEAD"], { cwd: workPath }).stdout.trim();
  const capturedIncrementalPrompt = fs.readFileSync(incrementalPromptCapture, "utf8");
  const incrementalReportBody = fs.readFileSync(incrementalBody, "utf8");
  assert.match(incrementalRun.stdout, new RegExp(`using Incremental review from ${fullReviewedHead}`));
  assert.match(capturedIncrementalPrompt, /Review mode: Incremental\./);
  assert.match(capturedIncrementalPrompt, new RegExp(`Review base: ${fullReviewedHead}`));
  assert.match(capturedIncrementalPrompt, /Do not reopen unchanged earlier branch work/);
  assert.match(capturedIncrementalPrompt, /ci-fix\.js \| class=reviewable/);
  assert.doesNotMatch(capturedIncrementalPrompt, /README\.md \| class=/);
  assert.match(incrementalReportBody, new RegExp(`Review-Mode: Incremental\\nReview-Base: ${fullReviewedHead}\\n${reviewedHeadMarker(incrementalHead)}`));
  assert.match(fs.readFileSync(incrementalStatusCapture, "utf8"), new RegExp(`statuses/${incrementalHead}`));
  assert.doesNotMatch(fs.readFileSync(incrementalStatusCapture, "utf8"), new RegExp(`statuses/${fullReviewedHead}`));

  const noChangeRun = run("scripts/agent-pr.sh", ["--owner", "tester", "--verification", "already-reviewed fixture", "--body-file", extraBodyFile], {
    cwd: workPath,
    env: {
      AGENT_GH_API_CAPTURE: noChangeStatusCapture,
      AGENT_GH_PR_BODY_FILE: incrementalBody,
      AGENT_GH_STATUS_SHA: incrementalHead,
      AGENT_PR_BODY_CAPTURE: noChangeBody,
      CODEX_CALLED_MARKER: noChangeCodexCalledMarker,
      GH_BIN: path.join(binPath, "gh"),
      PATH: `${binPath}:${process.env.PATH}`,
    },
  });
  const noChangeReportBody = fs.readFileSync(noChangeBody, "utf8");
  assert.match(noChangeRun.stdout, /skipping Codex, push, and status/);
  assert.equal(fs.existsSync(noChangeCodexCalledMarker), false, "verified current head must not relaunch Codex");
  assert.equal(fs.existsSync(noChangeStatusCapture), false, "verified current head must not repost status");
  assert.match(noChangeReportBody, new RegExp(`Review-Mode: Already Reviewed\\nReview-Base: ${incrementalHead}\\n${reviewedHeadMarker(incrementalHead)}`));
  assert.match(noChangeReportBody, /Captured report body\./, "already-reviewed rerun must preserve the durable report");
  assert.equal((noChangeReportBody.match(/## Fixture caller notes/g) ?? []).length, 1, "already-reviewed rerun must not duplicate caller body content");
  assert.equal((noChangeReportBody.match(/rts-agent-pr:quality-report:v1/g) ?? []).length, 1, "already-reviewed rerun must preserve exactly one bounded report");

  const workflowDryRun = run("scripts/agent-pr.sh", ["--dry-run", "--owner", "tester", "--verification", "dry-run fixture"], {
    cwd: workPath,
    env: {
      GH_BIN: path.join(binPath, "gh"),
      PATH: `${binPath}:${process.env.PATH}`,
    },
  });
  const dryPreflightMarker = "agent-pr preflight: would run git diff --check origin/main...HEAD";
  const dryInitialPreflight = workflowDryRun.stdout.indexOf(dryPreflightMarker);
  const dryReview = workflowDryRun.stdout.indexOf("quality-pass: would run codex");
  const dryFinalPreflight = workflowDryRun.stdout.indexOf(dryPreflightMarker, dryInitialPreflight + 1);
  const dryPush = workflowDryRun.stdout.indexOf("quality-pass: would push HEAD");
  const dryStatus = workflowDryRun.stdout.indexOf("quality-pass: would post adversarial-quality-pass status");
  const dryPrLifecycle = workflowDryRun.stdout.indexOf("agent-pr: would run:");
  assert(
    dryInitialPreflight >= 0 &&
      dryReview > dryInitialPreflight &&
      dryFinalPreflight > dryReview &&
      dryPush > dryFinalPreflight &&
      dryStatus > dryPush &&
      dryPrLifecycle > dryStatus,
  );

  run("git", ["checkout", "main"], { cwd: workPath });
  run("git", ["checkout", "-b", "zvorygin/protected-map-reject"], { cwd: workPath });
  fs.writeFileSync(path.join(workPath, "protected-map-candidate.js"), "export const candidate = true;\n");
  run("git", ["add", "protected-map-candidate.js"], { cwd: workPath });
  run("git", ["commit", "-m", "Branch for protected map rejection"], { cwd: workPath });
  const protectedMapFailure = spawnSync(
    "scripts/agent-pr.sh",
    ["--owner", "tester", "--verification", "protected map fixture"],
    {
      cwd: workPath,
      encoding: "utf8",
      env: testEnv({
        AGENT_GH_API_CAPTURE: protectedMapStatusCapture,
        AGENT_PR_BODY_CAPTURE: protectedMapBody,
        CODEX_CALLED_MARKER: protectedMapCodexCalledMarker,
        CODEX_MUTATE_PROTECTED_MAP: "1",
        GH_BIN: path.join(binPath, "gh"),
        PATH: `${binPath}:${process.env.PATH}`,
      }),
    },
  );
  assert.notEqual(protectedMapFailure.status, 0, "quality pass must reject bundled map mutations");
  assert.match(
    `${protectedMapFailure.stdout}\n${protectedMapFailure.stderr}`,
    /quality pass must not edit bundled map assets[\s\S]*server\/assets\/maps\/quality-pass-mutation\.json/,
  );
  assert.match(fs.readFileSync(protectedMapCodexCalledMarker, "utf8"), /codex called/);
  assert.equal(fs.existsSync(protectedMapStatusCapture), false, "protected map mutation must not post status");
  assert.equal(fs.existsSync(protectedMapBody), false, "protected map mutation must not create a PR");
  assert.equal(
    run("git", ["log", "-1", "--format=%s"], { cwd: workPath }).stdout.trim(),
    "Branch for protected map rejection",
    "protected map mutation must not be committed",
  );
  fs.rmSync(path.join(workPath, "server", "assets", "maps", "quality-pass-mutation.json"));

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

  const docsOnlyRun = run("scripts/agent-pr.sh", ["--owner", "tester", "--title", "Docs-only quality skip", "--verification", "docs fixture"], {
    cwd: workPath,
    env: {
      AGENT_GH_API_CAPTURE: docsOnlyStatusCapture,
      AGENT_PR_BODY_CAPTURE: docsOnlyBody,
      CODEX_CALLED_MARKER: docsOnlyCodexCalledMarker,
      GH_BIN: path.join(binPath, "gh"),
      PATH: `${binPath}:${process.env.PATH}`,
    },
  });

  assert.equal(docsOnlyRun.stdout.split(preflightMarker).length - 1, 1);
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

  run("git", ["checkout", "main"], { cwd: workPath });
  run("git", ["checkout", "-b", "zvorygin/final-head-preflight-reject"], { cwd: workPath });
  fs.writeFileSync(path.join(workPath, "candidate.js"), "export const candidate = true;\n");
  run("git", ["add", "candidate.js"], { cwd: workPath });
  run("git", ["commit", "-m", "Branch for invalid final head"], { cwd: workPath });
  const finalHeadFailure = spawnSync(
    "scripts/agent-pr.sh",
    ["--owner", "tester", "--verification", "final-head fixture"],
    {
      cwd: workPath,
      encoding: "utf8",
      env: testEnv({
        AGENT_GH_API_CAPTURE: finalHeadStatusCapture,
        AGENT_PR_BODY_CAPTURE: finalHeadBody,
        CODEX_CALLED_MARKER: finalHeadCodexCalledMarker,
        CODEX_CREATE_INVALID_FINAL_HEAD: "1",
        GH_BIN: path.join(binPath, "gh"),
        PATH: `${binPath}:${process.env.PATH}`,
      }),
    },
  );
  assert.notEqual(
    finalHeadFailure.status,
    0,
    `final-head preflight unexpectedly passed\nstdout:\n${finalHeadFailure.stdout}\nstderr:\n${finalHeadFailure.stderr}`,
  );
  assert.match(`${finalHeadFailure.stdout}\n${finalHeadFailure.stderr}`, /failed `node scripts\/check-source-file-sizes\.mjs`/);
  assert.match(fs.readFileSync(finalHeadCodexCalledMarker, "utf8"), /codex called/);
  assert.equal(fs.existsSync(finalHeadStatusCapture), false, "invalid final head must not post status");
  assert.equal(fs.existsSync(finalHeadBody), false, "invalid final head must not create a PR");
  assert.equal(fs.existsSync(path.join(workPath, "source-size-invalid.js")), true);
  assert.match(run("git", ["log", "-1", "--format=%s"], { cwd: workPath }).stdout, /Run adversarial quality pass/);
  assert.equal(
    run("git", ["ls-remote", "--heads", "origin", "zvorygin/final-head-preflight-reject"], { cwd: workPath }).stdout,
    "",
    "invalid final head must not push the reviewed branch",
  );

  run("git", ["checkout", "main"], { cwd: workPath });
  run("git", ["checkout", "-b", "zvorygin/reviewed-preflight-source"], { cwd: workPath });
  fs.writeFileSync(path.join(workPath, "candidate.js"), "export const candidate = true;\n");
  run("git", ["add", "candidate.js"], { cwd: workPath });
  run("git", ["commit", "-m", "Branch for reviewed preflight source"], { cwd: workPath });
  const rewrittenPreflightFailure = spawnSync(
    "scripts/agent-pr.sh",
    ["--owner", "tester", "--verification", "reviewed-preflight fixture"],
    {
      cwd: workPath,
      encoding: "utf8",
      env: testEnv({
        AGENT_GH_API_CAPTURE: rewrittenPreflightStatusCapture,
        AGENT_PR_BODY_CAPTURE: rewrittenPreflightBody,
        CODEX_CALLED_MARKER: rewrittenPreflightCodexCalledMarker,
        CODEX_REWRITE_PREFLIGHT_TO_REJECT: "1",
        GH_BIN: path.join(binPath, "gh"),
        PATH: `${binPath}:${process.env.PATH}`,
      }),
    },
  );
  assert.notEqual(
    rewrittenPreflightFailure.status,
    0,
    `rewritten final-head preflight unexpectedly passed\nstdout:\n${rewrittenPreflightFailure.stdout}\nstderr:\n${rewrittenPreflightFailure.stderr}`,
  );
  assert.match(`${rewrittenPreflightFailure.stdout}\n${rewrittenPreflightFailure.stderr}`, /fixture rewritten preflight rejection/);
  assert.match(fs.readFileSync(rewrittenPreflightCodexCalledMarker, "utf8"), /codex called/);
  assert.equal(fs.existsSync(rewrittenPreflightStatusCapture), false, "rewritten final preflight must not post status");
  assert.equal(fs.existsSync(rewrittenPreflightBody), false, "rewritten final preflight must not create a PR");
  assert.match(run("git", ["log", "-1", "--format=%s"], { cwd: workPath }).stdout, /Run adversarial quality pass/);
  assert.equal(
    run("git", ["ls-remote", "--heads", "origin", "zvorygin/reviewed-preflight-source"], { cwd: workPath }).stdout,
    "",
    "rewritten final preflight must not push the reviewed branch",
  );
} finally {
  fs.rmSync(tempRoot, { recursive: true, force: true });
}

console.log("adversarial quality pass tests passed");
