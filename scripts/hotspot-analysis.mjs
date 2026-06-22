#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const INCLUDED_ROOTS = ["server", "client", "tests", "scripts"];
const INCLUDED_EXTENSIONS = new Set([".rs", ".js", ".mjs", ".sh", ".css", ".html"]);
const EXCLUDED_PATH_FRAGMENTS = [
  "target/",
  "node_modules/",
  "dist/",
  "build/",
  "coverage/",
  "tmp/",
  "replay/",
  "replays/",
  "artifacts/",
];
const FIX_LIKE_SUBJECT = /\b(fix|bug|regress|panic|crash|deadlock|flake|repair|harden|guard|correct|restore)\b/i;
const COMMIT_MARKER = "__COMMIT__";
const MAX_BUFFER = 256 * 1024 * 1024;
const SCORE_FORMULA =
  "100 * (0.22*sqrt(non_empty_loc/max) + 0.23*sqrt(rename_aware_touches/max) + 0.22*sqrt(total_churn/max) + 0.13*sqrt(recent_churn/max) + 0.10*sqrt(fix_like_touches/max) + 0.10*sqrt(recent_cochange_degree/max))";

const GROUP_RULES = [
  {
    group: "protocol-and-contracts",
    exact: [
      "tests/client_contracts.mjs",
      "tests/protocol_parity.mjs",
      "client/src/protocol.js",
      "client/src/protocol_constants.js",
      "server/crates/protocol/src/contract_metadata.rs",
      "server/src/protocol.rs",
    ],
    prefixes: [
      "client/src/protocol/",
      "client/src/protocol_",
      "tests/client_contracts/",
      "server/crates/protocol/",
      "server/crates/contract/",
    ],
  },
  {
    group: "balance-and-config",
    exact: [
      "client/src/config.js",
      "server/src/config.rs",
      "server/crates/sim/src/config.rs",
      "server/crates/rules/src/balance.rs",
      "server/crates/rules/src/defs.rs",
      "server/crates/rules/src/faction.rs",
      "scripts/check-faction-catalog-parity.mjs",
      "scripts/check-wiki.mjs",
    ],
    prefixes: [],
  },
  {
    group: "server-lobby-runtime",
    exact: [],
    prefixes: ["server/src/lobby/"],
  },
  {
    group: "sim-command-service",
    exact: [
      "server/crates/sim/src/game/command.rs",
      "server/crates/sim/src/game/commands.rs",
      "server/crates/sim/src/game/services/commands.rs",
    ],
    prefixes: ["server/crates/sim/src/game/services/commands/"],
  },
  {
    group: "sim-tests",
    exact: ["server/crates/sim/src/game/tests.rs"],
    prefixes: ["server/crates/sim/src/game/tests/"],
  },
  {
    group: "sim-movement-service",
    exact: [],
    prefixes: ["server/crates/sim/src/game/services/movement/"],
  },
  {
    group: "sim-combat-service",
    exact: [],
    prefixes: ["server/crates/sim/src/game/services/combat/"],
  },
  {
    group: "client-match-shell",
    exact: [
      "client/src/app.js",
      "client/src/main.js",
      "client/src/match.js",
      "client/src/match_combat_audio.js",
      "client/src/match_health.js",
      "client/src/match_net_reporter.js",
      "client/src/match_settings_context.js",
      "client/src/frame_profiler.js",
      "client/src/frame_recovery.js",
      "client/src/frame_entity_views.js",
      "client/src/live_pause_overlay.js",
      "client/src/observer_analysis_overlay.js",
      "client/src/observer_analysis_signatures.js",
      "client/src/replay_controls.js",
      "client/src/replay_viewer.js",
      "client/src/room_capabilities.js",
    ],
    prefixes: [],
  },
  {
    group: "client-hud",
    exact: [
      "client/src/hud.js",
      "client/src/hud_command_card.js",
      "client/src/hud_command_dom.js",
      "client/src/hud_control_groups.js",
      "client/src/hud_resources.js",
      "client/src/hud_selection_panel.js",
      "client/src/hud_unit_commands.js",
      "client/src/resource_icons.js",
    ],
    prefixes: [],
  },
  {
    group: "client-state-model",
    exact: [
      "client/src/state.js",
      "client/src/state_queries.js",
      "client/src/state_visual_effects.js",
      "client/src/client_intent.js",
      "client/src/command_budget.js",
      "client/src/command_composer.js",
      "client/src/prediction_controller.js",
      "client/src/progress_extrapolator.js",
      "client/src/sim_wasm_adapter.js",
    ],
    prefixes: [],
  },
  {
    group: "client-input",
    exact: ["client/src/replay_camera_input.js"],
    prefixes: ["client/src/input/"],
  },
  {
    group: "client-renderer",
    exact: [
      "client/src/camera.js",
      "client/src/fog.js",
      "client/src/minimap.js",
    ],
    prefixes: ["client/src/renderer/"],
  },
  {
    group: "client-ui",
    exact: ["client/styles.css"],
    prefixes: ["client/src/lobby", "client/src/lab_", "client/src/settings", "client/src/match_history"],
  },
  {
    group: "ai",
    exact: [],
    prefixes: ["server/crates/ai/"],
  },
  {
    group: "scripts-tooling",
    exact: [],
    prefixes: ["scripts/"],
  },
  {
    group: "server-backend",
    exact: [],
    prefixes: ["server/src/"],
  },
  {
    group: "sim-services",
    exact: [],
    prefixes: ["server/crates/sim/src/game/services/"],
  },
  {
    group: "sim-core",
    exact: [],
    prefixes: ["server/crates/sim/"],
  },
  {
    group: "rules",
    exact: [],
    prefixes: ["server/crates/rules/"],
  },
];

