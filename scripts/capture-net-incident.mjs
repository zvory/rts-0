#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..");
const PARSER_SCRIPT = path.join(REPO_ROOT, "scripts", "parse-net-report-logs.mjs");
const FLY_LOGS_SCRIPT = path.join(REPO_ROOT, "scripts", "fly-logs.sh");
const DEFAULT_TIMELINE_BAND_MS = 60_000;
const DEFAULT_MAX_PAGES = 20;
const DEFAULT_LOG_FILTER = [
  "client network report",
  "match started",
  "match ended",
  "performance tick summary",
  "performance pathing diagnostics",
  "performance snapshot timing",
  "performance writer timing",
].join("|");
const FIXTURES = Object.freeze({
  "soupman-alex": Object.freeze({
    incidentId: "2026-06-30-beta-soupman-alex-lag",
    source: "beta_fixture",
    matchId: "103",
    runId: "alex-s-lobby-1782778605186-000004",
    logs: [
      path.join(
        REPO_ROOT,
        "docs",
        "network-incident-examples",
        "2026-06-30-beta-soupman-alex-lag",
        "match-103-runid-logs.jsonl",
      ),
    ],
    replay: path.join(
      REPO_ROOT,
      "docs",
      "network-incident-examples",
      "2026-06-30-beta-soupman-alex-lag",
      "match-103-replay.json",
    ),
    dbSummary: path.join(
      REPO_ROOT,
      "docs",
      "network-incident-examples",
      "2026-06-30-beta-soupman-alex-lag",
      "match-103-db-summary.json",
    ),
  }),
});

function usage() {
  console.log(`Usage:
  node scripts/capture-net-incident.mjs --out-dir DIR --logs FILE [--logs FILE...] [options]
  node scripts/capture-net-incident.mjs --out-dir DIR --fixture soupman-alex [options]
  node scripts/capture-net-incident.mjs --out-dir DIR --beta --from ISO8601 --to ISO8601 [options]

Options:
  --fixture NAME                  Use a preserved fixture. Available: ${Object.keys(FIXTURES).join(", ")}.
  --beta                          Query bounded Fly beta/mainline logs before packaging.
  --channel beta|mainline|APP      Fly app selector for --beta. Default: beta.
  --from ISO8601                  UTC start for --beta search.
  --to ISO8601                    UTC end for --beta search.
  --run-id ID                     Match run id to preserve and report.
  --match-id ID                   Public match id to report when known.
  --build SHA                     Deployed build id to report when known.
  --participants A,B              Participant names when known.
  --replay FILE                   Replay artifact to copy into replay/replay.json.
  --db-summary FILE               Match-history DB summary to copy into db/db-summary.json.
  --player-notes FILE             Markdown notes with player reports and absolute timestamps.
  --require-coverage LIST         Comma-separated diagnostics: command,snapshot,pathing,client-context.
  --timeline-band-ms MS           Parser timeline band width. Default: ${DEFAULT_TIMELINE_BAND_MS}.
  --max-pages N                   Fly log API page bound for --beta. Default: ${DEFAULT_MAX_PAGES}.
  --force                         Replace an existing non-empty output directory.
  --dry-run                       Print the planned capture/package command without writing files.
  -h, --help                      Show this help.

The script writes a complete evidence package with raw logs, parser output, agent digest,
key metrics, replay/DB availability, player-report notes, neutral analysis template, and a
beta evidence checklist. It never prints Fly tokens.`);
}

