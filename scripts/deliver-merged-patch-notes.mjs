#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import {
  branchSlug,
  parseFragmentChanges,
  renderDiscordMessage,
  renderDiscordPayload,
} from "./patch-note-pass.mjs";

export const DELIVERY_STATUS_CONTEXT = "patch-note-delivery";
const DEFAULT_RECONCILIATION_LIMIT = 50;
const DISCORD_ATTEMPTS = 4;

function required(value, label) {
  const normalized = String(value || "").trim();
  if (!normalized) throw new Error(`${label} is required`);
  return normalized;
}

function parseRepository(repository) {
  const [owner, repo, extra] = required(repository, "GITHUB_REPOSITORY").split("/");
  if (!owner || !repo || extra) throw new Error("GITHUB_REPOSITORY must use owner/repo form");
  return { owner, repo };
}

async function responseError(response) {
  const body = await response.text().catch(() => "");
  return new Error(`request failed (${response.status} ${response.statusText})${body ? `: ${body}` : ""}`);
}

export function createGitHubApi({
  fetchImpl = globalThis.fetch,
  repository,
  token,
} = {}) {
  if (typeof fetchImpl !== "function") throw new Error("fetch is unavailable");
  const { owner, repo } = parseRepository(repository);
  const auth = required(token, "GITHUB_TOKEN");
  const base = `https://api.github.com/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}`;
  return async function githubApi(pathname, { method = "GET", body } = {}) {
    const response = await fetchImpl(`${base}${pathname}`, {
      method,
      headers: {
        Accept: "application/vnd.github+json",
        Authorization: `Bearer ${auth}`,
        "Content-Type": "application/json",
        "X-GitHub-Api-Version": "2022-11-28",
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!response.ok) throw await responseError(response);
    if (response.status === 204) return null;
    return response.json();
  };
}

function retryDelayMs(attempt, response) {
  const retryAfter = Number(response?.headers?.get?.("retry-after"));
  if (Number.isFinite(retryAfter) && retryAfter > 0) {
    return Math.min(10_000, Math.ceil(retryAfter * 1000));
  }
  return Math.min(4_000, 500 * (2 ** attempt));
}

export async function postDiscordWithRetry({
  fetchImpl = globalThis.fetch,
  message,
  sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  webhookUrl,
} = {}) {
  if (typeof fetchImpl !== "function") throw new Error("fetch is unavailable");
  const destination = required(webhookUrl, "RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL");
  let lastError = null;
  for (let attempt = 0; attempt < DISCORD_ATTEMPTS; attempt += 1) {
    try {
      const response = await fetchImpl(destination, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: renderDiscordPayload(message),
      });
      if (response.ok) return;
      lastError = await responseError(response);
      if (response.status < 500 && response.status !== 429) {
        lastError.retryable = false;
        throw lastError;
      }
      if (attempt + 1 < DISCORD_ATTEMPTS) await sleep(retryDelayMs(attempt, response));
    } catch (error) {
      lastError = error;
      if (error?.retryable === false) throw error;
      if (attempt + 1 < DISCORD_ATTEMPTS) {
        await sleep(retryDelayMs(attempt));
        continue;
      }
    }
  }
  throw lastError || new Error("Discord delivery failed");
}

async function listPullFiles(api, number) {
  const files = [];
  for (let page = 1; ; page += 1) {
    const batch = await api(`/pulls/${number}/files?per_page=100&page=${page}`);
    files.push(...batch);
    if (batch.length < 100) return files;
  }
}

async function managedFragmentForPull(api, pull) {
  const branch = required(pull?.head?.ref, "pull request head branch");
  const headSha = required(pull?.head?.sha, "pull request head SHA");
  const suffix = `/${branchSlug(branch)}.md`;
  const files = await listPullFiles(api, pull.number);
  const candidates = files
    .filter((file) => file?.status !== "removed")
    .map((file) => file?.filename)
    .filter((filename) =>
      typeof filename === "string" &&
      filename.startsWith("patch-notes/") &&
      filename.endsWith(suffix))
    .sort();
  const matches = [];
  for (const filename of candidates) {
    const content = await api(`/contents/${filename.split("/").map(encodeURIComponent).join("/")}?ref=${encodeURIComponent(headSha)}`);
    if (content?.encoding !== "base64" || typeof content?.content !== "string") {
      throw new Error(`patch-note fragment ${filename} did not return base64 file content`);
    }
    const text = Buffer.from(content.content.replace(/\s+/g, ""), "base64").toString("utf8");
    if (
      text.startsWith("<!-- rts-patch-note:v1 -->\n") &&
      text.includes(`<!-- branch: ${branch} -->`)
    ) {
      matches.push({ filename, text });
    }
  }
  if (matches.length > 1) {
    throw new Error(`multiple managed patch-note fragments changed in PR #${pull.number}`);
  }
  return matches[0] || null;
}

async function deliveryAlreadyRecorded(api, headSha) {
  const combined = await api(`/commits/${encodeURIComponent(headSha)}/status?per_page=100`);
  return Array.isArray(combined?.statuses) && combined.statuses.some(
    (status) => status?.context === DELIVERY_STATUS_CONTEXT && status?.state === "success",
  );
}

async function recordDelivery(api, headSha, description) {
  await api(`/statuses/${encodeURIComponent(headSha)}`, {
    method: "POST",
    body: {
      state: "success",
      context: DELIVERY_STATUS_CONTEXT,
      description: String(description).slice(0, 140),
    },
  });
}

export async function deliverMergedPullRequest({
  api,
  postDiscord = (options) => postDiscordWithRetry(options),
  pull,
  webhookUrl,
} = {}) {
  if (!pull?.merged_at) return { status: "not-merged", number: pull?.number };
  const headSha = required(pull?.head?.sha, "pull request head SHA");
  if (await deliveryAlreadyRecorded(api, headSha)) {
    return { status: "already-recorded", number: pull.number };
  }
  const fragment = await managedFragmentForPull(api, pull);
  if (!fragment) {
    await recordDelivery(api, headSha, "No gameplay patch note in merged PR");
    return { status: "no-note", number: pull.number };
  }
  const changes = parseFragmentChanges(fragment.text);
  if (changes.length === 0) throw new Error(`${fragment.filename} has no change bullets to deliver`);
  const message = renderDiscordMessage({ changes });
  await postDiscord({ message, webhookUrl });
  await recordDelivery(api, headSha, "Gameplay patch note sent to Discord");
  return { status: "sent", number: pull.number, path: fragment.filename };
}

function parseCliArgs(argv) {
  let pullNumber = 0;
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--pr") {
      pullNumber = Number(argv[index + 1]);
      index += 1;
    } else {
      throw new Error(`unknown argument: ${argv[index]}`);
    }
  }
  if (pullNumber && (!Number.isInteger(pullNumber) || pullNumber <= 0)) {
    throw new Error("--pr requires a positive integer");
  }
  return { pullNumber };
}

