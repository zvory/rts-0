#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
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
]);
assert.equal(options.baseRef, "origin/main");
assert.equal(options.headBranch, "zvorygin/example");
assert.equal(options.context, "adversarial-quality-pass");
assert.equal(options.postStatus, true);
assert.equal(options.push, true);

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
  env: { ...process.env, [QUALITY_PASS_ENV]: "1" },
});
assert.equal(nestedAgentPr.status, 2);
assert.match(nestedAgentPr.stderr, /outer helper owns PR lifecycle/);

console.log("adversarial quality pass tests passed");
