import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

export class LabInteractTestArtifacts {
  constructor(workspaceRoot) {
    this.root = path.join(workspaceRoot, "target", "lab-interact");
    this.artifactDirectory = path.join(this.root, "artifacts");
    this.ownedFiles = new Set();
    this.ownedSessionDirectories = new Set();
  }

  ownSession(sessionId) {
    assert.match(sessionId, /^lab_[a-f0-9]{32}$/, "test-owned Lab session ids stay unguessable");
    this.ownedSessionDirectories.add(path.join(this.root, sessionId));
    return sessionId;
  }

  ownPortableArtifact(result) {
    assert.match(result?.artifactId || "", /^artifact_[a-f0-9]{32}$/, "portable test artifacts use UUID ids");
    for (const filePath of [result.path, result.sidecarPath]) {
      assert.equal(path.dirname(filePath), this.artifactDirectory, "test-owned portable artifacts stay in the shared artifact directory");
      this.ownedFiles.add(filePath);
    }
    return result;
  }

  cleanup() {
    for (const filePath of this.ownedFiles) fs.rmSync(filePath, { force: true });
    for (const directory of this.ownedSessionDirectories) {
      fs.rmSync(directory, { recursive: true, force: true });
    }
    removeIfEmpty(this.artifactDirectory);
    removeIfEmpty(this.root);
  }

  assertClean() {
    for (const filePath of this.ownedFiles) {
      assert.equal(fs.existsSync(filePath), false, `test-owned artifact was cleaned: ${path.basename(filePath)}`);
    }
    for (const directory of this.ownedSessionDirectories) {
      assert.equal(fs.existsSync(directory), false, `test-owned session artifacts were cleaned: ${path.basename(directory)}`);
    }
  }
}

function removeIfEmpty(directory) {
  if (fs.existsSync(directory) && fs.readdirSync(directory).length === 0) fs.rmdirSync(directory);
}