function usage() {
  console.log(`Usage:
  node scripts/hotspot-analysis.mjs [options]

Options:
  --base-ref REF       Git ref to analyze. Default: HEAD
  --recent-days N      Recent churn/coupling window in days. Default: 14
  --since YYYY-MM-DD   Override the recent window start date
  --limit N            Number of ranked current files to emit. Use 0 for all. Default: 25
  --output PATH        Write JSON to PATH instead of stdout
  --repo PATH          Repository root. Default: this checkout
  -h, --help           Show this help

The script emits JSON and has no external dependencies. It analyzes current source files at the
selected ref, follows whole-file renames per current file, compares raw stale path churn, summarizes
recent co-change pressure, and rolls files into architectural groups.`);
}

function parseArgs(argv) {
  const options = {
    baseRef: "HEAD",
    recentDays: 14,
    since: null,
    limit: 25,
    output: null,
    repoRoot,
    help: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const readValue = (name) => {
      const inlinePrefix = `${name}=`;
      if (arg.startsWith(inlinePrefix)) {
        return arg.slice(inlinePrefix.length);
      }
      index += 1;
      if (index >= argv.length || argv[index].startsWith("-")) {
        throw new Error(`${name} requires a value`);
      }
      return argv[index];
    };

    if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else if (arg === "--base-ref" || arg.startsWith("--base-ref=")) {
      options.baseRef = readValue("--base-ref");
    } else if (arg === "--recent-days" || arg.startsWith("--recent-days=")) {
      options.recentDays = parseNonNegativeInteger(readValue("--recent-days"), "--recent-days");
    } else if (arg === "--since" || arg.startsWith("--since=")) {
      options.since = readDate(readValue("--since"), "--since");
    } else if (arg === "--limit" || arg.startsWith("--limit=")) {
      options.limit = parseNonNegativeInteger(readValue("--limit"), "--limit");
    } else if (arg === "--output" || arg.startsWith("--output=")) {
      options.output = readValue("--output");
    } else if (arg === "--repo" || arg.startsWith("--repo=")) {
      options.repoRoot = path.resolve(readValue("--repo"));
    } else {
      throw new Error(`unknown option: ${arg}`);
    }
  }

  return options;
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    console.error("Run with --help for usage.");
    process.exit(2);
  }

  if (options.help) {
    usage();
    return;
  }

  const report = buildReport(options);
  const json = `${JSON.stringify(report, null, 2)}\n`;
  if (options.output) {
    writeFileSync(options.output, json);
  } else {
    process.stdout.write(json);
  }
}

