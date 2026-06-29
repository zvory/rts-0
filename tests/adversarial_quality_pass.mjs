#!/usr/bin/env node
import assert from "node:assert/strict";
import path from "node:path";

import {
  buildCodexArgs,
  buildFetchArgs,
  markdownReport,
  normalizeReport,
  parseArgs,
  renderPrompt,
  statusDescription,
} from "../scripts/adversarial-quality-pass.mjs";

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
    schemaFile: "/tmp/schema.json",
    reportFile: "/tmp/report.json",
    codexModel: "gpt-5.5",
    prompt: "Review.",
  }),
  [
    "exec",
    "--cd",
    "/tmp/repo",
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

assert.equal(path.basename(parseArgs([]).schemaFile), "adversarial-quality-pass.schema.json");

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

console.log("adversarial quality pass tests passed");
