#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";

import {
  DELIVERY_STATUS_CONTEXT,
  deliverMergedPullRequest,
  parseCliArgs,
  postDiscordWithRetry,
  run,
} from "../scripts/deliver-merged-patch-notes.mjs";

const workflow = fs.readFileSync(new URL("../.github/workflows/patch-note-delivery.yml", import.meta.url), "utf8");
assert.match(workflow, /pull_request_target:/);
assert.match(workflow, /schedule:/);
assert.match(workflow, /workflow_dispatch:/);
assert.match(workflow, /ref: \$\{\{ github\.event\.repository\.default_branch \}\}/);
assert.match(workflow, /persist-credentials: false/);
assert.match(workflow, /statuses: write/);
assert.match(workflow, /RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL: \$\{\{ secrets\.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL \}\}/);
assert.match(workflow, /\[ "\$REQUESTED_PR_NUMBER" != "0" \]/, "an omitted numeric dispatch input must reconcile recent merges");
assert.doesNotMatch(workflow, /\bcodex\b|\bopenai\b/i, "delivery workflow must never invoke an LLM");

const waiter = fs.readFileSync(new URL("../scripts/wait-pr.sh", import.meta.url), "utf8");
assert.doesNotMatch(waiter, /deliver_patch_notes|--deliver-discord/, "foreground waiter must not race GitHub delivery");

function managedFragment(branch, changes = ["Enemy arcs are now visible."]) {
  return [
    "<!-- rts-patch-note:v1 -->",
    `<!-- branch: ${branch} -->`,
    "# Fixture",
    "",
    "## Changes",
    "",
    ...changes.map((change) => `- ${change}`),
    "",
  ].join("\n");
}

function fixtureApi({
  files = [
    { filename: "client/src/state.js", status: "modified" },
    { filename: "patch-notes/2026-07-24/fixture.md", status: "added" },
  ],
  fragment = managedFragment("zvorygin/fixture"),
  pulls = [],
  statuses = [],
} = {}) {
  const calls = [];
  const api = async (pathname, options = {}) => {
    calls.push({ pathname, options });
    if (pathname.startsWith("/pulls?")) return pulls;
    if (/^\/pulls\/\d+$/.test(pathname)) {
      const number = Number(pathname.split("/").pop());
      return pulls.find((pull) => pull.number === number);
    }
    if (pathname.startsWith("/pulls/") && pathname.includes("/files?")) {
      return pathname.includes("page=1")
        ? files.map((file) => typeof file === "string" ? { filename: file, status: "modified" } : file)
        : [];
    }
    if (pathname.startsWith("/contents/")) {
      return { encoding: "base64", content: Buffer.from(fragment).toString("base64") };
    }
    if (pathname.startsWith("/commits/") && pathname.includes("/status?")) {
      return { statuses };
    }
    if (pathname.startsWith("/statuses/") && options.method === "POST") return {};
    throw new Error(`unexpected API call ${pathname}`);
  };
  return { api, calls };
}

function mergedPull(number = 17) {
  return {
    number,
    merged_at: "2026-07-24T12:00:00Z",
    head: { ref: "zvorygin/fixture", sha: `head-${number}` },
  };
}

assert.deepEqual(parseCliArgs(["--pr", "17"]), { pullNumber: 17 });
for (const args of [["--pr"], ["--pr", ""], ["--pr", "nope"], ["--pr", "0"], ["--pr", "-1"]]) {
  assert.throws(
    () => parseCliArgs(args),
    /--pr requires a positive integer/,
    `invalid explicit selection ${JSON.stringify(args)} must not fall back to broad reconciliation`,
  );
}

{
  const { api, calls } = fixtureApi();
  const delivered = [];
  const result = await deliverMergedPullRequest({
    api,
    pull: mergedPull(),
    webhookUrl: "https://discord.invalid/hook",
    postDiscord: async ({ message }) => delivered.push(message),
  });
  assert.equal(result.status, "sent");
  assert.deepEqual(delivered, ["• Enemy arcs are now visible."]);
  const statusCall = calls.find((call) => call.pathname === "/statuses/head-17");
  assert.deepEqual(statusCall?.options?.body, {
    state: "success",
    context: DELIVERY_STATUS_CONTEXT,
    description: "Gameplay patch note sent to Discord",
  });
}