function buildReport(options) {
  const baseSha = git(["rev-parse", `${options.baseRef}^{commit}`], options).trim();
  const baseDate = git(["log", "-1", "--format=%cs", options.baseRef], options).trim();
  const recentSince = options.since ?? subtractDays(baseDate, options.recentDays);
  const currentFiles = listCurrentFiles(options);
  const currentFileSet = new Set(currentFiles);

  console.error(`hotspot-analysis: ${currentFiles.length} current source files at ${options.baseRef} (${baseSha.slice(0, 12)})`);
  console.error(`hotspot-analysis: recent window starts ${recentSince}`);

  const recentCoupling = collectRecentCoupling(options, currentFileSet, recentSince);
  const fileRows = [];
  for (const file of currentFiles) {
    const content = git(["show", `${options.baseRef}:${file}`], options);
    const lineCounts = countLines(content);
    const history = collectFileHistory(options, file, recentSince);
    const cochangeSet = recentCoupling.cochangeSets.get(file) ?? new Set();
    fileRows.push({
      file,
      group: classifyGroup(file),
      loc: lineCounts.loc,
      non_empty_loc: lineCounts.nonEmptyLoc,
      touches: history.touches,
      added_lines: history.addedLines,
      deleted_lines: history.deletedLines,
      recent_touches: history.recentTouches,
      recent_churn: history.recentChurn,
      fix_like_touches: history.fixLikeTouches,
      latest_touch: history.latestTouch,
      oldest_touch: history.oldestTouch,
      total_churn: history.addedLines + history.deletedLines,
      recent_cochange_degree: cochangeSet.size,
      recent_cochange_commits: recentCoupling.fileCommitCounts.get(file) ?? 0,
    });
  }

  scoreRows(fileRows);
  fileRows.sort((a, b) => b.hotspot_score - a.hotspot_score || a.file.localeCompare(b.file));
  fileRows.forEach((row, index) => {
    row.rank = index + 1;
  });

  const rawPathComparison = collectRawPathComparison(options, currentFileSet);
  const renameEvents = collectRenameEvents(options, currentFileSet);
  const groupHotspots = summarizeGroups(fileRows, recentCoupling.cochangeSets);
  const limitRows = (rows) => (options.limit === 0 ? rows : rows.slice(0, options.limit));

  return {
    schema: "hotspots-analysis-v1",
    generated_at: new Date().toISOString(),
    base_ref: options.baseRef,
    base_sha: baseSha,
    base_date: baseDate,
    recent_window_days: options.since ? null : options.recentDays,
    recent_since: recentSince,
    source_filter: {
      included_roots: INCLUDED_ROOTS.map((root) => `${root}/`),
      included_extensions: [...INCLUDED_EXTENSIONS].sort(),
      excluded_path_fragments: EXCLUDED_PATH_FRAGMENTS,
    },
    ranking_formula: SCORE_FORMULA,
    totals: {
      current_source_files: currentFiles.length,
      current_non_empty_loc: sum(fileRows, "non_empty_loc"),
      current_rename_aware_touches: sum(fileRows, "touches"),
      current_rename_aware_churn: sum(fileRows, "total_churn"),
      recent_commits_with_source_files: recentCoupling.recentCommitCount,
      rename_events_affecting_source: renameEvents.length,
      raw_paths_seen: rawPathComparison.rawPathsSeen,
      stale_raw_paths_seen: rawPathComparison.staleRawPathsSeen,
    },
    current_file_hotspots: limitRows(fileRows),
    architectural_group_hotspots: groupHotspots,
    recent_coupling_pairs: limitRows(recentCoupling.pairs),
    stale_raw_path_hotspots: limitRows(rawPathComparison.staleRows),
    rename_events: limitRows(renameEvents),
    generator_notes: {
      rename_tracking: "Per-current-file history uses git log --follow --find-renames=40% --numstat. This follows whole-file renames but cannot perfectly preserve identity after a split.",
      raw_path_comparison: "Raw no-rename path churn is included to expose stale or removed paths that should not drive cleanup by themselves.",
      group_tracking: "Architectural groups are defined in this script and documented in plans/hotspots/group-map.md; update both when a cleanup introduces new split files.",
      blame_freshness: "This repeatable script omits blame freshness to stay cheap. Use git blame -w -M -C -C on the top rows when line-origin evidence matters.",
    },
  };
}

function listCurrentFiles(options) {
  return git(["ls-tree", "-r", "--name-only", options.baseRef, "--", ...INCLUDED_ROOTS], options)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .filter(isIncludedSourcePath)
    .sort();
}

