import assert from "node:assert/strict";
import fs from "node:fs";

const repoRoot = new URL("../../", import.meta.url);
const app = fs.readFileSync(new URL("client/src/app.js", repoRoot), "utf8");
const labPanel = fs.readFileSync(new URL("client/src/lab_panel.js", repoRoot), "utf8");
const clientProtocol = fs.readFileSync(new URL("client/src/protocol.js", repoRoot), "utf8");
const serverProtocol = fs.readFileSync(new URL("server/crates/protocol/src/lib.rs", repoRoot), "utf8");
const serverLab = fs.readFileSync(new URL("server/src/lobby/room_task/lab.rs", repoRoot), "utf8");

assert.match(labPanel, /Edit map/);
assert.match(app, /openCurrentLabMapInEditor/);
assert.match(app, /labClient\?\.exportMap\(\)/, "Lab-to-editor transfer requests only authoritative map data");
assert.doesNotMatch(labPanel, /LabMapEditorPanel|mapEditorSession|Restart test with this draft/);
assert.doesNotMatch(clientProtocol, /applyMapDraft/);
assert.doesNotMatch(serverProtocol, /ApplyMapDraft\s*\{\s*draft/);
assert.match(serverLab, /config\.map_draft/);
assert.equal(fs.existsSync(new URL("client/src/lab_map_editor_panel.js", repoRoot)), false);
assert.equal(fs.existsSync(new URL("client/src/lab_map_reset.js", repoRoot)), false);
