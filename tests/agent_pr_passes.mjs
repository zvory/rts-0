import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { loadPasses, markdownSummary, parseArgs as parseRunnerArgs } from "../scripts/agent-pr-passes.mjs";
import {
  PATCH_NOTE_SCOPE,
  branchSlug,
  buildCodexArgs,
  isGameplayCandidate,
  normalizeDecision,
  parseArgs as parsePatchArgs,
  parseEnvValue,
  parseFragmentChanges,
  renderDiscordMessage,
  renderDiscordPayload,
  renderFragment,
  sendDiscordPatchNote,
} from "../scripts/patch-note-pass.mjs";

const agentPrScript = fs.readFileSync(new URL("../scripts/agent-pr.sh", import.meta.url), "utf8");
assert.doesNotMatch(
  agentPrScript,
  /--deliver-discord/,
  "agent-pr must never deliver patch notes before merge",
);
assert.match(agentPrScript, /--skip-patch-notes/, "agent-pr should expose the conversational patch-note opt-out");
assert.match(agentPrScript, /Patch-Notes: \$PATCH_NOTES_MODE/, "agent-pr should persist the patch-note mode in PR metadata");
assert.match(
  agentPrScript,
  /--skip-patch-notes\) PATCH_NOTES_MODE="skipped-user-request"/,
  "agent-pr should select the canonical patch-note opt-out state",
);
assert.match(
  agentPrScript,
  /rg -q '\^Patch-Notes:\[\[:space:\]\]\*skipped-user-request.*[\s\S]*PATCH_NOTES_MODE="skipped-user-request"/,
  "agent-pr should inherit the canonical patch-note opt-out state",
);
assert.match(
  agentPrScript,
  /\[ "\$PATCH_NOTES_MODE" = "skipped-user-request" \]/,
  "agent-pr should pass the canonical patch-note opt-out state to specialist passes",
);
assert.doesNotMatch(
  fs.readFileSync(new URL("../scripts/wait-pr.sh", import.meta.url), "utf8"),
  /--deliver-discord/,
  "wait-pr must not race the post-merge GitHub delivery workflow",
);
assert.equal(parseRunnerArgs(["--base", "upstream/main", "--dry-run"]).baseRef, "upstream/main");
assert.equal(parseRunnerArgs(["--base", "upstream/main", "--dry-run"]).dryRun, true);
assert.equal(parsePatchArgs(["--codex-model", "small-model"]).codexModel, "small-model");
assert.equal(parsePatchArgs([], { RTS_PATCH_NOTES_USER_OPT_OUT: "1" }).userOptOut, true);
assert.equal(parsePatchArgs([], { RTS_PATCH_NOTES_USER_OPT_OUT: "0" }).userOptOut, false);
assert.equal(parsePatchArgs(["--user-opt-out"], {}).userOptOut, true);
assert.equal(parsePatchArgs(["--deliver-discord"]).deliverDiscord, true);
assert.equal(parsePatchArgs(["--delivery-ref", "abc123"]).deliveryRef, "abc123");
assert.deepEqual(parsePatchArgs(["--delivery-path", "patch-notes/note.md"]).deliveryPaths, ["patch-notes/note.md"]);
assert.equal(branchSlug("zvorygin/at-gun/range"), "at-gun-range");
assert.deepEqual(
  buildCodexArgs({
    repoRoot: "/tmp/repo",
    schemaFile: "/tmp/schema.json",
    outputFile: "/tmp/output.json",
    codexModel: "small-model",
  }),
  [
    "exec",
    "--cd",
    "/tmp/repo",
    "--sandbox",
    "read-only",
    "-c",
    'approval_policy="never"',
    "--ephemeral",
    "--output-schema",
    "/tmp/schema.json",
    "--output-last-message",
    "/tmp/output.json",
    "--model",
    "small-model",
    "-",
  ],
);