function parseArgs(argv) {
  const options = {
    logs: [],
    fixture: "",
    beta: false,
    channel: "beta",
    from: "",
    to: "",
    runId: "",
    matchId: "",
    build: "",
    participants: [],
    outDir: "",
    replay: "",
    dbSummary: "",
    playerNotes: "",
    replayUnavailableReason: "No replay artifact was provided for this package.",
    dbUnavailableReason: "No DB summary artifact was provided for this package.",
    requireCoverage: [],
    timelineBandMs: DEFAULT_TIMELINE_BAND_MS,
    maxPages: DEFAULT_MAX_PAGES,
    force: false,
    dryRun: false,
    source: "local",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else if (arg === "--logs") {
      options.logs.push(requiredValue(argv, ++index, arg));
    } else if (arg.startsWith("--logs=")) {
      options.logs.push(arg.slice("--logs=".length));
    } else if (arg === "--fixture") {
      options.fixture = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--fixture=")) {
      options.fixture = arg.slice("--fixture=".length);
    } else if (arg === "--beta") {
      options.beta = true;
      options.source = "beta_live";
    } else if (arg === "--channel") {
      options.channel = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--channel=")) {
      options.channel = arg.slice("--channel=".length);
    } else if (arg === "--from") {
      options.from = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--from=")) {
      options.from = arg.slice("--from=".length);
    } else if (arg === "--to") {
      options.to = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--to=")) {
      options.to = arg.slice("--to=".length);
    } else if (arg === "--run-id") {
      options.runId = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--run-id=")) {
      options.runId = arg.slice("--run-id=".length);
    } else if (arg === "--match-id") {
      options.matchId = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--match-id=")) {
      options.matchId = arg.slice("--match-id=".length);
    } else if (arg === "--build") {
      options.build = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--build=")) {
      options.build = arg.slice("--build=".length);
    } else if (arg === "--participants") {
      options.participants = splitList(requiredValue(argv, ++index, arg));
    } else if (arg.startsWith("--participants=")) {
      options.participants = splitList(arg.slice("--participants=".length));
    } else if (arg === "--out-dir") {
      options.outDir = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--out-dir=")) {
      options.outDir = arg.slice("--out-dir=".length);
    } else if (arg === "--replay") {
      options.replay = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--replay=")) {
      options.replay = arg.slice("--replay=".length);
    } else if (arg === "--db-summary") {
      options.dbSummary = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--db-summary=")) {
      options.dbSummary = arg.slice("--db-summary=".length);
    } else if (arg === "--player-notes") {
      options.playerNotes = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--player-notes=")) {
      options.playerNotes = arg.slice("--player-notes=".length);
    } else if (arg === "--replay-unavailable-reason") {
      options.replayUnavailableReason = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--replay-unavailable-reason=")) {
      options.replayUnavailableReason = arg.slice("--replay-unavailable-reason=".length);
    } else if (arg === "--db-unavailable-reason") {
      options.dbUnavailableReason = requiredValue(argv, ++index, arg);
    } else if (arg.startsWith("--db-unavailable-reason=")) {
      options.dbUnavailableReason = arg.slice("--db-unavailable-reason=".length);
    } else if (arg === "--require-coverage") {
      options.requireCoverage.push(...parseCoverageList(requiredValue(argv, ++index, arg)));
    } else if (arg.startsWith("--require-coverage=")) {
      options.requireCoverage.push(...parseCoverageList(arg.slice("--require-coverage=".length)));
    } else if (arg === "--timeline-band-ms") {
      options.timelineBandMs = Number(requiredValue(argv, ++index, arg));
    } else if (arg.startsWith("--timeline-band-ms=")) {
      options.timelineBandMs = Number(arg.slice("--timeline-band-ms=".length));
    } else if (arg === "--max-pages") {
      options.maxPages = Number(requiredValue(argv, ++index, arg));
    } else if (arg.startsWith("--max-pages=")) {
      options.maxPages = Number(arg.slice("--max-pages=".length));
    } else if (arg === "--force") {
      options.force = true;
    } else if (arg === "--dry-run") {
      options.dryRun = true;
    } else if (arg.startsWith("--")) {
      throw new Error(`unknown option: ${arg}`);
    } else {
      options.logs.push(arg);
    }
  }

  return applyFixture(options);
}

function applyFixture(options) {
  if (!options.fixture) return options;
  const fixture = FIXTURES[options.fixture];
  if (!fixture) {
    throw new Error(`unknown fixture: ${options.fixture}`);
  }
  return {
    ...options,
    source: fixture.source,
    logs: options.logs.length > 0 ? options.logs : [...fixture.logs],
    replay: options.replay || fixture.replay,
    dbSummary: options.dbSummary || fixture.dbSummary,
    runId: options.runId || fixture.runId,
    matchId: options.matchId || fixture.matchId,
  };
}