{
  const { api, calls } = fixtureApi({
    statuses: [{ context: DELIVERY_STATUS_CONTEXT, state: "success" }],
  });
  let posts = 0;
  const result = await deliverMergedPullRequest({
    api,
    pull: mergedPull(),
    webhookUrl: "https://discord.invalid/hook",
    postDiscord: async () => { posts += 1; },
  });
  assert.equal(result.status, "already-recorded");
  assert.equal(posts, 0);
  assert.equal(calls.some((call) => call.pathname.startsWith("/pulls/17/files")), false);
}

{
  const { api, calls } = fixtureApi({ files: ["client/src/state.js"] });
  const result = await deliverMergedPullRequest({ api, pull: mergedPull() });
  assert.equal(result.status, "no-note");
  assert.equal(
    calls.find((call) => call.pathname === "/statuses/head-17")?.options?.body?.description,
    "No gameplay patch note in merged PR",
  );
}

{
  const merged = mergedPull(21);
  const olderMerged = {
    ...mergedPull(20),
    merged_at: "2026-07-23T12:00:00Z",
  };
  const open = { ...mergedPull(22), merged_at: null };
  const { api } = fixtureApi({
    pulls: [open, merged, olderMerged],
    files: [],
  });
  const logs = [];
  const results = await run({ api, eventPath: "", log: (line) => logs.push(line) });
  assert.deepEqual(results.map((result) => result.number), [20, 21]);
  assert.match(logs[0], /PR #20 no-note/);
}

{
  const pulls = [mergedPull(30), mergedPull(31)];
  const attempted = [];
  const api = async (pathname, options = {}) => {
    if (pathname.startsWith("/pulls?")) return pulls;
    const number = Number(pathname.match(/^\/pulls\/(\d+)\/files/)?.[1]);
    if (number) {
      attempted.push(number);
      if (number === 30) throw new Error("fixture failure");
      return [];
    }
    if (pathname.startsWith("/commits/") && pathname.includes("/status?")) return { statuses: [] };
    if (pathname.startsWith("/statuses/") && options.method === "POST") return {};
    throw new Error(`unexpected API call ${pathname}`);
  };
  const logs = [];
  await assert.rejects(
    run({ api, eventPath: "", log: (line) => logs.push(line) }),
    /1 patch-note reconciliation failure/,
  );
  assert.deepEqual(attempted, [30, 31], "one bad PR must not starve later reconciliation candidates");
  assert.match(logs[0], /PR #30 failed: fixture failure/);
  assert.match(logs[1], /PR #31 no-note/);
}

{
  let attempts = 0;
  const sleeps = [];
  await postDiscordWithRetry({
    webhookUrl: "https://discord.invalid/hook",
    message: "• Fixture",
    fetchImpl: async () => {
      attempts += 1;
      return {
        ok: attempts === 3,
        status: attempts === 3 ? 204 : 503,
        statusText: attempts === 3 ? "No Content" : "Unavailable",
        headers: { get: () => null },
        text: async () => "",
      };
    },
    sleep: async (ms) => sleeps.push(ms),
  });
  assert.equal(attempts, 3);
  assert.deepEqual(sleeps, [500, 1000]);
}

{
  let attempts = 0;
  await assert.rejects(
    postDiscordWithRetry({
      webhookUrl: "https://discord.invalid/hook",
      message: "• Fixture",
      fetchImpl: async () => {
        attempts += 1;
        return {
          ok: false,
          status: 400,
          statusText: "Bad Request",
          headers: { get: () => null },
          text: async () => "invalid payload",
        };
      },
      sleep: async () => {},
    }),
    /400 Bad Request/,
  );
  assert.equal(attempts, 1, "permanent Discord errors must fail without retrying");
}

console.log("patch_note_delivery: trusted non-LLM workflow, deterministic delivery, idempotency, reconciliation, and retry contracts passed");
