#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  buildCodexExecArgs,
  discoverPhases,
  ensurePrReady,
  enrichHandoffWithPr,
  normalizePhase,
  parseArgs,
  phaseBaseRefForRun,
  phaseMarkedDoneText,
  readCodexSessionExecutorConfig,
  renderPrompt,
  resolveCodexExecutorConfig,
  validateOptions,
  verificationSummary,
  writePrBody,
} from "../scripts/phase-runner-agents.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

assert.equal(normalizePhase("1"), "phase-1");
assert.equal(normalizePhase("phase-5.5"), "phase-5.5");
assert.equal(normalizePhase("3a"), "phase-3a");
assert.throws(() => normalizePhase("phase-x"), /invalid phase/);

const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-phase-runner-agents-"));
try {
  const planDir = path.join(fixtureRoot, "plans", "fixture");
  fs.mkdirSync(planDir, { recursive: true });
  for (const name of [
    "plan.md",
    "phase-1.md",
    "phase-2.md",
    "phase-3.md",
    "phase-3a.md",
    "phase-5.5.md",
    "phase-6.md",
    "phase-7.md",
    "phase-x.md",
  ]) {
    fs.writeFileSync(path.join(planDir, name), `${name}\n`);
  }
  assert.deepEqual(discoverPhases(planDir, "1", "6"), [
    "phase-2",
    "phase-3",
    "phase-3a",
    "phase-5.5",
    "phase-6",
  ]);
  assert.throws(() => discoverPhases(planDir, "6", "1"), /--from must be before --to/);
} finally {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}

assert.equal(phaseMarkedDoneText("Status: Done.\n"), true);
assert.equal(phaseMarkedDoneText("## Status\n\nDone.\n"), true);
assert.equal(phaseMarkedDoneText("## Phase Status\n\n- [x] Done.\n"), true);
assert.equal(phaseMarkedDoneText("## Phase Status\n\n- [x] Done. Manual QA remains.\n"), true);
assert.equal(phaseMarkedDoneText("## Status\n\nDraft.\n"), false);
assert.equal(phaseMarkedDoneText("Status: Done-ish.\n"), false);
assert.equal(phaseMarkedDoneText("Status: Done? No.\n"), false);
assert.equal(phaseBaseRefForRun({ dryRun: true, baseBranch: "main", baseBranchAvailable: true }), "main");
assert.equal(phaseBaseRefForRun({ dryRun: true, baseBranch: "main", baseBranchAvailable: false }), "HEAD");
assert.equal(phaseBaseRefForRun({ dryRun: false, baseBranch: "main", baseBranchAvailable: false }), "main");

const options = parseArgs(["--plan", "svg", "--from", "1", "--to", "2", "--pr", "--wait"]);
validateOptions(options);
assert.equal(options.planName, "svg");
assert.equal(options.waitForPr, true);
const modelOptions = parseArgs(["--plan", "svg", "phase-1", "--model", "gpt-5.4-mini", "--pr"]);
validateOptions(modelOptions);
assert.equal(modelOptions.model, "gpt-5.4-mini");
const nestedOptions = parseArgs(["--plan", "lab/room", "phase-0", "--pr", "--wait"]);
validateOptions(nestedOptions);
assert.equal(nestedOptions.planName, "lab/room");
assert.throws(() => validateOptions(parseArgs(["--plan", "../bad", "1", "--pr"])), /plan name/);
assert.throws(() => validateOptions(parseArgs(["--plan", "bad//path", "1", "--pr"])), /plan name/);
assert.throws(() => validateOptions(parseArgs(["--plan", "svg", "1"])), /PR-first/);
assert.throws(() => parseArgs(["--plan", "svg", "1", "--executor", "agents-sdk", "--pr"]), /unknown option: --executor/);

const prompt = renderPrompt({ planName: "svg", phaseId: "phase-2", branch: "zvorygin/svg-phase-2" });
assert.match(prompt, /\$phase-runner/);
assert.match(prompt, /Plan: plans\/svg\/plan.md/);
assert.match(prompt, /Current branch: zvorygin\/svg-phase-2/);
const nestedPrompt = renderPrompt({ planName: "lab/room", phaseId: "phase-0", branch: "zvorygin/lab/room-phase-0" });
assert.match(nestedPrompt, /Plan: plans\/lab\/room\/plan.md/);
assert.match(nestedPrompt, /Phase: plans\/lab\/room\/phase-0.md/);

const readyPr = [
  {
    number: 123,
    url: "https://github.example/pr/123",
    state: "OPEN",
    autoMergeRequest: { enabledAt: "now" },
    mergeStateStatus: "CLEAN",
    headRefOid: "abc",
  },
];
assert.equal(ensurePrReady(readyPr, "zvorygin/x").number, 123);
assert.throws(() => ensurePrReady([], "zvorygin/x"), /did not leave an open PR/);
assert.throws(() => ensurePrReady([{ ...readyPr[0], autoMergeRequest: null }], "zvorygin/x"), /missing auto-merge/);
assert.throws(() => ensurePrReady([{ ...readyPr[0], mergeStateStatus: "DIRTY" }], "zvorygin/x"), /merge conflicts/);
assert.deepEqual(enrichHandoffWithPr({ status: "completed" }, readyPr, "def", "merged"), {
  status: "completed",
  pr_number: 123,
  pr_url: "https://github.example/pr/123",
  head_sha: "def",
  auto_merge_state: "armed",
  merge_wait_state: "merged",
});

assert.equal(verificationSummary({ verification: ["node test", "", "git diff --check"] }), "node test; git diff --check");
assert.equal(verificationSummary({ verification: [] }), "Focused verification not recorded by executor.");

const sessionRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-phase-runner-codex-home-"));
try {
  const threadId = "019efeed-1111-7222-8333-deadbeefcafe";
  const sessionDir = path.join(sessionRoot, "sessions", "2026", "06", "21");
  const sessionFile = path.join(sessionDir, `rollout-2026-06-21T12-00-00-${threadId}.jsonl`);
  fs.mkdirSync(sessionDir, { recursive: true });
  fs.writeFileSync(
    sessionFile,
    [
      JSON.stringify({ type: "session_meta", payload: { id: threadId, model_provider: "openai" } }),
      JSON.stringify({
        type: "turn_context",
        payload: {
          model: "gpt-5.4-mini",
          collaboration_mode: { settings: { model: "gpt-5.4-mini", reasoning_effort: "high" } },
          effort: "high",
        },
      }),
      JSON.stringify({
        type: "turn_context",
        payload: {
          model: "gpt-5.5",
          collaboration_mode: { settings: { model: "gpt-5.5", reasoning_effort: "xhigh" } },
          effort: "xhigh",
        },
      }),
      "",
    ].join("\n"),
  );

  assert.deepEqual(readCodexSessionExecutorConfig(sessionFile), {
    model: "gpt-5.5",
    reasoningEffort: "xhigh",
    source: "codex-session",
  });
  assert.deepEqual(resolveCodexExecutorConfig({ explicitModel: "gpt-5.4-mini", env: { CODEX_HOME: sessionRoot, CODEX_THREAD_ID: threadId } }), {
    model: "gpt-5.4-mini",
    reasoningEffort: "",
    source: "explicit",
  });
  assert.deepEqual(resolveCodexExecutorConfig({ env: { CODEX_MODEL: "gpt-5.3-codex-spark", CODEX_REASONING_EFFORT: "low" } }), {
    model: "gpt-5.3-codex-spark",
    reasoningEffort: "low",
    source: "env",
  });
  assert.deepEqual(resolveCodexExecutorConfig({ env: { CODEX_HOME: sessionRoot, CODEX_THREAD_ID: threadId } }), {
    model: "gpt-5.5",
    reasoningEffort: "xhigh",
    source: "codex-session",
  });
  assert.deepEqual(resolveCodexExecutorConfig({ env: { CODEX_HOME: sessionRoot, CODEX_THREAD_ID: "missing" } }), {
    model: "",
    reasoningEffort: "",
    source: "codex-default",
  });
} finally {
  fs.rmSync(sessionRoot, { recursive: true, force: true });
}

assert.deepEqual(
  buildCodexExecArgs({
    worktreePath: "/tmp/worktree",
    gitCommonDir: "/tmp/git-common",
    schemaFile: "/tmp/schema.json",
    handoffFile: "/tmp/handoff.json",
    executorConfig: { model: "gpt-5.5", reasoningEffort: "xhigh" },
    prompt: "Do the phase.",
  }),
  [
    "exec",
    "--cd",
    "/tmp/worktree",
    "--add-dir",
    "/tmp/git-common",
    "--sandbox",
    "workspace-write",
    "--output-schema",
    "/tmp/schema.json",
    "--output-last-message",
    "/tmp/handoff.json",
    "--model",
    "gpt-5.5",
    "--config",
    'model_reasoning_effort="xhigh"',
    "Do the phase.",
  ],
);

const bodyPath = path.join(os.tmpdir(), `phase-runner-body-${process.pid}.md`);
try {
  writePrBody(
    {
      status: "completed",
      summary: "Summary.",
      files_changed: ["a.md"],
      verification: ["node tests/phase_runner_agents.mjs"],
      gameplay_impact: "None.",
      next_executor_notes: "Next.",
      manual_test_notes: "Manual.",
    },
    bodyPath,
  );
  const body = fs.readFileSync(bodyPath, "utf8");
  assert.match(body, /## Phase runner handoff/);
  assert.match(body, /### Focused verification/);
  assert.match(body, /node tests\/phase_runner_agents.mjs/);
} finally {
  fs.rmSync(bodyPath, { force: true });
}

const dryRunOutput = execFileSync(
  "node",
  ["scripts/phase-runner-agents.mjs", "--plan", "archive/svg", "phase-0", "phase-1", "--pr", "--dry-run"],
  { cwd: repoRoot, encoding: "utf8", env: { ...process.env, CODEX_THREAD_ID: "", CODEX_MODEL: "" } },
);
assert.match(dryRunOutput, /phase-runner: creating .*svg-phase-0/);
assert.match(dryRunOutput, /phase-runner: would run Codex/);
assert.match(dryRunOutput, /would stop with a pending handoff/);
assert.doesNotMatch(dryRunOutput, /svg-phase-1/);

const waitDryRunOutput = execFileSync(
  "node",
  ["scripts/phase-runner-agents.mjs", "--plan", "archive/svg", "--from", "0", "--to", "1", "--pr", "--wait", "--dry-run"],
  { cwd: repoRoot, encoding: "utf8", env: { ...process.env, CODEX_THREAD_ID: "", CODEX_MODEL: "" } },
);
assert.match(waitDryRunOutput, /phase-runner: discovered phases: phase-1/);
assert.match(waitDryRunOutput, /would run scripts\/wait-pr.sh/);

const nestedDryRunOutput = execFileSync(
  "node",
  ["scripts/phase-runner-agents.mjs", "--plan", "archive/lab/room", "phase-0", "--pr", "--dry-run"],
  { cwd: repoRoot, encoding: "utf8", env: { ...process.env, CODEX_THREAD_ID: "", CODEX_MODEL: "" } },
);
assert.match(nestedDryRunOutput, /phase-runner: creating .*archive-lab-room-phase-0/);
assert.match(nestedDryRunOutput, /would push zvorygin\/archive\/lab\/room-phase-0 to origin/);

console.log("phase runner agents tests passed");