const normalizedPatchNoteScope = PATCH_NOTE_SCOPE.replace(/\s+/g, " ");
assert.match(normalizedPatchNoteScope, /only when the branch changes the experience of an active participant playing an ordinary live match/);
assert.match(normalizedPatchNoteScope, /spectators, observers, casters, observer analysis, replays or replay playback/);
assert.match(normalizedPatchNoteScope, /Convenience and presentation changes outside an active player's live-match experience are not gameplay/);
assert.match(normalizedPatchNoteScope, /runtime source path is only a reason to inspect the diff, not evidence of gameplay impact/);

assert.equal(isGameplayCandidate("server/crates/rules/src/balance/support_weapons.rs"), true);
assert.equal(isGameplayCandidate("server/crates/ai/src/ai_core/decision/production.rs"), true);
assert.equal(isGameplayCandidate("server/src/lobby/room_task/lobby.rs"), true);
assert.equal(isGameplayCandidate("client/src/config/rules_mirror.js"), true);
assert.equal(isGameplayCandidate("client/src/renderer/entities.js"), true);
assert.equal(isGameplayCandidate("client/src/lobby_view.js"), true);
assert.equal(isGameplayCandidate("client/styles.css"), true);
assert.equal(isGameplayCandidate("tests/client_contracts/protocol_contracts.mjs"), false);
assert.equal(isGameplayCandidate("docs/design/balance.md"), false);
assert.equal(isGameplayCandidate("scripts/agent-pr.sh"), false);

const decision = normalizeDecision({
  decision: "write_patch_note",
  title: "Longer-ranged anti-tank guns",
  changes: ["Deployed anti-tank-gun range increased from 20 to 40 tiles."],
  playtest_watch: ["Watch whether the larger firing zone is too easy to protect."],
  reason: "The authoritative and mirrored range constants doubled.",
});
assert.equal(decision.playtestWatch.length, 1);
assert.equal(
  renderDiscordMessage(decision),
  "• Deployed anti-tank-gun range increased from 20 to 40 tiles.",
);
assert.deepEqual(
  JSON.parse(renderDiscordPayload("@everyone changed")),
  { content: "@everyone changed", allowed_mentions: { parse: [] } },
);
assert.equal(
  parseEnvValue("OTHER=value\nRTS_PATCH_NOTES_DISCORD_WEBHOOK_URL='https://example.invalid/hook'\n", "RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL"),
  "https://example.invalid/hook",
);
assert.deepEqual(
  parseFragmentChanges("# Note\n\n## Changes\n\n- First change.\n- Second change.\n\n## Playtest watch\n\n- Not delivered.\n"),
  ["First change.", "Second change."],
);
assert.match(
  renderFragment({ branch: "zvorygin/at-gun-range", date: "2026-07-20", decision }),
  /patch-notes|Longer-ranged anti-tank guns|20 to 40 tiles|Playtest watch/s,
);
assert.throws(
  () => normalizeDecision({ decision: "write_patch_note", title: "", changes: [], playtest_watch: [], reason: "" }),
  /requires a title/,
);
assert.deepEqual(
  normalizeDecision({
    decision: "write_patch_note",
    title: "One\nline title",
    changes: ["One\nline change"],
    playtest_watch: [],
    reason: "One\nline reason",
  }),
  {
    decision: "write_patch_note",
    title: "One line title",
    changes: ["One line change"],
    playtestWatch: [],
    reason: "One line reason",
  },
);
const maximumDiscordDecision = normalizeDecision({
  decision: "write_patch_note",
  title: "Bounded changes",
  changes: ["Infantry movement improved.", "Tank movement improved.", "Formation movement improved."],
  playtest_watch: [],
  reason: "Exercise the Discord content limit.",
});
assert.equal(maximumDiscordDecision.changes.length, 3);
assert(renderDiscordMessage(maximumDiscordDecision).length <= 240, "Discord patch notes must fit 240 characters");
assert.equal(
  renderFragment({ branch: "zvorygin/bounded", date: "2026-07-20", decision: maximumDiscordDecision })
    .includes("Formation movement improved."),
  true,
  "the canonical patch-note fragment should contain the bounded Discord changes",
);
assert.throws(
  () => normalizeDecision({
    decision: "write_patch_note",
    title: "Too many changes",
    changes: ["One.", "Two.", "Three.", "Four."],
    playtest_watch: [],
    reason: "Exercise the bullet-count limit.",
  }),
  /allows at most 3 changes/,
);
assert.throws(
  () => normalizeDecision({
    decision: "write_patch_note",
    title: "Too verbose",
    changes: ["x".repeat(236), "y"],
    playtest_watch: [],
    reason: "Exercise the whole-message limit.",
  }),
  /must fit one 240-character Discord message/,
);
const legacyOversizedMessage = renderDiscordMessage({ changes: Array.from({ length: 8 }, () => "x".repeat(300)) });
assert.equal(legacyOversizedMessage, "• Gameplay changed; see the full patch notes for details.");
assert.equal(legacyOversizedMessage.includes("…"), false, "legacy notes should not be cut off mid-sentence");
assert.equal(
  renderDiscordMessage({ changes: ["One.", "Two.", "Three.", "Four."] }),
  "• Gameplay changed; see the full patch notes for details.",
  "legacy notes with extra short bullets should not be silently shortened",
);

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  assert.equal(result.status, 0, `${command} ${args.join(" ")} failed\n${result.stderr || result.stdout}`);
  return result.stdout.trim();
}

