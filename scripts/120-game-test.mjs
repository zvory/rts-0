#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const GAME_COUNT = 120;
const SEEDS_PER_MAP = 5;
const DEFAULT_TICKS = 25_000;
const DEFAULT_CONCURRENCY = Math.min(
  32,
  Math.max(1, os.availableParallelism?.() ?? os.cpus().length),
);

const MAPS = [
  { id: "the-river", name: "The River" },
  { id: "schone-tage", name: "Schone Tage" },
  { id: "1v1", name: "1v1" },
  { id: "crossroads", name: "Crossroads" },
];

let options;
try {
  options = parseArgs(process.argv.slice(2));
} catch (error) {
  console.error(`120 game test: ${error instanceof Error ? error.message : error}`);
  process.exit(2);
}
if (options.help) {
  console.log(usage());
  process.exit(0);
}
const MATCHUPS = createMatchups(options.profiles);

const jobs = createJobs(options);
if (jobs.length * 2 !== GAME_COUNT) {
  throw new Error(`120 game test planned ${jobs.length * 2} games instead of ${GAME_COUNT}`);
}

if (options.dryRun) {
  printPlan(options, jobs);
  process.exit(0);
}

const activeChildren = new Set();
let interrupted = false;
for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => {
    if (interrupted) return;
    interrupted = true;
    console.error(`\n120 game test interrupted by ${signal}; terminating active arena jobs.`);
    for (const child of activeChildren) child.kill();
    process.exitCode = 130;
  });
}

try {
  fs.mkdirSync(options.outDir, { recursive: true });
  const gitSha = commandOutput("git", ["rev-parse", "HEAD"]) || "unknown";
  writeRunConfig(options, gitSha, "running");

  console.log("120 game test");
  console.log(`  games:       ${GAME_COUNT}`);
  console.log(`  arena jobs:  ${jobs.length} (each job side-swaps one seed)`);
  console.log(`  concurrency: ${options.concurrency}`);
  console.log(`  tick cap:    ${options.ticks}`);
  console.log(`  output:      ${options.outDir}`);

  if (!options.skipBuild) {
    console.log("\nBuilding release ai-arena...");
    runChecked("cargo", [
      "build",
      "--manifest-path",
      "server/Cargo.toml",
      "-p",
      "rts-ai",
      "--release",
      "--bin",
      "ai-arena",
    ]);
  }

  const arenaBinary = path.join(
    repoRoot,
    "server",
    "target",
    "release",
    process.platform === "win32" ? "ai-arena.exe" : "ai-arena",
  );
  if (!fs.existsSync(arenaBinary)) {
    throw new Error(`ai-arena binary is missing: ${arenaBinary}`);
  }

  const { pending, resumed } = partitionJobs(jobs, options);
  if (resumed > 0) {
    console.log(`\nResuming ${resumed * 2}/${GAME_COUNT} completed games from existing summaries.`);
  }
  await runPool(pending, options, arenaBinary, resumed);

  const report = aggregateResults(jobs, options, gitSha);
  writeReports(report, options);
  writeRunConfig(options, gitSha, "complete");

  console.log("\n120 game test complete.");
  console.log(`  Markdown: ${path.join(options.outDir, "analysis.md")}`);
  console.log(`  JSON:     ${path.join(options.outDir, "summary.json")}`);
  console.log(`  CSV:      ${path.join(options.outDir, "summary.csv")}`);
} catch (error) {
  for (const child of activeChildren) child.kill();
  try {
    const gitSha = commandOutput("git", ["rev-parse", "HEAD"]) || "unknown";
    writeRunConfig(options, gitSha, "failed", error instanceof Error ? error.message : String(error));
  } catch {
    // Preserve the original failure if the status file cannot be updated.
  }
  console.error(`\n120 game test failed: ${error instanceof Error ? error.message : error}`);
  process.exitCode = interrupted ? 130 : 1;
}