function validateArgs(options) {
  if (options.help) return;
  if (!options.outDir) throw new Error("--out-dir is required");
  if (options.fixture && options.beta) throw new Error("--fixture cannot be combined with --beta");
  if (options.beta && !options.from) throw new Error("--beta requires --from");
  if (!options.beta && options.logs.length === 0) throw new Error("at least one --logs file or --fixture is required");
  if (!Number.isFinite(options.timelineBandMs) || options.timelineBandMs < 1_000) {
    throw new Error("--timeline-band-ms must be a number of at least 1000");
  }
  if (!Number.isInteger(options.maxPages) || options.maxPages < 1) {
    throw new Error("--max-pages must be a positive integer");
  }
  for (const file of [...options.logs, options.replay, options.dbSummary, options.playerNotes].filter(Boolean)) {
    if (!existsSync(file)) throw new Error(`input file does not exist: ${file}`);
  }
}

function requiredValue(argv, index, flag) {
  const value = argv[index];
  if (!value || value.startsWith("--")) throw new Error(`${flag} requires a value`);
  return value;
}

function splitList(value) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function parseCoverageList(value) {
  return splitList(value).map(normalizeCoverageId);
}

function normalizeCoverageId(value) {
  return value.toLowerCase().replace(/_/g, "-");
}

function prepareOutputDir(outDir, force) {
  if (existsSync(outDir)) {
    const entries = readdirSync(outDir).filter((entry) => entry !== ".DS_Store");
    if (entries.length > 0 && !force) {
      throw new Error(`output directory is not empty: ${outDir} (pass --force to replace it)`);
    }
    if (entries.length > 0) rmSync(outDir, { recursive: true, force: true });
  }
  mkdirSync(outDir, { recursive: true });
}

function plannedFlyCommand(options) {
  const args = [
    "scripts/fly-logs.sh",
    options.channel,
    "search",
    "--from",
    options.from,
    "--filter",
    DEFAULT_LOG_FILTER,
    "--max-pages",
    String(options.maxPages),
  ];
  if (options.to) args.push("--to", options.to);
  return args;
}

function captureBetaLogs(options, rawDir) {
  const output = path.join(rawDir, "fly-search-logs.jsonl");
  const args = [
    FLY_LOGS_SCRIPT,
    options.channel,
    "search",
    "--from",
    options.from,
    "--filter",
    DEFAULT_LOG_FILTER,
    "--max-pages",
    String(options.maxPages),
  ];
  if (options.to) args.push("--to", options.to);
  const logText = execFileSync("bash", args, {
    cwd: REPO_ROOT,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
  });
  writeFileSync(output, logText);
  return [output];
}

function stageLogInputs(options, rawDir) {
  const sourceLogs = options.beta ? captureBetaLogs(options, rawDir) : options.logs;
  if (sourceLogs.length === 0) throw new Error("no log inputs were captured");
  if (options.runId) {
    const staged = path.join(rawDir, `${safeName(options.runId)}-logs.jsonl`);
    const lines = sourceLogs.flatMap((file) => readLines(file).filter((line) => line.includes(options.runId)));
    if (lines.length === 0) {
      throw new Error(`no log rows matched --run-id ${options.runId}`);
    }
    writeFileSync(staged, `${lines.join("\n")}\n`);
    for (const file of sourceLogs) {
      if (options.beta && path.dirname(file) === rawDir && file !== staged) rmSync(file, { force: true });
    }
    return [staged];
  }

  if (options.beta) return sourceLogs;

  return sourceLogs.map((file, index) => {
    const name = `${String(index + 1).padStart(2, "0")}-${safeName(path.basename(file))}`;
    const staged = path.join(rawDir, name);
    copyFileSync(file, staged);
    return staged;
  });
}

function readLines(file) {
  return readFileSync(file, "utf8")
    .split(/\r?\n/)
    .filter((line) => line.trim().length > 0);
}

