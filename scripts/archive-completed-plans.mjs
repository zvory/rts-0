#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { phaseMarkedDoneText } from "./plan-phase-status.mjs";

export { phaseMarkedDoneText };

export function planNameFromActivePhasePath(file) {
  const normalized = file.split(path.sep).join("/");
  const match = /^plans\/(?!archive(?:\/|$))(.+)\/phase-[^/]+\.md$/.exec(normalized);
  return match?.[1] || "";
}

function git(repoRoot, args, options = {}) {
  return execFileSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.stdio || ["ignore", "pipe", "pipe"],
  }).trim();
}

function baseFileText(repoRoot, baseRef, file) {
  try {
    return git(repoRoot, ["show", `${baseRef}:${file}`]);
  } catch {
    return "";
  }
}

function phaseFilesUnder(planDir) {
  const files = [];
  const visit = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const entryPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        visit(entryPath);
      } else if (entry.isFile() && /^phase-[^/]+\.md$/.test(entry.name)) {
        files.push(entryPath);
      }
    }
  };
  visit(planDir);
  return files.sort();
}

export function findArchivablePlans({ repoRoot, baseRef }) {
  const changedFiles = git(repoRoot, [
    "diff",
    "--name-only",
    "--diff-filter=AM",
    `${baseRef}...HEAD`,
    "--",
    "plans",
  ]).split("\n").filter(Boolean);
  const candidates = new Set();

  for (const file of changedFiles) {
    const planName = planNameFromActivePhasePath(file);
    if (!planName) {
      continue;
    }
    const currentFile = path.join(repoRoot, file);
    if (
      fs.existsSync(currentFile) &&
      phaseMarkedDoneText(fs.readFileSync(currentFile, "utf8")) &&
      !phaseMarkedDoneText(baseFileText(repoRoot, baseRef, file))
    ) {
      candidates.add(planName);
    }
  }

  const qualified = [...candidates].sort().filter((planName) => {
    const planDir = path.join(repoRoot, "plans", planName);
    if (!fs.existsSync(path.join(planDir, "plan.md"))) {
      return false;
    }
    const phaseFiles = phaseFilesUnder(planDir);
    return phaseFiles.length > 0 && phaseFiles.every((file) =>
      phaseMarkedDoneText(fs.readFileSync(file, "utf8"))
    );
  });
  return qualified.filter((planName) =>
    !qualified.some((other) => other !== planName && planName.startsWith(`${other}/`))
  );
}

export function archivePlanDirectories({ repoRoot, planNames, dryRun = false }) {
  const moves = planNames.map((planName) => ({
    planName,
    source: path.join(repoRoot, "plans", planName),
    destination: path.join(repoRoot, "plans", "archive", planName),
  }));

  for (const move of moves) {
    if (fs.existsSync(move.destination)) {
      throw new Error(`archive destination already exists: plans/archive/${move.planName}`);
    }
  }
  if (dryRun) {
    return moves;
  }
  for (const move of moves) {
    fs.mkdirSync(path.dirname(move.destination), { recursive: true });
    fs.renameSync(move.source, move.destination);
  }
  return moves;
}

export function commitArchives({ repoRoot, moves }) {
  if (moves.length === 0) {
    return;
  }
  const paths = moves.flatMap((move) => [
    path.relative(repoRoot, move.source),
    path.relative(repoRoot, move.destination),
  ]);
  git(repoRoot, ["add", "-A", "--", ...paths]);
  const subject = moves.length === 1
    ? `Archive completed plan: ${moves[0].planName}`
    : "Archive completed plans";
  const body = moves.map((move) => `- ${move.planName}`).join("\n");
  execFileSync("git", ["commit", "-m", subject, "-m", body], {
    cwd: repoRoot,
    stdio: "inherit",
  });
}

function parseArgs(argv) {
  const options = { baseRef: "origin/main", commit: false, dryRun: false };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--base") {
      options.baseRef = argv[++index] || "";
    } else if (arg === "--commit") {
      options.commit = true;
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  if (!options.baseRef) {
    throw new Error("--base requires a value");
  }
  return options;
}

function usage() {
  return `Usage: scripts/archive-completed-plans.mjs [options]\n\n` +
    `Archives active plans completed by the current branch. A plan qualifies only when a changed\n` +
    `phase transitions to done relative to the base and every phase in that plan is done.\n\n` +
    `Options:\n` +
    `  --base REF   Comparison base, default: origin/main.\n` +
    `  --commit     Commit qualifying plan moves.\n` +
    `  --dry-run    Report qualifying moves without changing files.\n` +
    `  -h, --help   Show this help.\n`;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(usage());
    return;
  }
  if (options.commit && options.dryRun) {
    throw new Error("--commit and --dry-run cannot be combined");
  }
  const repoRoot = git(process.cwd(), ["rev-parse", "--show-toplevel"]);
  const planNames = findArchivablePlans({ repoRoot, baseRef: options.baseRef });
  const moves = archivePlanDirectories({ repoRoot, planNames, dryRun: options.dryRun });
  if (moves.length === 0) {
    console.log("archive-plans: no newly completed plans found");
    return;
  }
  for (const move of moves) {
    const prefix = options.dryRun ? "would archive" : "archived";
    console.log(`archive-plans: ${prefix} plans/${move.planName} -> plans/archive/${move.planName}`);
  }
  if (options.commit) {
    commitArchives({ repoRoot, moves });
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    main();
  } catch (error) {
    console.error(`archive-plans: ${error.message}`);
    process.exit(1);
  }
}