function usage() {
  return `Usage:
  node scripts/120-game-test.mjs <AI2.1 profile> <pre-change Jeff profile> <post-change Jeff profile> [options]

Example:
  node scripts/120-game-test.mjs ai_2_1 jeffs_ai_pre_defense_envelope jeffs_ai

Runs the standard 120 game test:
  - supplied AI 2.1 vs supplied pre-change Jeff
  - supplied post-change Jeff vs supplied AI 2.1
  - supplied post-change Jeff vs supplied pre-change Jeff
  - The River, Schone Tage, 1v1, and Crossroads
  - seeds 0-4 with both player assignments (120 games total)

Options:
  --out-dir DIR       Output directory. Reuse it to resume completed seed jobs.
  --concurrency N     Parallel arena jobs (default: ${DEFAULT_CONCURRENCY}).
  --ticks N           Per-game tick cap (default: ${DEFAULT_TICKS}).
  --skip-build        Use the existing release ai-arena binary.
  --verify-replay     Verify every replay (slower; default skips replay verification).
  --dry-run           Print the fixed test plan without building or running games.
  -h, --help          Show this help.

Default output:
  comparison-results/120-game-test-<UTC timestamp>
`;
}

function parseArgs(argv) {
  const parsed = {
    profiles: null,
    concurrency: DEFAULT_CONCURRENCY,
    ticks: DEFAULT_TICKS,
    outDir: "",
    skipBuild: false,
    verifyReplay: false,
    dryRun: false,
    help: false,
  };
  const positionals = [];
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = () => {
      index += 1;
      if (index >= argv.length || argv[index].startsWith("--")) {
        throw new Error(`${arg} requires a value`);
      }
      return argv[index];
    };
    if (arg === "-h" || arg === "--help") parsed.help = true;
    else if (arg === "--out-dir") parsed.outDir = path.resolve(value());
    else if (arg === "--concurrency") parsed.concurrency = positiveInteger(value(), arg);
    else if (arg === "--ticks") parsed.ticks = positiveInteger(value(), arg);
    else if (arg === "--skip-build") parsed.skipBuild = true;
    else if (arg === "--verify-replay") parsed.verifyReplay = true;
    else if (arg === "--dry-run") parsed.dryRun = true;
    else if (!arg.startsWith("-")) positionals.push(arg);
    else throw new Error(`unknown argument: ${arg}\n\n${usage()}`);
  }
  if (!parsed.help) {
    if (positionals.length !== 3) {
      throw new Error(`120 game test requires exactly 3 profile inputs\n\n${usage()}`);
    }
    const [ai21, preChangeJeff, postChangeJeff] = positionals;
    if (new Set(positionals).size !== 3) {
      throw new Error("the 3 profile inputs must be distinct");
    }
    parsed.profiles = { ai21, preChangeJeff, postChangeJeff };
  }
  if (!parsed.outDir) {
    parsed.outDir = path.join(repoRoot, "comparison-results", `120-game-test-${utcStamp()}`);
  }
  return parsed;
}

function createMatchups(profiles) {
  return [
    {
      id: "ai21-vs-prechange-jeff",
      label: "AI 2.1 vs pre-change Jeff",
      candidate: profiles.ai21,
      baseline: profiles.preChangeJeff,
    },
    {
      id: "postchange-jeff-vs-ai21",
      label: "Post-change Jeff vs AI 2.1",
      candidate: profiles.postChangeJeff,
      baseline: profiles.ai21,
    },
    {
      id: "postchange-jeff-vs-prechange-jeff",
      label: "Post-change Jeff vs pre-change Jeff",
      candidate: profiles.postChangeJeff,
      baseline: profiles.preChangeJeff,
    },
  ];
}

function positiveInteger(raw, flag) {
  const number = Number(raw);
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new Error(`${flag} must be a positive integer`);
  }
  return number;
}

function utcStamp() {
  return new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
}