function runParser(stagedLogs, parserDir, options) {
  const args = [
    PARSER_SCRIPT,
    "--out-dir",
    parserDir,
    "--timeline-band-ms",
    String(options.timelineBandMs),
    ...stagedLogs,
  ];
  execFileSync("node", args, {
    cwd: REPO_ROOT,
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
  });
  return {
    command: [
      "node",
      "scripts/parse-net-report-logs.mjs",
      "--out-dir",
      rel(parserDir),
      "--timeline-band-ms",
      String(options.timelineBandMs),
      ...stagedLogs.map(rel),
    ],
    report: readJson(path.join(parserDir, "incident-summary.json")),
  };
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function writeArtifactOrUnavailable({ source, outputDir, packageRoot, presentName, unavailableReason }) {
  mkdirSync(outputDir, { recursive: true });
  if (source) {
    const output = path.join(outputDir, presentName);
    copyFileSync(source, output);
    return { present: true, path: packageRel(output, packageRoot), source: rel(source) };
  }
  const output = path.join(outputDir, "UNAVAILABLE.md");
  writeFileSync(output, `# Unavailable\n\n${unavailableReason}\n`);
  return { present: false, path: packageRel(output, packageRoot), reason: unavailableReason };
}

function writePlayerNotes(options, outDir) {
  const output = path.join(outDir, "player-report-notes.md");
  if (options.playerNotes) {
    copyFileSync(options.playerNotes, output);
    return { path: packageRel(output, outDir), source: rel(options.playerNotes), provided: true };
  }
  writeFileSync(
    output,
    [
      "# Player Report Notes",
      "",
      "No player report notes were provided to this package.",
      "",
      "Future updates should record player-visible symptoms with absolute UTC timestamps, for example:",
      "",
      "| UTC timestamp | player | report | source |",
      "| --- | --- | --- | --- |",
      "| 2026-06-30T00:00:00Z | player-name | observed symptom | chat, call note, or issue link |",
      "",
    ].join("\n"),
  );
  return { path: packageRel(output, outDir), provided: false };
}

function selectedMatch(report, options) {
  const matches = report.matches || [];
  return (
    matches.find((match) => options.runId && match.matchRunId === options.runId) ||
    matches.find((match) => options.matchId && String(match.match) === String(options.matchId)) ||
    matches[0] ||
    {}
  );
}

function incidentSummary(report, options) {
  const match = selectedMatch(report, options);
  const digestMatch =
    report.agentDigest?.matches?.find((item) => item.matchId === match.match || item.matchRunId === match.matchRunId) ||
    report.agentDigest?.matches?.[0] ||
    {};
  const diagnosis =
    report.agentDigest?.summary?.primaryDiagnoses?.find((item) => item.match === `Match ${match.match}`)?.diagnosis ||
    digestMatch.diagnosis ||
    "No parser diagnosis was generated.";
  return {
    matchId: options.matchId || match.match || "",
    matchRunId: options.runId || match.matchRunId || "",
    utcWindow: {
      start: options.from || match.startedAt || digestMatch.utcWindow?.start || "",
      end: options.to || match.endedAt || digestMatch.utcWindow?.end || "",
    },
    build: options.build || (match.buildIds || []).join(", "),
    participants: options.participants.length > 0 ? options.participants : match.participants || [],
    room: match.rooms?.join(", ") || match.room || "",
    source: options.source,
    neutralDiagnosis: `${diagnosis} This package records evidence and unknowns only; it does not prescribe a fix.`,
  };
}

function buildDiagnosticCoverage(report, parserDir) {
  const digest = report.agentDigest || {};
  const coverageItems = (digest.coverageMatrix?.matches || []).flatMap((match) => match.items || []);
  const slowestPhaseEvidence = serverTickSlowestPhaseEvidence(parserDir);
  const requirements = [
    coverageRequirement("command", [
      presentCoverage(coverageItems, "command_lifecycle", "command lifecycle fields"),
      topWindowEvidence(digest, "command", "command response top windows"),
      topWindowEvidence(digest, "command_density", "command density top windows"),
    ]),
    coverageRequirement("snapshot", [
      presentCoverage(coverageItems, "snapshot_perf_rows", "snapshot projection rows"),
      presentCoverage(coverageItems, "writer_rows", "writer send rows"),
      topWindowEvidence(digest, "snapshot_payload", "snapshot payload top windows"),
      topWindowEvidence(digest, "outbound", "outbound writer top windows"),
      topWindowEvidence(digest, "network", "snapshot delivery/network top windows"),
    ]),
    coverageRequirement("pathing", [
      presentCoverage(coverageItems, "pathing_perf_rows", "pathing diagnostics rows"),
      slowestPhaseEvidence.length > 0
        ? {
            present: true,
            evidence: slowestPhaseEvidence,
          }
        : null,
      topWindowEvidence(digest, "server_pathing", "server pathing top windows"),
    ]),
    coverageRequirement("client-context", [
      presentCoverage(coverageItems, "client_frame_render", "client frame/render fields"),
      topWindowEvidence(digest, "frame_render", "browser frame/render top windows"),
      topWindowEvidence(digest, "prediction", "prediction/late-snapshot top windows"),
    ]),
  ];
  return {
    schemaVersion: 1,
    generatedAt: report.generatedAt,
    requirements,
    missing: requirements.filter((item) => !item.present).map((item) => item.id),
  };
}

function coverageRequirement(id, checks) {
  const evidence = checks.filter(Boolean).flatMap((check) => (check.present ? check.evidence : []));
  return {
    id,
    present: evidence.length > 0,
    evidence,
    caveat: coverageCaveat(id),
  };
}

function presentCoverage(items, id, label) {
  const rows = items.filter((item) => item.id === id && item.present).reduce((sum, item) => sum + (item.rows || 0), 0);
  if (rows <= 0) return { present: false, evidence: [] };
  return { present: true, evidence: [`${label}: ${rows} row(s)`] };
}

function topWindowEvidence(digest, groupId, label) {
  const group = (digest.topWindows?.groups || []).find((item) => item.id === groupId);
  const first = group?.windows?.[0];
  if (!first) return { present: false, evidence: [] };
  return {
    present: true,
    evidence: [`${label}: ${first.dominantField}=${first.dominantValue} at ${first.timestamp}`],
  };
}

function serverTickSlowestPhaseEvidence(parserDir) {
  const file = path.join(parserDir, "server-tick-rows.tsv");
  if (!existsSync(file)) return [];
  const lines = readFileSync(file, "utf8").trim().split(/\r?\n/);
  if (lines.length < 2) return [];
  const headers = lines[0].split("\t");
  const phaseIndex = headers.indexOf("slowest_phase");
  const phaseMsIndex = headers.indexOf("slowest_phase_ms");
  const timeIndex = headers.indexOf("timestamp");
  if (phaseIndex < 0) return [];
  const counts = new Map();
  let worst = null;
  for (const line of lines.slice(1)) {
    const cols = line.split("\t");
    const phase = cols[phaseIndex];
    if (!phase || phase === "none") continue;
    counts.set(phase, (counts.get(phase) || 0) + 1);
    const phaseMs = Number(cols[phaseMsIndex]);
    if (Number.isFinite(phaseMs) && (!worst || phaseMs > worst.phaseMs)) {
      worst = { phase, phaseMs, timestamp: cols[timeIndex] || "" };
    }
  }
  const top = [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, 3)
    .map(([phase, count]) => `${phase} slowest phase: ${count} row(s)`);
  if (worst) top.unshift(`worst slowest phase ${worst.phase}=${worst.phaseMs}ms at ${worst.timestamp}`);
  return top;
}

function coverageCaveat(id) {
  return {
    command: "Command coverage is aggregate lifecycle/density evidence; it does not include raw command payloads or unit lists.",
    snapshot: "Snapshot coverage is payload/lifecycle/delivery evidence; it does not include raw snapshot bodies.",
    pathing: "Pathing coverage may be detailed pathing rows or slow-tick phase evidence; missing detailed rows remain an unknown.",
    "client-context": "Client context is bounded frame/render/prediction aggregate evidence, not a browser trace.",
  }[id];
}

function assertRequiredCoverage(coverage, required) {
  const missing = required.filter((id) => !coverage.requirements.find((item) => item.id === id && item.present));
  if (missing.length > 0) {
    throw new Error(`required diagnostic coverage missing: ${missing.join(", ")}`);
  }
}

function formatAgentDigestMarkdown(report, coverage) {
  const digest = report.agentDigest || {};
  const lines = ["# Agent Digest", "", `Generated: ${report.generatedAt || "unknown"}`, ""];
  lines.push("## Supported Diagnoses");
  for (const item of digest.summary?.primaryDiagnoses || []) {
    lines.push(`- ${item.match}: ${item.diagnosis}`);
  }
  if ((digest.summary?.primaryDiagnoses || []).length === 0) lines.push("- No primary diagnosis generated.");
  lines.push("");
  lines.push("## Coverage Requirements");
  for (const item of coverage.requirements) {
    lines.push(`- ${item.id}: ${item.present ? "present" : "not logged or unavailable"}`);
    for (const evidence of item.evidence.slice(0, 3)) lines.push(`  - ${evidence}`);
    lines.push(`  - Caveat: ${item.caveat}`);
  }
  lines.push("");
  lines.push("## Top Bad Windows");
  for (const group of digest.topWindows?.groups || []) {
    const first = group.windows?.[0];
    if (first) {
      lines.push(`- ${group.id}: ${first.timestamp} match ${first.match} ${first.summary}`);
    } else {
      lines.push(`- ${group.id}: no threshold-crossing windows`);
    }
  }
  lines.push("");
  lines.push("## Unknowns");
  for (const item of (digest.unknowns || []).slice(0, 12)) {
    lines.push(`- Match ${item.match}: ${item.text}`);
  }
  if ((digest.unknowns || []).length === 0) lines.push("- No parser unknowns were generated.");
  lines.push("");
  return `${lines.join("\n")}\n`;
}

function formatReadme({ summary, files, coverage, parserCommand }) {
  const lines = ["# Network Incident Evidence Package", ""];
  lines.push("## Identity");
  lines.push("| field | value |");
  lines.push("| --- | --- |");
  lines.push(`| Match | ${tableValue(summary.matchId)} |`);
  lines.push(`| Match run id | ${tableValue(summary.matchRunId)} |`);
  lines.push(`| UTC window | ${tableValue(formatWindow(summary.utcWindow))} |`);
  lines.push(`| Build | ${tableValue(summary.build)} |`);
  lines.push(`| Participants | ${tableValue(summary.participants.join(", "))} |`);
  lines.push(`| Room | ${tableValue(summary.room)} |`);
  lines.push(`| Source | ${tableValue(summary.source)} |`);
  lines.push("");
  lines.push("## Neutral Diagnosis");
  lines.push("");
  lines.push(summary.neutralDiagnosis);
  lines.push("");
  lines.push("## Source Files");
  for (const file of files) {
    lines.push(`- \`${file.path}\`: ${file.description}`);
  }
  lines.push("");
  lines.push("## Parser Command");
  lines.push("");
  lines.push("```bash");
  lines.push(parserCommand.map(shellQuote).join(" "));
  lines.push("```");
  lines.push("");
  lines.push("## Diagnostic Coverage");
  for (const item of coverage.requirements) {
    lines.push(`- ${item.id}: ${item.present ? "present" : "not logged or unavailable"}`);
  }
  lines.push("");
  lines.push("## Reading Order");
  lines.push("- Start with `agent-digest.md`, then `key-metrics.json`, then `parser/incident-summary.md`.");
  lines.push("- Use `raw/` only to verify exact source rows named by parser line references.");
  lines.push("- Fill `player-report-notes.md` with absolute UTC timestamps before comparing against player reports.");
  lines.push("- Keep `analysis.md` neutral: supported, contradicted, unknown, and next diagnostic gap sections only.");
  lines.push("");
  return `${lines.join("\n")}\n`;
}

function formatWindow(window) {
  if (window.start && window.end) return `${window.start} to ${window.end}`;
  return window.start || window.end || "";
}

function tableValue(value) {
  if (!value) return "not provided";
  return `\`${String(value).replace(/\|/g, "\\|")}\``;
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_./:=+-]+$/.test(value)) return value;
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}