function eventPullRequest(eventPath) {
  if (!eventPath || !fs.existsSync(eventPath)) return null;
  const event = JSON.parse(fs.readFileSync(eventPath, "utf8"));
  return event?.pull_request || null;
}

async function pullsToReconcile(api, explicitNumber, eventPath) {
  if (explicitNumber) return [await api(`/pulls/${explicitNumber}`)];
  const eventPull = eventPullRequest(eventPath);
  if (eventPull) return [eventPull];
  const closed = await api(`/pulls?state=closed&sort=updated&direction=desc&per_page=${DEFAULT_RECONCILIATION_LIMIT}`);
  return closed.filter((pull) => pull?.merged_at);
}

export async function run({
  api,
  eventPath = process.env.GITHUB_EVENT_PATH,
  explicitNumber = 0,
  log = console.log,
  postDiscord,
  webhookUrl,
} = {}) {
  const pulls = await pullsToReconcile(api, explicitNumber, eventPath);
  const results = [];
  for (const pull of pulls) {
    const result = await deliverMergedPullRequest({ api, postDiscord, pull, webhookUrl });
    results.push(result);
    log(`patch-note-delivery: PR #${pull.number} ${result.status}${result.path ? ` (${result.path})` : ""}`);
  }
  return results;
}

async function main() {
  const { pullNumber } = parseCliArgs(process.argv.slice(2));
  const api = createGitHubApi({
    repository: process.env.GITHUB_REPOSITORY,
    token: process.env.GITHUB_TOKEN,
  });
  await run({
    api,
    explicitNumber: pullNumber,
    webhookUrl: process.env.RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL,
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main().catch((error) => {
    process.stderr.write(`patch-note-delivery: ${error.message}\n`);
    process.exitCode = 1;
  });
}