const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-agent-pr-passes-test-"));
try {
  const config = path.join(tempRoot, "passes.json");
  fs.writeFileSync(config, JSON.stringify({
    version: 1,
    passes: [{ id: "fixture", command: ["node", "fixture.mjs"], modelEnv: "RTS_FIXTURE_MODEL" }],
  }));
  assert.deepEqual(loadPasses(config), [{
    id: "fixture",
    command: ["node", "fixture.mjs"],
    modelEnv: "RTS_FIXTURE_MODEL",
  }]);
  assert.match(markdownSummary([{ id: "fixture", report: "Decision: no-op" }]), /Agent PR passes.*fixture.*no-op/s);

  run("git", ["init", "-b", "main"], tempRoot);
  const delivered = [];
  const deliveryOptions = {
    branch: "zvorygin/at-gun-range",
    decision,
    env: { RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL: "https://example.invalid/hook" },
    post: (_url, message) => delivered.push(message),
    repoRoot: tempRoot,
  };
  assert.equal(sendDiscordPatchNote(deliveryOptions).status, "sent");
  assert.deepEqual(delivered, ["• Deployed anti-tank-gun range increased from 20 to 40 tiles."]);
  assert.equal(sendDiscordPatchNote(deliveryOptions).status, "unchanged");
  assert.equal(delivered.length, 1, "unchanged patch notes should not be sent twice");
  const movedDelivery = {
    ...deliveryOptions,
    env: { RTS_PATCH_NOTES_DISCORD_WEBHOOK_URL: "https://example.invalid/another-hook" },
  };
  assert.equal(sendDiscordPatchNote(movedDelivery).status, "sent");
  assert.equal(delivered.length, 2, "a new Discord destination should receive the current patch note");

  fs.writeFileSync(config, JSON.stringify({ version: 2, passes: [] }));
  assert.throws(() => loadPasses(config), /version 1/);

  fs.writeFileSync(config, JSON.stringify({
    version: 1,
    passes: [
      { id: "duplicate", command: ["true"] },
      { id: "duplicate", command: ["true"] },
    ],
  }));
  assert.throws(() => loadPasses(config), /duplicate agent PR pass id/);

  fs.writeFileSync(config, JSON.stringify({
    version: 1,
    passes: [{ id: "../unsafe", command: ["true"] }],
  }));
  assert.throws(() => loadPasses(config), /invalid id/);
} finally {
  fs.rmSync(tempRoot, { recursive: true, force: true });
}

