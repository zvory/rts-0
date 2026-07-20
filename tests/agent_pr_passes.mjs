import assert from "node:assert/strict";
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
} finally {
  fs.rmSync(tempRoot, { recursive: true, force: true });
}

console.log("agent PR passes tests passed");
