import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { loadPasses, markdownSummary, parseArgs as parseRunnerArgs } from "../scripts/agent-pr-passes.mjs";
import {
  branchSlug,
  isGameplayCandidate,
  normalizeDecision,
  parseArgs as parsePatchArgs,
  renderFragment,
} from "../scripts/patch-note-pass.mjs";

assert.equal(parseRunnerArgs(["--base", "upstream/main", "--dry-run"]).baseRef, "upstream/main");
assert.equal(parseRunnerArgs(["--base", "upstream/main", "--dry-run"]).dryRun, true);
assert.equal(parsePatchArgs(["--codex-model", "small-model"]).codexModel, "small-model");
assert.equal(branchSlug("zvorygin/at-gun/range"), "at-gun-range");

assert.equal(isGameplayCandidate("server/crates/rules/src/balance/support_weapons.rs"), true);
assert.equal(isGameplayCandidate("client/src/config/rules_mirror.js"), true);
assert.equal(isGameplayCandidate("tests/client_contracts/protocol_contracts.mjs"), false);
assert.equal(isGameplayCandidate("docs/design/balance.md"), false);

const decision = normalizeDecision({
  decision: "write_patch_note",
  title: "Longer-ranged anti-tank guns",
  changes: ["Deployed anti-tank-gun range increased from 20 to 40 tiles."],
  playtest_watch: ["Watch whether the larger firing zone is too easy to protect."],
  reason: "The authoritative and mirrored range constants doubled.",
});
assert.equal(decision.playtestWatch.length, 1);
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
  shift
done
printf '%s\n' '{"decision":"no_patch_note","title":"","changes":[],"playtest_watch":[],"reason":"The rules edit is not player-facing."}' > "$output"
`, { mode: 0o755 });

  const patchOptions = parsePatchArgs([
    "--base", "origin/main",
    "--head-branch", "zvorygin/stale-note",
    "--codex-command", fakeCodex,
    "--repo", lifecycleRoot,
  ]);
  const { execute } = await import("../scripts/patch-note-pass.mjs");
  execute(patchOptions);

  assert.equal(fs.existsSync(staleFragment), false, "a no-patch-note decision should remove the branch-owned fragment");
  assert.equal(
    fs.existsSync(path.join(lifecycleRoot, "patch-notes", "2026-01-01", "stale-note.md")),
    true,
    "historical base fragments with a reused branch slug must remain untouched",
  );
  assert.match(run("git", ["log", "-1", "--format=%s"], lifecycleRoot), /Remove stale gameplay patch note/);
  assert.equal(run("git", ["status", "--porcelain=v1"], lifecycleRoot), "");

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
} finally {
  fs.rmSync(lifecycleRoot, { recursive: true, force: true });
  fs.rmSync(fakeCodex, { force: true });
}

console.log("agent PR passes tests passed");
