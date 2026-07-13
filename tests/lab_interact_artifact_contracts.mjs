import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { LAB_INTERACT_LIMITS, LabInteractService, validateCommandInput } from "../scripts/lab-interact/command_service.mjs";
import { openLabInteractDriver } from "./fixtures/lab_interact_fake_driver.mjs";
import { LabInteractTestArtifacts } from "./fixtures/lab_interact_test_artifacts.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const testArtifacts = new LabInteractTestArtifacts(root);
let service;

try {
  service = new LabInteractService({ workspaceRoot: root, driverFactory: openLabInteractDriver });
  const opened = await service.execute("open", {});
  const sessionId = testArtifacts.ownSession(opened.sessionId);

  await service.execute("spawn", { sessionId, spawns: [
    { owner: 1, kind: "rifleman", x: 100, y: 100, alias: "shooter" },
    { owner: 2, kind: "rifleman", x: 200, y: 100, alias: "target" },
  ] });

  const setup = testArtifacts.ownPortableArtifact(
    await service.execute("export", { sessionId, kind: "setup", name: "Portable setup", reproduction: true }),
  );
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

  const maximumAliases = Array.from({ length: 400 }, (_, index) => ({
    alias: `A${String(index).padStart(3, "0")}${"x".repeat(28)}`,
    id: 0xffff_ffff - index,
  }));
  const maximumSidecar = `${JSON.stringify({
    schemaVersion: 1,
    artifactId: setup.artifactId,
    kind: "setup",
    artifactFile: path.basename(setup.path),
    aliases: maximumAliases,
    reproduction: null,
  }, null, 2)}\n`;
  assert.equal(LAB_INTERACT_LIMITS.maxAliases, 400, "portable artifacts share the operational alias bound");
  assert.ok(Buffer.byteLength(maximumSidecar) < LAB_INTERACT_LIMITS.maxAliasSidecarBytes, "64 KiB safely admits 400 maximum-length aliases and u32 ids");
  fs.writeFileSync(setup.sidecarPath, maximumSidecar);
  assert.equal((await service.execute("artifact-inspect", { sessionId, artifactId: setup.artifactId })).aliasCount, 400, "artifact inspection accepts the full bounded sidecar");
  const maximumAliasImport = await service.execute("import", { sessionId, kind: "setup", artifactId: setup.artifactId });
  assert.deepEqual(
    {
      count: maximumAliasImport.aliases.stale.count,
      detailed: maximumAliasImport.aliases.stale.details.length,
      truncated: maximumAliasImport.aliases.stale.truncated,
    },
    { count: 400, detailed: LAB_INTERACT_LIMITS.maxResponseDetails, truncated: true },
    "default import output summarizes a maximum-size stale alias reconciliation",
  );
  assert.deepEqual(maximumAliasImport.aliases.restored, { count: 0, details: [], truncated: false });
  assert.equal("result" in maximumAliasImport, false, "default import output omits the raw import result");
  assert.ok(JSON.stringify(maximumAliasImport).length < 2_500, "maximum default alias reconciliation stays compact");
  fs.writeFileSync(setup.sidecarPath, `${JSON.stringify({
    schemaVersion: 1,
    artifactId: setup.artifactId,
    kind: "setup",
    artifactFile: path.basename(setup.path),
    aliases: [{ alias: "shooter", id: 100 }, { alias: "target", id: 101 }],
    reproduction: null,
  }, null, 2)}\n`);

  const imported = await service.execute("import", { sessionId, kind: "setup", artifactId: setup.artifactId });
  assert.deepEqual(imported.aliases.stale, { count: 0, details: [], truncated: false });
  assert.equal(imported.aliases.restored.count, 2);
  const entities = await service.execute("inspect", { sessionId, refs: ["shooter", "target"] });
  assert.deepEqual(entities.entities.map((entity) => entity.id).sort(), [1100, 1101]);

  const detailedImport = await service.execute("import", { sessionId, kind: "setup", artifactId: setup.artifactId, details: true });
  assert.deepEqual(detailedImport.aliases.stale, [], "details=true returns every stale alias row");
  assert.equal(detailedImport.aliases.restored.length, 2, "details=true returns every restored alias row");
  assert.equal(detailedImport.result.entityIdMap.length, 2, "details=true preserves the raw authoritative import result");

  const replay = testArtifacts.ownPortableArtifact(
    await service.execute("export", { sessionId, kind: "replay", name: "Portable replay" }),
  );
  assert.equal(replay.operationCount, 0);
  assert.equal((await service.execute("import", { sessionId, kind: "replay", artifactId: replay.artifactId })).validation.ok, true);

  assert.throws(
    () => validateCommandInput("import", { sessionId, kind: "setup", artifactId: setup.artifactId, path: setup.path }),
    /exactly one/,
  );
  assert.throws(
    () => validateCommandInput("import", { sessionId, kind: "setup", artifactId: setup.artifactId, details: "yes" }),
    /boolean/,
  );
  await assert.rejects(
    service.execute("import", { sessionId, kind: "setup", path: "/etc/passwd" }),
    (error) => error.code === "unsafeArtifactPath",
  );

    console.log("lab interact artifact contracts passed");
} finally {
  await service?.shutdown("test");
  testArtifacts.cleanup();
  testArtifacts.assertClean();
}
