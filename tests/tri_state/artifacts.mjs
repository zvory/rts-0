import fs from "node:fs";
import path from "node:path";

const DEFAULT_ROOT = path.resolve("server/target/tri-state-scenarios");

function runId() {
  const stamp = new Date().toISOString().replaceAll(":", "").replace(/\.\d+Z$/, "Z");
  return `${stamp}-${process.pid}`;
}

export class ArtifactWriter {
  constructor(scenarioName, options = {}) {
    this.scenarioName = scenarioName;
    this.root = options.root || process.env.RTS_TRI_STATE_ARTIFACT_ROOT || DEFAULT_ROOT;
    this.dir = path.join(this.root, scenarioName, options.runId || runId());
    fs.mkdirSync(this.dir, { recursive: true });
    this.files = new Map();
    for (const name of ["timeline", "remote", "client", "local", "diffs"]) {
      this.files.set(name, path.join(this.dir, `${name}.jsonl`));
      fs.writeFileSync(this.files.get(name), "");
    }
  }

  writeScenario(scenario) {
    fs.writeFileSync(
      path.join(this.dir, "scenario.json"),
      `${JSON.stringify(scenario, null, 2)}\n`,
    );
  }

  append(stream, entry) {
    const file = this.files.get(stream);
    if (!file) throw new Error(`unknown artifact stream: ${stream}`);
    fs.appendFileSync(file, `${stableJson({ at: new Date().toISOString(), ...entry })}\n`);
  }

  timeline(entry) {
    this.append("timeline", entry);
  }

  remote(entry) {
    this.append("remote", entry);
  }

  client(entry) {
    this.append("client", entry);
  }

  local(entry) {
    this.append("local", entry);
  }

  diff(entry) {
    this.append("diffs", entry);
  }

  writeSummary({ status, failure = null, command = null, notes = [] }) {
    const lines = [
      `# ${this.scenarioName}`,
      "",
      `Status: ${status}`,
      "",
    ];
    if (failure) {
      lines.push("## First failure", "", `- ${failure.message || failure}`, "");
      if (failure.step) lines.push(`- Step: ${failure.step}`, "");
    }
    if (command) {
      lines.push("## Reproduction", "", "```sh", command, "```", "");
    }
    if (notes.length > 0) {
      lines.push("## Notes", "", ...notes.map((note) => `- ${note}`), "");
    }
    fs.writeFileSync(path.join(this.dir, "summary.md"), `${lines.join("\n")}\n`);
  }
}

export function stableJson(value) {
  return JSON.stringify(sortKeys(value));
}

function sortKeys(value) {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, sortKeys(value[key])]),
  );
}
