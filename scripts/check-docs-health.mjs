#!/usr/bin/env node
import { existsSync, lstatSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const contextLimitBytes = 5 * 1024;
const planningPolicyFiles = ["CLAUDE.md", "docs/context/planning.md", "plans/README.md"];
const phaseNumber = "(?:one|two|three|four|five|six|seven|eight|nine|ten|\\d+)";
const fixedPhaseLimitPatterns = [
  new RegExp(
    `\\b(?:no more than|at most|maximum of|exactly)\\s+${phaseNumber}\\s+(?:executable\\s+|implementation\\s+)?phases?\\b`,
    "i"
  ),
  new RegExp(
    `\\b(?:name|plan|include|create)\\s+only\\s+(?:the\\s+next\\s+)?${phaseNumber}(?:\\s+or\\s+${phaseNumber})?\\s+(?:evidence-backed\\s+|executable\\s+|implementation\\s+)?phases?\\b`,
    "i"
  ),
];
const errors = [];

function repoPath(...parts) {
  return path.join(repoRoot, ...parts);
}

function toRepoRelative(absPath) {
  return path.relative(repoRoot, absPath).split(path.sep).join("/");
}

function listFiles(rootDir, predicate) {
  const absRoot = repoPath(rootDir);
  const files = [];
  if (!existsSync(absRoot)) {
    return files;
  }
  const stack = [absRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    const entries = readdirSync(current, { withFileTypes: true }).sort((a, b) =>
      a.name.localeCompare(b.name)
    );
    for (const entry of entries) {
      const absPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(absPath);
      } else if (entry.isFile() && predicate(absPath)) {
        files.push(absPath);
      }
    }
  }
  return files.sort();
}

function addError(message) {
  errors.push(message);
}

function validateDocMap() {
  const mapPath = repoPath("docs", "doc-map.json");
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(mapPath, "utf8"));
  } catch (error) {
    addError(`docs/doc-map.json does not parse: ${error.message}`);
    return;
  }

  if (!Object.hasOwn(parsed, "version")) {
    addError("docs/doc-map.json is missing required field version");
  }
  if (!Array.isArray(parsed.routes)) {
    addError("docs/doc-map.json routes must be an array");
    return;
  }

  const routeKeys = new Map();

  parsed.routes.forEach((route, index) => {
    const routeNumber = index + 1;
    const sources = route?.source;
    const docs = route?.docs;

    if (!Array.isArray(sources) || sources.length === 0) {
      addError(`docs/doc-map.json route ${routeNumber} must include at least one source entry`);
    }
    if (!Array.isArray(docs) || docs.length === 0) {
      addError(`docs/doc-map.json route ${routeNumber} must include at least one docs entry`);
    }

    const sourceList = Array.isArray(sources) ? sources : [];
    const docList = Array.isArray(docs) ? docs : [];
    for (const [field, values] of [["source", sourceList], ["docs", docList]]) {
      const seen = new Set();
      values.forEach((value) => {
        if (typeof value !== "string" || value.trim() === "") {
          addError(`docs/doc-map.json route ${routeNumber} has a blank or non-string ${field} entry`);
          return;
        }
        if (seen.has(value)) {
          addError(`docs/doc-map.json route ${routeNumber} repeats ${field} entry ${value}`);
        }
        seen.add(value);
      });
    }

    for (const doc of docList) {
      const absDoc = repoPath(doc);
      if (!existsSync(absDoc)) {
        addError(`docs/doc-map.json route ${routeNumber} references missing doc ${doc}`);
      } else if (!lstatSync(absDoc).isFile()) {
        addError(`docs/doc-map.json route ${routeNumber} references non-file doc ${doc}`);
      }
    }

    const key = `${[...sourceList].sort().join("\0")} -> ${[...docList].sort().join("\0")}`;
    const previous = routeKeys.get(key);
    if (previous !== undefined) {
      addError(`docs/doc-map.json route ${routeNumber} duplicates route ${previous}`);
    } else {
      routeKeys.set(key, routeNumber);
    }
  });
}

function validateContextCapsuleSizes() {
  for (const absPath of listFiles("docs/context", (file) => file.endsWith(".md"))) {
    const size = readFileSync(absPath).byteLength;
    if (size > contextLimitBytes) {
      addError(
        `${toRepoRelative(absPath)} is ${size} bytes; capsules must be <= ${contextLimitBytes} bytes`
      );
    }
  }
}

function validatePlanningPolicyHasNoFixedPhaseLimit() {
  for (const relPath of planningPolicyFiles) {
    const text = readFileSync(repoPath(relPath), "utf8");
    for (const pattern of fixedPhaseLimitPatterns) {
      const match = pattern.exec(text);
      if (match) {
        const line = text.slice(0, match.index).split("\n").length;
        addError(
          `${relPath}:${line} imposes a fixed phase-count limit; let actual scope determine the phase count`
        );
      }
    }
  }
}

function stripMarkdownLinkTarget(rawTarget) {
  const trimmed = rawTarget.trim();
  if (trimmed.startsWith("<") && trimmed.endsWith(">")) {
    return trimmed.slice(1, -1).trim();
  }
  return trimmed;
}

function isExternalOrAnchor(target) {
  return (
    target === "" ||
    target.startsWith("#") ||
    /^[a-z][a-z0-9+.-]*:/i.test(target) ||
    target.startsWith("//")
  );
}

function validateMarkdownLinks() {
  const markdownFiles = [
    ...listFiles("docs", (file) => file.endsWith(".md")),
    ...listFiles("plans", (file) => file.endsWith(".md")),
  ];
  const linkPattern = /!?\[[^\]\n]*\]\(([^)\n]+)\)/g;

  for (const absPath of markdownFiles) {
    const text = readFileSync(absPath, "utf8");
    const relPath = toRepoRelative(absPath);
    const lines = text.split("\n");
    lines.forEach((line, lineIndex) => {
      for (const match of line.matchAll(linkPattern)) {
        const target = stripMarkdownLinkTarget(match[1]);
        if (isExternalOrAnchor(target)) {
          continue;
        }
        const withoutAnchor = target.split("#", 1)[0].split("?", 1)[0];
        if (!withoutAnchor.endsWith(".md")) {
          continue;
        }
        const resolved = path.resolve(path.dirname(absPath), decodeURIComponent(withoutAnchor));
        if (!resolved.startsWith(`${repoRoot}${path.sep}`) && resolved !== repoRoot) {
          addError(`${relPath}:${lineIndex + 1} links outside the repository: ${target}`);
          continue;
        }
        if (!existsSync(resolved)) {
          addError(`${relPath}:${lineIndex + 1} links to missing ${target}`);
        } else if (!lstatSync(resolved).isFile()) {
          addError(`${relPath}:${lineIndex + 1} links to non-file ${target}`);
        }
      }
    });
  }
}

validateDocMap();
validateContextCapsuleSizes();
validatePlanningPolicyHasNoFixedPhaseLimit();
validateMarkdownLinks();

if (errors.length > 0) {
  console.error("docs health check failed:");
  for (const error of errors) {
    console.error(`  - ${error}`);
  }
  process.exit(1);
}

console.log("docs health check passed");