function formatAnalysisTemplate(report) {
  const digest = report.agentDigest || {};
  const classifications = (report.matches || []).flatMap((match) =>
    (match.classifications || []).map((item) => ({ match: match.match, ...item })),
  );
  const supported = classifications.filter((item) => item.status === "indicated");
  const contradicted = classifications.filter((item) => item.status === "contradicted");
  const unknowns = digest.unknowns || [];
  const lines = [
    "# Analysis",
    "",
    "This file is an evidence interpretation template. Do not prescribe gameplay, transport, pathing, render, or balance changes here.",
    "",
    "## Supported",
  ];
  for (const item of supported.slice(0, 12)) {
    lines.push(`- Match ${item.match}: ${item.label}`);
  }
  if (supported.length === 0) lines.push("- No thresholded support recorded yet.");
  lines.push("");
  lines.push("## Contradicted");
  for (const item of contradicted.slice(0, 12)) {
    const evidence = (item.evidenceAgainst || []).slice(0, 1).join("; ");
    lines.push(`- Match ${item.match}: ${item.label}${evidence ? ` (${evidence})` : ""}`);
  }
  if (contradicted.length === 0) lines.push("- No contradicted pressure classes recorded yet.");
  lines.push("");
  lines.push("## Unknown");
  for (const item of unknowns.slice(0, 12)) {
    lines.push(`- Match ${item.match}: ${item.text}`);
  }
  if (unknowns.length === 0) lines.push("- No parser unknowns recorded yet.");
  lines.push("");
  lines.push("## Next Diagnostic Gaps");
  if (unknowns.length > 0) {
    for (const item of unknowns.slice(0, 8)) lines.push(`- Resolve or explicitly accept: Match ${item.match} ${item.text}`);
  } else {
    lines.push("- Identify missing evidence before proposing behavior changes.");
  }
  lines.push("");
  return `${lines.join("\n")}\n`;
}