function collectFileHistory(options, file, recentSince) {
  const output = git(
    [
      "log",
      options.baseRef,
      "--no-merges",
      "--follow",
      "--find-renames=40%",
      "--numstat",
      `--format=${COMMIT_MARKER}\t%H\t%cs\t%s`,
      "--",
      file,
    ],
    options,
  );
  const stats = {
    touches: 0,
    addedLines: 0,
    deletedLines: 0,
    recentTouches: 0,
    recentChurn: 0,
    fixLikeTouches: 0,
    latestTouch: null,
    oldestTouch: null,
  };
  let current = null;

  const finishCommit = () => {
    if (!current || !current.touched) return;
    const churn = current.addedLines + current.deletedLines;
    stats.touches += 1;
    stats.addedLines += current.addedLines;
    stats.deletedLines += current.deletedLines;
    if (current.date >= recentSince) {
      stats.recentTouches += 1;
      stats.recentChurn += churn;
    }
    if (FIX_LIKE_SUBJECT.test(current.subject)) {
      stats.fixLikeTouches += 1;
    }
    stats.latestTouch = maxDate(stats.latestTouch, current.date);
    stats.oldestTouch = minDate(stats.oldestTouch, current.date);
  };

  for (const line of output.split("\n")) {
    if (!line) continue;
    if (line.startsWith(COMMIT_MARKER)) {
      finishCommit();
      const [, hash, date, subject = ""] = line.split("\t");
      current = { hash, date, subject, touched: false, addedLines: 0, deletedLines: 0 };
      continue;
    }
    if (!current) continue;
    const parsed = parseNumstatLine(line);
    if (!parsed) continue;
    current.touched = true;
    current.addedLines += parsed.added;
    current.deletedLines += parsed.deleted;
  }
  finishCommit();

  return stats;
}

function collectRecentCoupling(options, currentFileSet, recentSince) {
  const output = git(
    [
      "log",
      options.baseRef,
      "--no-merges",
      `--since=${recentSince}`,
      "--name-only",
      `--format=${COMMIT_MARKER}\t%H\t%cs\t%s`,
      "--",
      ...INCLUDED_ROOTS,
    ],
    options,
  );
  const cochangeSets = new Map();
  const fileCommitCounts = new Map();
  const pairCounts = new Map();
  let currentFiles = new Set();
  let recentCommitCount = 0;

  const finishCommit = () => {
    const files = [...currentFiles].filter((file) => currentFileSet.has(file));
    if (files.length === 0) return;
    recentCommitCount += 1;
    for (const file of files) {
      fileCommitCounts.set(file, (fileCommitCounts.get(file) ?? 0) + 1);
      if (!cochangeSets.has(file)) cochangeSets.set(file, new Set());
    }
    for (let leftIndex = 0; leftIndex < files.length; leftIndex += 1) {
      for (let rightIndex = leftIndex + 1; rightIndex < files.length; rightIndex += 1) {
        const left = files[leftIndex];
        const right = files[rightIndex];
        cochangeSets.get(left).add(right);
        cochangeSets.get(right).add(left);
        const key = [left, right].sort().join("\0");
        pairCounts.set(key, (pairCounts.get(key) ?? 0) + 1);
      }
    }
  };

  for (const line of output.split("\n")) {
    if (line.startsWith(COMMIT_MARKER)) {
      finishCommit();
      currentFiles = new Set();
      continue;
    }
    const file = line.trim();
    if (file && isIncludedSourcePath(file)) {
      currentFiles.add(file);
    }
  }
  finishCommit();

  const pairs = [...pairCounts.entries()]
    .map(([key, commits]) => {
      const [left, right] = key.split("\0");
      return {
        files: [left, right],
        commits,
        groups: [classifyGroup(left), classifyGroup(right)],
      };
    })
    .sort((a, b) => b.commits - a.commits || a.files.join("\0").localeCompare(b.files.join("\0")));

  return {
    cochangeSets,
    fileCommitCounts,
    pairs,
    recentCommitCount,
  };
}

function collectRawPathComparison(options, currentFileSet) {
  const output = git(
    [
      "log",
      options.baseRef,
      "--no-merges",
      "--no-renames",
      "--numstat",
      `--format=${COMMIT_MARKER}\t%H\t%cs\t%s`,
      "--",
      ...INCLUDED_ROOTS,
    ],
    options,
  );
  const statsByPath = new Map();
  let currentHash = null;

  for (const line of output.split("\n")) {
    if (!line) continue;
    if (line.startsWith(COMMIT_MARKER)) {
      const [, hash] = line.split("\t");
      currentHash = hash;
      continue;
    }
    const parsed = parseNumstatLine(line);
    if (!parsed || !isIncludedSourcePath(parsed.path)) continue;
    if (!statsByPath.has(parsed.path)) {
      statsByPath.set(parsed.path, { path: parsed.path, touches: 0, churn: 0, lastHash: null });
    }
    const row = statsByPath.get(parsed.path);
    if (row.lastHash !== currentHash) {
      row.touches += 1;
      row.lastHash = currentHash;
    }
    row.churn += parsed.added + parsed.deleted;
  }

  const staleRows = [...statsByPath.values()]
    .filter((row) => !currentFileSet.has(row.path))
    .map((row) => ({
      path: row.path,
      raw_touches: row.touches,
      raw_churn: row.churn,
      current_replacement_hint: replacementHint(row.path, currentFileSet),
    }))
    .sort((a, b) => b.raw_churn - a.raw_churn || a.path.localeCompare(b.path));

  return {
    rawPathsSeen: statsByPath.size,
    staleRawPathsSeen: staleRows.length,
    staleRows,
  };
}

