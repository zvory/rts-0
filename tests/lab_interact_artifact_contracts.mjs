import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { LabInteractService, validateCommandInput } from "../scripts/lab-interact/command_service.mjs";
import { openLabInteractDriver } from "./fixtures/lab_interact_fake_driver.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const service = new LabInteractService({ workspaceRoot: root, driverFactory: openLabInteractDriver });
const opened = await service.execute("open", {});
const sessionId = opened.sessionId;

await service.execute("spawn", { sessionId, spawns: [
  { owner: 1, kind: "rifleman", x: 100, y: 100, alias: "shooter" },
  { owner: 2, kind: "rifleman", x: 200, y: 100, alias: "target" },
] });

const setup = await service.execute("export", { sessionId, kind: "setup", name: "Portable setup", reproduction: true });
assert.match(setup.artifactId, /^artifact_[a-f0-9]{32}$/);
assert.equal(setup.entityCount, 2);
assert.equal(setup.aliasCount, 2);
assert.equal("checkpointPayload" in setup, false);
assert.ok(fs.realpathSync(setup.path).startsWith(fs.realpathSync(path.join(root, "target", "lab-interact"))));

const inspected = await service.execute("artifact-inspect", { sessionId, artifactId: setup.artifactId });
assert.equal(inspected.kind, "setup");
assert.equal(inspected.validation.ok, true);

fs.writeFileSync(setup.sidecarPath, Buffer.alloc(64 * 1024 + 1, 0x20));
await assert.rejects(
  service.execute("artifact-inspect", { sessionId, artifactId: setup.artifactId }),
  (error) => error.code === "invalidAliasSidecar" && /64 KiB/.test(error.message),
);
fs.writeFileSync(setup.sidecarPath, `${JSON.stringify({
  schemaVersion: 1,
  artifactId: setup.artifactId,
  kind: "setup",
  artifactFile: path.basename(setup.path),
  aliases: [{ alias: "shooter", id: 100 }, { alias: "target", id: 101 }],
  reproduction: null,
}, null, 2)}\n`);

const imported = await service.execute("import", { sessionId, kind: "setup", artifactId: setup.artifactId });
assert.deepEqual(imported.aliases.stale, []);
const entities = await service.execute("inspect", { sessionId, refs: ["shooter", "target"] });
assert.deepEqual(entities.entities.map((entity) => entity.id).sort(), [1100, 1101]);

const replay = await service.execute("export", { sessionId, kind: "replay", name: "Portable replay" });
assert.equal(replay.operationCount, 0);
assert.equal((await service.execute("import", { sessionId, kind: "replay", artifactId: replay.artifactId })).validation.ok, true);

assert.throws(
  () => validateCommandInput("import", { sessionId, kind: "setup", artifactId: setup.artifactId, path: setup.path }),
  /exactly one/,
);
await assert.rejects(
  service.execute("import", { sessionId, kind: "setup", path: "/etc/passwd" }),
  (error) => error.code === "unsafeArtifactPath",
);

await service.shutdown("test");
fs.rmSync(path.dirname(setup.path), { recursive: true, force: true });
console.log("lab interact artifact contracts passed");