function formatChecklist(summary, options) {
  return [
    "# Beta Evidence Gate Checklist",
    "",
    "- [ ] Deployed build recorded before analysis.",
    `  - Current package value: ${summary.build || "not provided"}`,
    "- [ ] Exact match run id recorded.",
    `  - Current package value: ${summary.matchRunId || "not provided"}`,
    "- [ ] Exact UTC window recorded with start and end.",
    `  - Current package value: ${formatWindow(summary.utcWindow) || "not provided"}`,
    "- [ ] Focused bounded log query preserved.",
    `  - Query: ${options.beta ? plannedFlyCommand(options).map(shellQuote).join(" ") : "local/preserved logs were provided"}`,
    "- [ ] Parser command recorded and rerunnable from repo root.",
    "- [ ] Source coverage checked for command, snapshot, pathing, and client context.",
    "- [ ] Unknowns copied into `analysis.md` before any fixing plan starts.",
    "- [ ] Replay artifact copied or `replay/UNAVAILABLE.md` explains why not.",
    "- [ ] DB summary copied or `db/UNAVAILABLE.md` explains why not.",
    "- [ ] Player report notes use absolute UTC timestamps.",
    "",
  ].join("\n");
}

function packageFiles({ stagedLogs, outDir, replayArtifact, dbArtifact, playerNotes }) {
  const files = [
    ...stagedLogs.map((file) => ({
      path: packageRel(file, outDir),
      description: "raw run-id or window log rows used by the parser",
    })),
    { path: "parser/README.md", description: "parser-generated agent-first package README" },
    { path: "parser/incident-summary.md", description: "parser markdown summary" },
    { path: "parser/incident-summary.json", description: "parser JSON summary with embedded agent digest" },
    { path: "parser/incident-rows.tsv", description: "parser per-player TSV summary" },
    { path: "parser/client-net-rows.tsv", description: "filtered client report windows" },
    { path: "parser/server-tick-rows.tsv", description: "filtered server slow-tick rows" },
    { path: "agent-digest.md", description: "standalone agent-readable digest" },
    { path: "agent-digest.json", description: "standalone agent digest JSON" },
    { path: "key-metrics.json", description: "stable key metrics JSON copied from parser output" },
    { path: "diagnostic-coverage.json", description: "coverage gate result for command, snapshot, pathing, and client context" },
    { path: replayArtifact.path, description: replayArtifact.present ? "replay artifact" : "replay unavailable reason" },
    { path: dbArtifact.path, description: dbArtifact.present ? "DB summary artifact" : "DB summary unavailable reason" },
    { path: playerNotes.path, description: playerNotes.provided ? "player report notes" : "player report notes template" },
    { path: "analysis.md", description: "neutral supported/contradicted/unknown analysis template" },
    { path: "beta-evidence-checklist.md", description: "future beta evidence gate checklist" },
    { path: "package-manifest.json", description: "machine-readable package provenance" },
  ];
  return files;
}