function createJobs(runOptions) {
  const planned = [];
  for (const matchup of MATCHUPS) {
    for (const map of MAPS) {
      for (let seed = 0; seed < SEEDS_PER_MAP; seed += 1) {
        const outDir = path.join(runOptions.outDir, matchup.id, map.id, `seed-${seed}`);
        planned.push({ matchup, map, seed, outDir });
      }
    }
  }
  return planned;
}

function printPlan(runOptions, plannedJobs) {
  console.log("120 game test dry run");
  console.log(`  games:       ${plannedJobs.length * 2}`);
  console.log(`  arena jobs:  ${plannedJobs.length}`);
  console.log(`  concurrency: ${runOptions.concurrency}`);
  console.log(`  tick cap:    ${runOptions.ticks}`);
  console.log(`  output:      ${runOptions.outDir}`);
  console.log(`  replay:      ${runOptions.verifyReplay ? "verify" : "skip verification"}`);
  for (const matchup of MATCHUPS) {
    console.log(`  matchup:     ${matchup.label}`);
  }
  console.log(`  maps:        ${MAPS.map((map) => map.name).join(", ")}`);
  console.log(`  seeds:       0-${SEEDS_PER_MAP - 1}, side-swapped`);
}

function runChecked(command, args) {
  const result = spawnSync(command, args, { cwd: repoRoot, stdio: "inherit", shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status}`);
  }
}

function commandOutput(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
    shell: false,
  });
  return result.status === 0 ? result.stdout.trim() : "";
}

function partitionJobs(plannedJobs, runOptions) {
  const pending = [];
  let resumed = 0;
  for (const job of plannedJobs) {
    const summaryPath = path.join(job.outDir, "arena-summary.json");
    if (!fs.existsSync(summaryPath)) {
      pending.push(job);
      continue;
    }
    const summary = readArenaSummary(job, runOptions);
    if (summary.runs.length !== 2) {
      throw new Error(`${summaryPath} has ${summary.runs.length} runs; expected 2`);
    }
    resumed += 1;
  }
  return { pending, resumed };
}

async function runPool(pendingJobs, runOptions, arenaBinary, resumedJobs) {
  if (pendingJobs.length === 0) return;
  let nextIndex = 0;
  let completeJobs = resumedJobs;
  let aborted = false;
  const workerCount = Math.min(runOptions.concurrency, pendingJobs.length);

  const worker = async () => {
    while (!aborted && !interrupted) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= pendingJobs.length) return;
      const job = pendingJobs[index];
      try {
        await runArenaJob(job, runOptions, arenaBinary);
        completeJobs += 1;
        console.log(
          `[${String(completeJobs).padStart(2)}/${jobs.length} jobs | ${String(completeJobs * 2).padStart(3)}/${GAME_COUNT} games] ` +
            `${job.matchup.id} / ${job.map.name} / seed ${job.seed}`,
        );
      } catch (error) {
        aborted = true;
        throw error;
      }
    }
  };

  await Promise.all(Array.from({ length: workerCount }, worker));
}

function runArenaJob(job, runOptions, arenaBinary) {
  fs.mkdirSync(job.outDir, { recursive: true });
  const args = [
    "--candidate",
    job.matchup.candidate,
    "--baseline",
    job.matchup.baseline,
    "--seed-start",
    String(job.seed),
    "--seeds",
    "1",
    "--ticks",
    String(runOptions.ticks),
    "--map",
    job.map.name,
    "--out-dir",
    job.outDir,
  ];
  if (!runOptions.verifyReplay) args.push("--no-verify-replay");

  return new Promise((resolve, reject) => {
    const child = spawn(arenaBinary, args, {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
      shell: false,
    });
    activeChildren.add(child);
    let output = "";
    const capture = (chunk) => {
      output = `${output}${chunk}`.slice(-16_000);
    };
    child.stdout.on("data", capture);
    child.stderr.on("data", capture);
    child.on("error", (error) => {
      activeChildren.delete(child);
      reject(error);
    });
    child.on("exit", (code, signal) => {
      activeChildren.delete(child);
      if (code === 0) resolve();
      else {
        reject(
          new Error(
            `${job.matchup.id}/${job.map.id}/seed-${job.seed} exited with ` +
              `${signal || code}${output ? `\n${output.trim()}` : ""}`,
          ),
        );
      }
    });
  });
}

function readArenaSummary(job, runOptions) {
  const summaryPath = path.join(job.outDir, "arena-summary.json");
  const summary = JSON.parse(fs.readFileSync(summaryPath, "utf8"));
  const mismatches = [];
  if (summary.candidate !== job.matchup.candidate) mismatches.push("candidate");
  if (summary.baseline !== job.matchup.baseline) mismatches.push("baseline");
  if (summary.mapName !== job.map.name) mismatches.push("mapName");
  if (summary.seedStart !== job.seed) mismatches.push("seedStart");
  if (summary.seeds !== 1) mismatches.push("seeds");
  if (summary.maxTicks !== runOptions.ticks) mismatches.push("maxTicks");
  if (!Array.isArray(summary.runs)) mismatches.push("runs");
  if (mismatches.length > 0) {
    throw new Error(`${summaryPath} does not match this run (${mismatches.join(", ")})`);
  }
  return summary;
}

function aggregateResults(plannedJobs, runOptions, gitSha) {
  const rows = [];
  for (const matchup of MATCHUPS) {
    for (const map of MAPS) {
      const matchingJobs = plannedJobs.filter(
        (job) => job.matchup.id === matchup.id && job.map.id === map.id,
      );
      const runs = matchingJobs.flatMap((job) => readArenaSummary(job, runOptions).runs);
      rows.push(aggregateRuns(matchup, map, runs));
    }
  }
  const totals = MATCHUPS.map((matchup) => {
    const matchupRows = rows.filter((row) => row.matchupId === matchup.id);
    return {
      matchupId: matchup.id,
      matchupLabel: matchup.label,
      candidate: matchup.candidate,
      baseline: matchup.baseline,
      games: sum(matchupRows, "games"),
      candidateWins: sum(matchupRows, "candidateWins"),
      baselineWins: sum(matchupRows, "baselineWins"),
      draws: sum(matchupRows, "draws"),
    };
  });
  const games = sum(totals, "games");
  if (games !== GAME_COUNT) throw new Error(`aggregated ${games} games instead of ${GAME_COUNT}`);
  return {
    schema: "rts-120-game-test-summary-v1",
    generatedAt: new Date().toISOString(),
    gitSha,
    ticks: runOptions.ticks,
    seedsPerMap: SEEDS_PER_MAP,
    sideSwapped: true,
    games,
    rows,
    totals,
  };
}

function aggregateRuns(matchup, map, runs) {
  if (runs.length !== SEEDS_PER_MAP * 2) {
    throw new Error(`${matchup.id}/${map.id} has ${runs.length} games; expected 10`);
  }
  const diagnostics = runs.map((run) => {
    const candidate = run.result.players.find((player) => player.playerId === run.candidatePlayerId);
    const baseline = run.result.players.find((player) => player.playerId === run.baselinePlayerId);
    if (!candidate || !baseline) throw new Error(`${matchup.id}/${map.id} has invalid player ids`);
    return { run, candidate, baseline };
  });
  const candidateWins = diagnostics.filter(({ run }) => run.outcome.candidateWon).length;
  const baselineWins = diagnostics.filter(({ run }) => run.outcome.baselineWon).length;
  return {
    matchupId: matchup.id,
    matchupLabel: matchup.label,
    mapId: map.id,
    mapName: map.name,
    candidate: matchup.candidate,
    baseline: matchup.baseline,
    games: runs.length,
    candidateWins,
    baselineWins,
    draws: runs.length - candidateWins - baselineWins,
    averageTicks: average(diagnostics.map(({ run }) => run.result.ticks)),
    candidateAverageArmy: average(diagnostics.map(({ candidate }) => candidate.armyValue)),
    baselineAverageArmy: average(diagnostics.map(({ baseline }) => baseline.armyValue)),
    candidateAverageDeaths: average(diagnostics.map(({ candidate }) => candidate.deathCount)),
    baselineAverageDeaths: average(diagnostics.map(({ baseline }) => baseline.deathCount)),
  };
}

function average(values) {
  return Math.round((values.reduce((total, value) => total + value, 0) / values.length) * 10) / 10;
}

function sum(rows, key) {
  return rows.reduce((total, row) => total + row[key], 0);
}

function writeReports(report, runOptions) {
  fs.writeFileSync(
    path.join(runOptions.outDir, "summary.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
  fs.writeFileSync(path.join(runOptions.outDir, "summary.csv"), renderCsv(report));
  fs.writeFileSync(path.join(runOptions.outDir, "analysis.md"), renderMarkdown(report));
}

function renderCsv(report) {
  const headers = [
    "matchup",
    "map",
    "games",
    "candidate_wins",
    "baseline_wins",
    "draws",
    "average_ticks",
    "candidate_average_army",
    "baseline_average_army",
    "candidate_average_deaths",
    "baseline_average_deaths",
  ];
  const rows = report.rows.map((row) => [
    row.matchupLabel,
    row.mapName,
    row.games,
    row.candidateWins,
    row.baselineWins,
    row.draws,
    row.averageTicks,
    row.candidateAverageArmy,
    row.baselineAverageArmy,
    row.candidateAverageDeaths,
    row.baselineAverageDeaths,
  ]);
  return `${[headers, ...rows].map((row) => row.map(csvCell).join(",")).join("\n")}\n`;
}

function csvCell(value) {
  const text = String(value);
  return /[",\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function renderMarkdown(report) {
  const lines = [
    "# 120 game test analysis",
    "",
    `Generated from commit \`${report.gitSha}\` with a ${report.ticks.toLocaleString("en-US")}-tick cap.`,
    `${SEEDS_PER_MAP} seeds per map were side-swapped. Tick-cap games remain draws; army value is not a tiebreaker.`,
    "",
    "W-L-D is from the first profile's perspective.",
    "",
    "| Matchup | Map | W-L-D | Average ticks | Average final army | Average deaths |",
    "| --- | --- | ---: | ---: | ---: | ---: |",
  ];
  for (const row of report.rows) {
    lines.push(
      `| ${row.matchupLabel} | ${row.mapName} | ${row.candidateWins}-${row.baselineWins}-${row.draws} | ` +
        `${number(row.averageTicks)} | ${number(row.candidateAverageArmy)} / ${number(row.baselineAverageArmy)} | ` +
        `${number(row.candidateAverageDeaths)} / ${number(row.baselineAverageDeaths)} |`,
    );
  }
  lines.push("", "## Totals", "", "| Matchup | W-L-D |", "| --- | ---: |");
  for (const total of report.totals) {
    lines.push(
      `| ${total.matchupLabel} | ${total.candidateWins}-${total.baselineWins}-${total.draws} |`,
    );
  }
  lines.push("");
  return `${lines.join("\n")}\n`;
}

function number(value) {
  return value.toLocaleString("en-US", { maximumFractionDigits: 1 });
}

function writeRunConfig(runOptions, gitSha, status, error = "") {
  fs.mkdirSync(runOptions.outDir, { recursive: true });
  fs.writeFileSync(
    path.join(runOptions.outDir, "run-config.json"),
    `${JSON.stringify(
      {
        schema: "rts-120-game-test-run-v1",
        status,
        updatedAt: new Date().toISOString(),
        gitSha,
        games: GAME_COUNT,
        seedsPerMap: SEEDS_PER_MAP,
        sideSwapped: true,
        ticks: runOptions.ticks,
        concurrency: runOptions.concurrency,
        verifyReplay: runOptions.verifyReplay,
        matchups: MATCHUPS,
        maps: MAPS,
        ...(error ? { error } : {}),
      },
      null,
      2,
    )}\n`,
  );
}
