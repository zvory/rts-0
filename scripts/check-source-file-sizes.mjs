#!/usr/bin/env node
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const baselinePath = path.join(repoRoot, "scripts", "source-file-size-baseline.json");

const MAX_LINES = 1_500;
const INCLUDED_ROOTS = ["server", "client/src", "tests", "scripts"];
const INCLUDED_EXTENSIONS = new Set([".rs", ".js", ".mjs", ".ts"]);
const INCLUDED_FILES = ["client/styles.css"];
const EXCLUDED_PATH_PREFIXES = [
  "server/target/",
  "client/vendor/",
  "tests/node_modules/",
  "node_modules/",
];

const baseline = readBaseline();
const sourceFiles = listSourceFiles();
const { failures, ratchetNotes } = evaluateSizeRatchet(sourceFiles, baseline.exceptions);

if (ratchetNotes.length > 0) {
  console.log("source file size ratchet notes:");
  for (const note of ratchetNotes.sort()) {
    console.log(`  - ${note}`);
  }
}

if (failures.length > 0) {
  console.error("source file size check failed:");
  for (const failure of failures.sort()) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}

const overCap = sourceFiles.filter((file) => file.lines > MAX_LINES).length;
console.log(
  `source file size check passed (${sourceFiles.length} files, ${overCap} frozen exceptions over ${MAX_LINES} lines)`,
);

if (process.argv.includes("--verify")) {
  verifyCssRatchetFixtures();
}

function evaluateSizeRatchet(files, baselineExceptions) {
  const fixtureFailures = [];
  const fixtureNotes = [];
  const exceptions = new Map();

  for (const entry of baselineExceptions) {
    if (exceptions.has(entry.path)) {
      fixtureFailures.push(`baseline repeats exception for ${entry.path}`);
      continue;
    }
    exceptions.set(entry.path, entry);
  }

  const sourceByPath = new Map(files.map((file) => [file.path, file]));
  for (const file of files) {
    const exception = exceptions.get(file.path);
    if (file.lines <= MAX_LINES) {
      if (exception) {
        fixtureNotes.push(
          `${file.path} is now ${file.lines} lines, at or below the ${MAX_LINES}-line cap; remove its baseline exception`,
        );
      }
      continue;
    }
    if (!exception) {
      fixtureFailures.push(`${file.path}: ${file.lines} lines exceeds the ${MAX_LINES}-line cap`);
      continue;
    }
    if (!Number.isInteger(exception.lines) || exception.lines <= MAX_LINES) {
      fixtureFailures.push(`${file.path}: baseline exception must record an integer line count above ${MAX_LINES}`);
    } else if (file.lines > exception.lines) {
      fixtureFailures.push(
        `${file.path}: grew to ${file.lines} lines; frozen exception is ${exception.lines}. Split the file or update the baseline with a reviewable reason.`,
      );
    } else if (file.lines < exception.lines) {
      fixtureNotes.push(
        `${file.path} shrank from frozen exception ${exception.lines} to ${file.lines} lines; lower or remove the exception`,
      );
    }
    if (typeof exception.reason !== "string" || exception.reason.trim() === "") {
      fixtureFailures.push(`${file.path}: baseline exception reason must not be blank`);
    }
  }
  for (const entry of baselineExceptions) {
    if (!sourceByPath.has(entry.path)) {
      fixtureNotes.push(`${entry.path} is no longer a tracked source file; remove its baseline exception`);
    }
  }
  return { failures: fixtureFailures, ratchetNotes: fixtureNotes };
}

function verifyCssRatchetFixtures() {
  const cssPath = "client/fixture.css";
  const exception = { path: cssPath, lines: 1_600, reason: "fixture" };
  const unbaselined = evaluateSizeRatchet([{ path: cssPath, lines: 1_501 }], []);
  const growth = evaluateSizeRatchet([{ path: cssPath, lines: 1_601 }], [exception]);
  const shrinkage = evaluateSizeRatchet([{ path: cssPath, lines: 1_550 }], [exception]);
  if (unbaselined.failures.length !== 1 || growth.failures.length !== 1) {
    throw new Error("CSS size fixtures must reject unbaselined oversize files and frozen-count growth");
  }
  if (shrinkage.failures.length !== 0 || shrinkage.ratchetNotes.length !== 1) {
    throw new Error("CSS size fixture shrinkage must pass with one advisory ratchet note");
  }
  console.log("source file size CSS fixture verification passed");
}

function readBaseline() {
  const failures = [];
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(baselinePath, "utf8"));
  } catch (error) {
    console.error(`source file size baseline could not be read: ${error.message}`);
    process.exit(1);
  }

  if (parsed?.schema !== "source-file-size-baseline-v1") {
    failures.push("baseline schema must be source-file-size-baseline-v1");
  }
  if (parsed?.max_lines !== MAX_LINES) {
    failures.push(`baseline max_lines must be ${MAX_LINES}`);
  }
  if (typeof parsed?.reason !== "string" || parsed.reason.trim() === "") {
    failures.push("baseline reason must not be blank");
  }
  if (!Array.isArray(parsed?.exceptions)) {
    failures.push("baseline exceptions must be an array");
    parsed.exceptions = [];
  }
  if (failures.length > 0) {
    console.error("source file size baseline is invalid:");
    for (const failure of failures) {
      console.error(`  - ${failure}`);
    }
    process.exit(1);
  }
  return parsed;
}

function listSourceFiles() {
  const files = [];
  for (const root of INCLUDED_ROOTS) {
    const absRoot = path.join(repoRoot, root);
    if (existsSync(absRoot)) {
      walk(absRoot);
    }
  }
  for (const relPath of INCLUDED_FILES) {
    const absPath = path.join(repoRoot, relPath);
    if (existsSync(absPath) && !isExcluded(relPath)) {
      files.push({ path: relPath, lines: countLines(readFileSync(absPath, "utf8")) });
    }
  }
  return files.sort((a, b) => a.path.localeCompare(b.path));

  function walk(absDir) {
    for (const entry of readdirSync(absDir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const absPath = path.join(absDir, entry.name);
      const relPath = toRepoRelative(absPath);
      if (isExcluded(relPath)) {
        continue;
      }
      if (entry.isDirectory()) {
        walk(absPath);
      } else if (entry.isFile() && INCLUDED_EXTENSIONS.has(path.extname(entry.name))) {
        const text = readFileSync(absPath, "utf8");
        files.push({ path: relPath, lines: countLines(text) });
      }
    }
  }
}

function isExcluded(relPath) {
  return EXCLUDED_PATH_PREFIXES.some((prefix) => relPath === prefix.slice(0, -1) || relPath.startsWith(prefix));
}

function countLines(text) {
  if (text.length === 0) {
    return 0;
  }
  const lines = text.split(/\r?\n/).length;
  return text.endsWith("\n") || text.endsWith("\r\n") ? lines - 1 : lines;
}

function toRepoRelative(absPath) {
  return path.relative(repoRoot, absPath).split(path.sep).join("/");
}
