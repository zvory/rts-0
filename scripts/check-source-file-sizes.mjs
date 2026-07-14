#!/usr/bin/env node
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const baselinePath = path.join(repoRoot, "scripts", "source-file-size-baseline.json");

const MAX_LINES = 1_500;
const INCLUDED_ROOTS = ["server", "client/src", "tests", "scripts"];
const INCLUDED_EXTENSIONS = new Set([".rs", ".js", ".mjs", ".ts"]);
const EXCLUDED_PATH_PREFIXES = [
  "server/target/",
  "client/vendor/",
  "tests/node_modules/",
  "node_modules/",
];

const failures = [];
const ratchetNotes = [];

const baseline = readBaseline();
const exceptions = new Map();

for (const entry of baseline.exceptions) {
  if (exceptions.has(entry.path)) {
    failures.push(`baseline repeats exception for ${entry.path}`);
    continue;
  }
  exceptions.set(entry.path, entry);
}

const sourceFiles = listSourceFiles();
const sourceByPath = new Map(sourceFiles.map((file) => [file.path, file]));

for (const file of sourceFiles) {
  const exception = exceptions.get(file.path);
  if (file.lines <= MAX_LINES) {
    if (exception) {
      ratchetNotes.push(
        `${file.path} is now ${file.lines} lines, at or below the ${MAX_LINES}-line cap; remove its baseline exception`,
      );
    }
    continue;
  }

  if (!exception) {
    failures.push(`${file.path}: ${file.lines} lines exceeds the ${MAX_LINES}-line cap`);
    continue;
  }

  if (!Number.isInteger(exception.lines) || exception.lines <= MAX_LINES) {
    failures.push(`${file.path}: baseline exception must record an integer line count above ${MAX_LINES}`);
  } else if (file.lines > exception.lines) {
    failures.push(
      `${file.path}: grew to ${file.lines} lines; frozen exception is ${exception.lines}. Split the file or update the baseline with a reviewable reason.`,
    );
  } else if (file.lines < exception.lines) {
    ratchetNotes.push(
      `${file.path} shrank from frozen exception ${exception.lines} to ${file.lines} lines; lower or remove the exception`,
    );
  }

  if (typeof exception.reason !== "string" || exception.reason.trim() === "") {
    failures.push(`${file.path}: baseline exception reason must not be blank`);
  }
}

for (const entry of baseline.exceptions) {
  if (!sourceByPath.has(entry.path)) {
    ratchetNotes.push(`${entry.path} is no longer a tracked source file; remove its baseline exception`);
  }
}

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

function readBaseline() {
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