const lifecycleRoot = fs.mkdtempSync(path.join(os.tmpdir(), "rts-patch-note-lifecycle-test-"));
const fakeCodex = path.join(os.tmpdir(), `rts-fake-patch-note-codex-${process.pid}.sh`);
try {
  run("git", ["init", "-b", "main"], lifecycleRoot);
  run("git", ["config", "user.email", "qa@example.invalid"], lifecycleRoot);
  run("git", ["config", "user.name", "Patch Note Test"], lifecycleRoot);
  fs.mkdirSync(path.join(lifecycleRoot, "patch-notes", "2026-01-01"), { recursive: true });
  fs.writeFileSync(
    path.join(lifecycleRoot, "patch-notes", "2026-01-01", "stale-note.md"),
    "<!-- rts-patch-note:v1 -->\n<!-- branch: zvorygin/stale-note -->\n# Historical note\n",
  );
  fs.writeFileSync(path.join(lifecycleRoot, "README.md"), "fixture\n");
  run("git", ["add", "-A"], lifecycleRoot);
  run("git", ["commit", "-m", "Base"], lifecycleRoot);
  run("git", ["update-ref", "refs/remotes/origin/main", "HEAD"], lifecycleRoot);
  run("git", ["checkout", "-b", "zvorygin/stale-note"], lifecycleRoot);
  fs.mkdirSync(path.join(lifecycleRoot, "server", "crates", "rules", "src"), { recursive: true });
  fs.writeFileSync(path.join(lifecycleRoot, "server", "crates", "rules", "src", "fixture.rs"), "const RANGE: u32 = 40;\n");
  fs.mkdirSync(path.join(lifecycleRoot, "patch-notes", "2026-07-20"), { recursive: true });
  const staleFragment = path.join(lifecycleRoot, "patch-notes", "2026-07-20", "stale-note.md");
  fs.writeFileSync(
    staleFragment,
    "<!-- rts-patch-note:v1 -->\n<!-- branch: zvorygin/stale-note -->\n# Stale note\n",
  );
  run("git", ["add", "-A"], lifecycleRoot);
  run("git", ["commit", "-m", "Add gameplay change and note"], lifecycleRoot);

  fs.writeFileSync(fakeCodex, `#!/usr/bin/env bash
set -euo pipefail
output=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then output="$2"; shift; fi
  printf '%s\\n' "$1" >> "${fakeCodex}.args"
  shift
done
cat > "${fakeCodex}.stdin"
printf '%s\n' '{"decision":"no_patch_note","title":"","changes":[],"playtest_watch":[],"reason":"The rules edit is not player-facing."}' > "$output"
`, { mode: 0o755 });

  const patchOptions = parsePatchArgs([
    "--base", "origin/main",
    "--head-branch", "zvorygin/stale-note",
    "--codex-command", fakeCodex,
    "--repo", lifecycleRoot,
  ]);
  const { execute } = await import("../scripts/patch-note-pass.mjs");

  const optOutReport = path.join(lifecycleRoot, "opt-out-report.md");
  execute({ ...patchOptions, markdownReportFile: optOutReport, userOptOut: true });
  assert.equal(fs.existsSync(`${fakeCodex}.args`), false, "a user opt-out must bypass Codex classification");
  assert.equal(fs.existsSync(staleFragment), false, "a user opt-out should remove an existing branch-owned fragment");
  assert.match(run("git", ["log", "-1", "--format=%s"], lifecycleRoot), /Remove gameplay patch note by request/);
  assert.match(fs.readFileSync(optOutReport, "utf8"), /Decision: skipped_user_request.*Removed fragment/s);
  fs.rmSync(optOutReport);
  assert.equal(run("git", ["status", "--porcelain=v1"], lifecycleRoot), "");

  fs.mkdirSync(path.dirname(staleFragment), { recursive: true });
  fs.writeFileSync(
    staleFragment,
    "<!-- rts-patch-note:v1 -->\n<!-- branch: zvorygin/stale-note -->\n# Stale note\n",
  );
  run("git", ["add", "patch-notes/2026-07-20/stale-note.md"], lifecycleRoot);
  run("git", ["commit", "-m", "Restore stale note for classification"], lifecycleRoot);

  execute(patchOptions);

  const codexArgs = fs.readFileSync(`${fakeCodex}.args`, "utf8").trim().split("\n");
  assert.deepEqual(
    codexArgs.slice(-1),
    ["-"],
    "patch-note classification should use prompt-driven read-only execution compatible with custom instructions",
  );
  assert.equal(
    codexArgs.some((arg) => arg.includes("Bounded diff") || arg.includes("const RANGE")),
    false,
    "the classifier prompt and branch diff must not be passed as command-line arguments",
  );
  const codexPrompt = fs.readFileSync(`${fakeCodex}.stdin`, "utf8");
  assert.match(codexPrompt, /complete repository diff from origin\/main to HEAD/);
  assert.match(codexPrompt, /use read-only repository\s+inspection only/);
  assert.doesNotMatch(codexPrompt, /do not edit files or run commands/);
  assert.match(codexPrompt, /server\/crates\/rules\/src\/fixture\.rs/);
  assert.doesNotMatch(codexPrompt, /const RANGE/, "the branch diff should be inspected from the repository, not embedded in stdin");

  assert.equal(fs.existsSync(staleFragment), false, "a no-patch-note decision should remove the branch-owned fragment");
  assert.equal(
    fs.existsSync(path.join(lifecycleRoot, "patch-notes", "2026-01-01", "stale-note.md")),
    true,
    "historical base fragments with a reused branch slug must remain untouched",
  );
  assert.match(run("git", ["log", "-1", "--format=%s"], lifecycleRoot), /Remove stale gameplay patch note/);
  assert.equal(run("git", ["status", "--porcelain=v1"], lifecycleRoot), "");

  fs.mkdirSync(path.dirname(staleFragment), { recursive: true });
  fs.writeFileSync(
    staleFragment,
    "<!-- rts-patch-note:v1 -->\n<!-- branch: zvorygin/stale-note -->\n# Final note\n\n## Changes\n\n- Merged factual change.\n",
  );
  run("git", ["add", "patch-notes/2026-07-20/stale-note.md"], lifecycleRoot);
  run("git", ["commit", "-m", "Add final patch note"], lifecycleRoot);
  const deliveryRef = run("git", ["rev-parse", "HEAD"], lifecycleRoot);
  run("git", ["checkout", "main"], lifecycleRoot);
  fs.writeFileSync(path.join(lifecycleRoot, "unrelated.txt"), "delivery must not depend on the checkout\n");
  const delivery = execute(parsePatchArgs([
    "--deliver-discord",
    "--delivery-ref", deliveryRef,
    "--delivery-path", "patch-notes/2026-07-20/stale-note.md",
    "--head-branch", "zvorygin/stale-note",
    "--repo", lifecycleRoot,
    "--dry-run",
  ]));
  assert.deepEqual(delivery.changes, ["Merged factual change."], "delivery should read the immutable merged head");

  const historicalOnly = execute(parsePatchArgs([
    "--deliver-discord",
    "--delivery-ref", deliveryRef,
    "--head-branch", "zvorygin/stale-note",
    "--repo", lifecycleRoot,
    "--dry-run",
  ]));
  assert.equal(historicalOnly, null, "delivery should not rediscover an unchanged historical fragment");

  const deletedFragment = execute(parsePatchArgs([
    "--deliver-discord",
    "--delivery-ref", deliveryRef,
    "--delivery-path", "patch-notes/2025-12-31/stale-note.md",
    "--head-branch", "zvorygin/stale-note",
    "--repo", lifecycleRoot,
    "--dry-run",
  ]));
  assert.equal(deletedFragment, null, "delivery should ignore a changed fragment that is absent at the immutable head");
  run("git", ["checkout", "zvorygin/stale-note"], lifecycleRoot);
  fs.rmSync(path.join(lifecycleRoot, "unrelated.txt"));

  run("git", ["rm", "server/crates/rules/src/fixture.rs"], lifecycleRoot);
  run("git", ["commit", "-m", "Revert gameplay change"], lifecycleRoot);
  fs.mkdirSync(path.dirname(staleFragment), { recursive: true });
  fs.writeFileSync(
    staleFragment,
    "<!-- rts-patch-note:v1 -->\n<!-- branch: zvorygin/stale-note -->\n# Orphaned note\n",
  );
  run("git", ["add", "patch-notes/2026-07-20/stale-note.md"], lifecycleRoot);
  run("git", ["commit", "-m", "Restore orphaned note"], lifecycleRoot);

  execute(patchOptions);

  assert.equal(fs.existsSync(staleFragment), false, "a fragment left after its gameplay diff is reverted should be removed");
  assert.match(run("git", ["log", "-1", "--format=%s"], lifecycleRoot), /Remove stale gameplay patch note/);
  assert.equal(run("git", ["status", "--porcelain=v1"], lifecycleRoot), "");

  const scratchFile = path.join(lifecycleRoot, "uncommitted-scratch.txt");
  fs.writeFileSync(scratchFile, "not part of the branch diff\n");
  const dirtyStatus = run("git", ["status", "--porcelain=v1"], lifecycleRoot);
  execute({ ...patchOptions, dryRun: true });
  assert.equal(
    run("git", ["status", "--porcelain=v1"], lifecycleRoot),
    dirtyStatus,
    "dry-run should allow and preserve unrelated worktree changes",
  );
} finally {
  fs.rmSync(lifecycleRoot, { recursive: true, force: true });
  fs.rmSync(fakeCodex, { force: true });
  fs.rmSync(`${fakeCodex}.args`, { force: true });
  fs.rmSync(`${fakeCodex}.stdin`, { force: true });
}

console.log("agent PR passes tests passed");