function formatManifest({ report, summary, files, coverage, parserCommand, options, replayArtifact, dbArtifact }) {
  return `${JSON.stringify(
    {
      schemaVersion: 1,
      generatedAt: report.generatedAt,
      tool: "scripts/capture-net-incident.mjs",
      incident: summary,
      capture: {
        source: options.source,
        beta: options.beta,
        channel: options.beta ? options.channel : "",
        from: options.from,
        to: options.to,
        maxPages: options.beta ? options.maxPages : null,
        filter: options.beta ? DEFAULT_LOG_FILTER : "",
      },
      parserCommand,
      files,
      coverage,
      artifacts: {
        replay: replayArtifact,
        dbSummary: dbArtifact,
      },
      privacyBoundary:
        "Package files preserve bounded diagnostic aggregates and raw server log rows for the requested incident only. They must not include Fly tokens, raw command payloads, raw snapshots, entity ids, target ids, player-entered text, stack traces, secrets, or browser-local traces unless a later opt-in artifact explicitly documents that boundary.",
    },
    null,
    2,
  )}\n`;
}

function safeName(value) {
  return String(value)
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 120) || "input";
}

function rel(file) {
  return path.relative(REPO_ROOT, path.resolve(file)).replace(/\\/g, "/");
}

function packageRel(file, packageRoot) {
  return path.relative(packageRoot, path.resolve(file)).replace(/\\/g, "/");
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
    validateArgs(options);
  } catch (error) {
    console.error(`error: ${error.message}`);
    usage();
    process.exit(2);
  }
  if (options.help) {
    usage();
    return;
  }

  const outDir = path.resolve(options.outDir);
  if (options.dryRun) {
    console.log(
      JSON.stringify(
        {
          outDir,
          logs: options.logs.map(rel),
          betaCommand: options.beta ? plannedFlyCommand(options) : null,
          parser: "scripts/parse-net-report-logs.mjs --out-dir parser <staged-logs>",
          requireCoverage: options.requireCoverage,
        },
        null,
        2,
      ),
    );
    return;
  }

  try {
    prepareOutputDir(outDir, options.force);
    const rawDir = path.join(outDir, "raw");
    const parserDir = path.join(outDir, "parser");
    mkdirSync(rawDir, { recursive: true });
    const stagedLogs = stageLogInputs(options, rawDir);
    const parser = runParser(stagedLogs, parserDir, options);
    const report = parser.report;
    const coverage = buildDiagnosticCoverage(report, parserDir);
    assertRequiredCoverage(coverage, options.requireCoverage);
    const summary = incidentSummary(report, options);
    const replayArtifact = writeArtifactOrUnavailable({
      source: options.replay,
      outputDir: path.join(outDir, "replay"),
      packageRoot: outDir,
      presentName: "replay.json",
      unavailableReason: options.replayUnavailableReason,
    });
    const dbArtifact = writeArtifactOrUnavailable({
      source: options.dbSummary,
      outputDir: path.join(outDir, "db"),
      packageRoot: outDir,
      presentName: "db-summary.json",
      unavailableReason: options.dbUnavailableReason,
    });
    const playerNotes = writePlayerNotes(options, outDir);
    const files = packageFiles({ stagedLogs, outDir, replayArtifact, dbArtifact, playerNotes });
    writeFileSync(path.join(outDir, "agent-digest.json"), `${JSON.stringify(report.agentDigest || {}, null, 2)}\n`);
    writeFileSync(path.join(outDir, "agent-digest.md"), formatAgentDigestMarkdown(report, coverage));
    copyFileSync(path.join(parserDir, "key-metrics.json"), path.join(outDir, "key-metrics.json"));
    writeFileSync(path.join(outDir, "diagnostic-coverage.json"), `${JSON.stringify(coverage, null, 2)}\n`);
    writeFileSync(path.join(outDir, "analysis.md"), formatAnalysisTemplate(report));
    writeFileSync(path.join(outDir, "beta-evidence-checklist.md"), formatChecklist(summary, options));
    writeFileSync(
      path.join(outDir, "package-manifest.json"),
      formatManifest({
        report,
        summary,
        files,
        coverage,
        parserCommand: parser.command,
        options,
        replayArtifact,
        dbArtifact,
      }),
    );
    writeFileSync(path.join(outDir, "README.md"), formatReadme({ summary, files, coverage, parserCommand: parser.command }));
    console.log(`incident package: ${outDir}`);
    console.log(`agent digest: ${path.join(outDir, "agent-digest.md")}`);
    console.log(`parser summary: ${path.join(parserDir, "incident-summary.md")}`);
    console.log(`coverage: ${coverage.requirements.map((item) => `${item.id}=${item.present ? "present" : "missing"}`).join(", ")}`);
  } catch (error) {
    console.error(`error: ${error.message}`);
    process.exit(1);
  }
}

main();