function collectRenameEvents(options, currentFileSet) {
  const output = git(
    [
      "log",
      options.baseRef,
      "--no-merges",
      "--find-renames=40%",
      "--name-status",
      `--format=${COMMIT_MARKER}\t%H\t%cs\t%s`,
      "--",
      ...INCLUDED_ROOTS,
    ],
    options,
  );
  const events = [];
  let current = null;

  for (const line of output.split("\n")) {
    if (!line) continue;
    if (line.startsWith(COMMIT_MARKER)) {
      const [, hash, date, subject = ""] = line.split("\t");
      current = { hash, date, subject };
      continue;
    }
    if (!current || !line.startsWith("R")) continue;
    const [status, from, to] = line.split("\t");
    if (!from || !to) continue;
    if (!isIncludedSourcePath(from) && !isIncludedSourcePath(to)) continue;
    events.push({
      hash: current.hash,
      date: current.date,
      subject: current.subject,
      similarity: Number.parseInt(status.slice(1), 10),
      from,
      to,
      from_group: classifyGroup(from),
      to_group: classifyGroup(to),
      current_target_exists: currentFileSet.has(to),
    });
  }

  return events.sort((a, b) => b.date.localeCompare(a.date) || a.to.localeCompare(b.to));
}

function summarizeGroups(fileRows, cochangeSets) {
  const groups = new Map();
  for (const row of fileRows) {
    if (!groups.has(row.group)) {
      groups.set(row.group, {
        group: row.group,
        files: 0,
        non_empty_loc: 0,
        touches: 0,
        total_churn: 0,
        recent_churn: 0,
        fix_like_touches: 0,
        cochangeFiles: new Set(),
        topFiles: [],
      });
    }
    const group = groups.get(row.group);
    group.files += 1;
    group.non_empty_loc += row.non_empty_loc;
    group.touches += row.touches;
    group.total_churn += row.total_churn;
    group.recent_churn += row.recent_churn;
    group.fix_like_touches += row.fix_like_touches;
    group.topFiles.push(row);
    for (const other of cochangeSets.get(row.file) ?? []) {
      if (classifyGroup(other) !== row.group) {
        group.cochangeFiles.add(other);
      }
    }
  }

  return [...groups.values()]
    .map((group) => ({
      group: group.group,
      files: group.files,
      non_empty_loc: group.non_empty_loc,
      touches: group.touches,
      total_churn: group.total_churn,
      recent_churn: group.recent_churn,
      fix_like_touches: group.fix_like_touches,
      recent_external_cochange_degree: group.cochangeFiles.size,
      top_ranked_files: group.topFiles
        .sort((a, b) => a.rank - b.rank)
        .slice(0, 5)
        .map((row) => row.file),
    }))
    .sort(
      (a, b) =>
        b.recent_churn - a.recent_churn ||
        b.total_churn - a.total_churn ||
        a.group.localeCompare(b.group),
    );
}

function scoreRows(rows) {
  const max = {
    nonEmptyLoc: Math.max(0, ...rows.map((row) => row.non_empty_loc)),
    touches: Math.max(0, ...rows.map((row) => row.touches)),
    totalChurn: Math.max(0, ...rows.map((row) => row.total_churn)),
    recentChurn: Math.max(0, ...rows.map((row) => row.recent_churn)),
    fixLikeTouches: Math.max(0, ...rows.map((row) => row.fix_like_touches)),
    cochangeDegree: Math.max(0, ...rows.map((row) => row.recent_cochange_degree)),
  };

  for (const row of rows) {
    row.hotspot_score = round2(
      100 *
        (0.22 * damp(row.non_empty_loc, max.nonEmptyLoc) +
          0.23 * damp(row.touches, max.touches) +
          0.22 * damp(row.total_churn, max.totalChurn) +
          0.13 * damp(row.recent_churn, max.recentChurn) +
          0.10 * damp(row.fix_like_touches, max.fixLikeTouches) +
          0.10 * damp(row.recent_cochange_degree, max.cochangeDegree)),
    );
  }
}

function classifyGroup(file) {
  for (const rule of GROUP_RULES) {
    if (rule.exact?.includes(file)) return rule.group;
    if (rule.prefixes?.some((prefix) => file.startsWith(prefix))) return rule.group;
  }
  if (file.startsWith("client/")) return "client-ui";
  if (file.startsWith("server/")) return "server-backend";
  if (file.startsWith("tests/")) return "tests";
  return "other";
}

function isIncludedSourcePath(file) {
  return (
    INCLUDED_ROOTS.some((root) => file.startsWith(`${root}/`)) &&
    INCLUDED_EXTENSIONS.has(path.posix.extname(file)) &&
    !EXCLUDED_PATH_FRAGMENTS.some((fragment) => file.includes(fragment))
  );
}

function parseNumstatLine(line) {
  const parts = line.split("\t");
  if (parts.length < 3) return null;
  const [addedRaw, deletedRaw] = parts;
  if (!/^\d+$/.test(addedRaw) || !/^\d+$/.test(deletedRaw)) return null;
  const changedPath = parts[2];
  return {
    added: Number.parseInt(addedRaw, 10),
    deleted: Number.parseInt(deletedRaw, 10),
    path: normalizeNumstatPath(changedPath),
  };
}

function normalizeNumstatPath(changedPath) {
  if (!changedPath.includes(" => ")) return changedPath;
  const match = changedPath.match(/\{(.+) => (.+)\}/);
  if (match) {
    const prefix = changedPath.slice(0, match.index);
    const suffix = changedPath.slice(match.index + match[0].length);
    return `${prefix}${match[2]}${suffix}`;
  }
  return changedPath.split(" => ").at(-1);
}

function replacementHint(stalePath, currentFileSet) {
  const migratedCandidates = [
    stalePath.replace(/^server\/src\/game\/ai_core\//, "server/crates/ai/src/ai_core/"),
    stalePath.replace(/^server\/src\/game\/selfplay/, "server/crates/ai/src/selfplay"),
    stalePath.replace(/^server\/src\/game\//, "server/crates/sim/src/game/"),
  ];
  for (const candidate of migratedCandidates) {
    if (candidate !== stalePath && currentFileSet.has(candidate)) return candidate;
    const moduleRoot = candidate.replace(/\.rs$/, "/mod.rs");
    if (moduleRoot !== candidate && currentFileSet.has(moduleRoot)) return moduleRoot;
  }

  const basename = path.posix.basename(stalePath);
  const sameBasename = [...currentFileSet].filter((file) => path.posix.basename(file) === basename);
  if (sameBasename.length === 1) return sameBasename[0];
  if (sameBasename.length > 1) return `same basename: ${sameBasename.slice(0, 3).join(", ")}`;
  return null;
}

function countLines(content) {
  const lines = content.length === 0 ? [] : content.split(/\r?\n/);
  if (lines.at(-1) === "") lines.pop();
  return {
    loc: lines.length,
    nonEmptyLoc: lines.filter((line) => line.trim().length > 0).length,
  };
}

function git(args, options) {
  return execFileSync("git", args, {
    cwd: options.repoRoot,
    encoding: "utf8",
    maxBuffer: MAX_BUFFER,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function parseNonNegativeInteger(value, name) {
  if (!/^\d+$/.test(value)) {
    throw new Error(`${name} must be a non-negative integer`);
  }
  return Number.parseInt(value, 10);
}

function readDate(value, name) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    throw new Error(`${name} must use YYYY-MM-DD`);
  }
  return value;
}

function subtractDays(dateString, days) {
  const [year, month, day] = dateString.split("-").map((part) => Number.parseInt(part, 10));
  const date = new Date(Date.UTC(year, month - 1, day));
  date.setUTCDate(date.getUTCDate() - days);
  return date.toISOString().slice(0, 10);
}

function maxDate(left, right) {
  if (!left) return right;
  return left > right ? left : right;
}

function minDate(left, right) {
  if (!left) return right;
  return left < right ? left : right;
}

function damp(value, max) {
  return max > 0 ? Math.sqrt(value / max) : 0;
}

function round2(value) {
  return Math.round(value * 100) / 100;
}

function sum(rows, field) {
  return rows.reduce((total, row) => total + row[field], 0);
}

main();
