#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const suiteOrder = [
  "source-file-sizes",
  "crate-boundaries",
  "cargo-fmt",
  "nextest-contract-protocol",
  "nextest-rules",
  "nextest-sim",
  "nextest-ai",
  "nextest-server",
  "cargo-clippy",
  "agent-workflow-phase-runner",
  "agent-workflow-quality-pass",
  "client-architecture",
  "faction-assumptions",
  "faction-catalog-parity",
  "js-protocol-contracts",
  "node-server-integration",
  "node-regression",
  "node-ai-integration",
  "node-team-integration",
  "node-minimap-input-contracts",
  "interact-contracts",
  "client-smoke",
  "full-ai",
  "docs-only",
];

function usage() {
  console.log(`usage:
  node tests/select-suites.mjs --from=<git-ref>
  node tests/select-suites.mjs --staged
  node tests/select-suites.mjs <file>...
  node tests/select-suites.mjs --ci-policy <file>...
  node tests/select-suites.mjs --verify`);
}

function changedFrom(ref) {
  const out = execFileSync("git", ["diff", "--name-only", `${ref}...HEAD`], { encoding: "utf8" });
  return out.split("\n").filter(Boolean);
}

function staged() {
  const out = execFileSync("git", ["diff", "--cached", "--name-only"], { encoding: "utf8" });
  return out.split("\n").filter(Boolean);
}

function addAll(set, suites) {
  for (const suite of suites) {
    set.add(suite);
  }
}

const configModulePolicies = new Map([
  [
    "client/src/config.js",
    {
      ciClass: "full",
      suites: ["nextest-rules", "nextest-sim", "nextest-server", "faction-assumptions", "faction-catalog-parity", "js-protocol-contracts"],
    },
  ],
  [
    "client/src/config/rules_mirror.js",
    { ciClass: "full", suites: ["nextest-rules", "nextest-sim", "js-protocol-contracts"] },
  ],
  [
    "client/src/config/factions.js",
    {
      ciClass: "full",
      suites: ["nextest-rules", "nextest-sim", "faction-assumptions", "faction-catalog-parity", "js-protocol-contracts"],
    },
  ],
  [
    "client/src/config/timing.js",
    { ciClass: "full", suites: ["nextest-rules", "nextest-sim", "js-protocol-contracts"] },
  ],
  [
    "client/src/config/player_palette_mirror.js",
    { ciClass: "full", suites: ["nextest-server", "js-protocol-contracts"] },
  ],
  ["client/src/config/presentation.js", { ciClass: "client_only", suites: [] }],
]);

const ciClientFullFallbackPaths = new Set([
  ...[...configModulePolicies]
    .filter(([, policy]) => policy.ciClass === "full")
    .map(([modulePath]) => modulePath),
  "client/src/lobby_view.js",
  "client/src/net.js",
  "client/src/protocol.js",
]);

const ciClientFullFallbackPrefixes = [
  "client/vendor/sim-wasm/",
];

function normalizeChangedPath(pathname) {
  return pathname.split(path.sep).join("/");
}

function isMarkdownPath(pathname) {
  return pathname.endsWith(".md");
}

function isClientOnlyCiPath(pathname) {
  if (!pathname.startsWith("client/")) {
    return false;
  }
  if (ciClientFullFallbackPaths.has(pathname)) {
    return false;
  }
  return !ciClientFullFallbackPrefixes.some((prefix) => pathname.startsWith(prefix));
}

function isSourceFileSizePath(pathname) {
  return (
    (pathname.startsWith("client/") && pathname.endsWith(".css")) ||
    pathname === "scripts/source-file-size-baseline.json" ||
    pathname === "tests/run-all.sh" ||
    (
      (
        pathname.startsWith("server/") ||
        pathname.startsWith("client/src/") ||
        pathname.startsWith("tests/") ||
        pathname.startsWith("scripts/")
      ) &&
      (
        pathname.endsWith(".rs") ||
        pathname.endsWith(".js") ||
        pathname.endsWith(".mjs") ||
        pathname.endsWith(".ts")
      )
    )
  );
}

function isPhaseRunnerWorkflowPath(pathname) {
  return (
    pathname === "scripts/phase-runner.sh" ||
    pathname === "scripts/phase-runner-agents.mjs" ||
    pathname === "scripts/plan-phase-status.mjs" ||
    pathname === "scripts/phase-runner-result.schema.json" ||
    pathname === "tests/phase_runner_agents.mjs" ||
    pathname === "tests/run-all.sh"
  );
}

function isQualityPassWorkflowPath(pathname) {
  return (
    pathname === "scripts/adversarial-quality-pass.mjs" ||
    pathname === "scripts/adversarial-quality-pass.schema.json" ||
    pathname === "scripts/agent-pr-passes.mjs" ||
    pathname === "scripts/agent-pr-passes.json" ||
    pathname === "scripts/patch-note-pass.mjs" ||
    pathname === "scripts/patch-note-pass.schema.json" ||
    pathname === "scripts/agent-pr.sh" ||
    pathname === "scripts/archive-completed-plans.mjs" ||
    pathname === "scripts/plan-phase-status.mjs" ||
    pathname === "scripts/format-touched-rust.sh" ||
    pathname === "tests/archive_completed_plans.mjs" ||
    pathname === "tests/adversarial_quality_pass.mjs" ||
    pathname === "tests/agent_pr_passes.mjs" ||
    pathname === "tests/run-all.sh"
  );
}

function isInteractPath(pathname) {
  return (
    pathname.startsWith("scripts/interact/") ||
    pathname === "scripts/tailnet-preview.mjs" ||
    pathname === "scripts/tailnet-preview.d.mts" ||
    pathname === "scripts/check-interact-architecture.mjs" ||
    pathname === "client/src/interact_bridge.js" ||
    pathname === "server/src/interact_lab_artifacts.rs" ||
    pathname.startsWith("tests/interact_") ||
    pathname.startsWith("tests/client_contracts/interact_") ||
    pathname.startsWith("tests/fixtures/interact_") ||
    pathname === "package.json" ||
    pathname === "package-lock.json" ||
    pathname === "docs/interact-cli.md" ||
    pathname === "docs/context/testing.md" ||
    pathname === "docs/design/testing.md" ||
    pathname.startsWith(".agents/skills/interact/")
  );
}

export function ciPolicy(files) {
  const normalized = files.map(normalizeChangedPath).filter(Boolean);
  let ciClass = "full";

  if (normalized.length > 0 && normalized.every(isMarkdownPath)) {
    ciClass = "docs_only";
  } else if (normalized.length > 0 && normalized.every(isClientOnlyCiPath)) {
    ciClass = "client_only";
  }

  return {
    ci_class: ciClass,
    docs_only: ciClass === "docs_only",
    run_server_build: ciClass !== "docs_only",
    run_rust: ciClass === "full",
    run_live_node: ciClass !== "docs_only",
    run_browser: ciClass !== "docs_only",
  };
}

function printCiPolicy(policy) {
  for (const key of ["ci_class", "docs_only", "run_server_build", "run_rust", "run_live_node", "run_browser"]) {
    console.log(`${key}=${policy[key]}`);
  }
}

function isProtocolShape(pathname) {
  return (
    pathname === "server/crates/protocol/src/lib.rs" ||
    pathname === "server/src/protocol.rs" ||
    pathname === "client/src/protocol.js" ||
    pathname === "docs/design/protocol.md" ||
    pathname.startsWith("docs/context/protocol")
  );
}

function isRulesVisibleBalance(pathname) {
  return (
    pathname === "server/crates/rules/src/balance.rs" ||
    pathname === "server/src/config.rs" ||
    pathname === "server/crates/sim/src/config.rs" ||
    pathname === "client/src/config.js" ||
    pathname === "client/src/config/rules_mirror.js" ||
    pathname === "docs/design/balance.md" ||
    pathname.startsWith("docs/context/balance")
  );
}

function isFactionDocsOrPlan(pathname) {
  return (
    pathname === "docs/design/faction-architecture-inventory.md" ||
    pathname.startsWith("docs/context/faction") ||
    pathname.startsWith("plans/factionguardrails/")
  );
}

function isFactionCatalogSurface(pathname) {
  return (
    pathname === "server/crates/rules/src/faction.rs" ||
    pathname === "server/crates/rules/src/bin/dump-faction-catalog.rs" ||
    pathname === "client/src/config.js" ||
    pathname === "client/src/config/factions.js" ||
    pathname === "client/src/lobby_view.js"
  );
}

function isFactionRuntimeSurface(pathname) {
  return (
    pathname === "server/src/lobby/faction_validation.rs" ||
    pathname === "tests/faction_integration.mjs"
  );
}

function isFactionChecker(pathname) {
  return (
    pathname === "scripts/check-faction-assumptions.mjs" ||
    pathname === "scripts/check-faction-catalog-parity.mjs"
  );
}

function addFactionSuites(suites, pathname) {
  if (
    isFactionDocsOrPlan(pathname) ||
    isFactionCatalogSurface(pathname) ||
    isFactionRuntimeSurface(pathname) ||
    isProtocolShape(pathname) ||
    pathname === "server/src/config.rs" ||
    isFactionChecker(pathname)
  ) {
    suites.add("faction-assumptions");
  }
  if (
    isFactionCatalogSurface(pathname) ||
    isProtocolShape(pathname) ||
    pathname === "server/src/config.rs" ||
    pathname === "scripts/check-faction-catalog-parity.mjs"
  ) {
    suites.add("faction-catalog-parity");
  }
}

function isTeamRelated(pathname) {
  return (
    pathname === "tests/team_integration.mjs" ||
    pathname === "tests/team_harness.mjs" ||
    pathname === "server/crates/sim/src/game/teams.rs" ||
    pathname === "server/crates/sim/src/game/snapshot.rs" ||
    pathname === "server/crates/sim/src/game/fog.rs" ||
    pathname === "server/crates/sim/src/game/building_memory.rs" ||
    pathname === "server/crates/sim/src/game/map/authored/assignment.rs" ||
    pathname === "server/crates/sim/src/game/map/team_assignment_tests.rs" ||
    pathname === "server/src/lobby/room_task.rs" ||
    pathname === "server/src/lobby/team_setup.rs" ||
    pathname === "client/src/state.js" ||
    pathname === "client/src/lobby.js" ||
    pathname === "client/src/lobby_view.js" ||
    pathname === "client/src/scoreboard.js" ||
    pathname === "client/src/replay_viewer.js" ||
    pathname === "client/src/replay_controls.js" ||
    pathname.startsWith("client/src/input/") ||
    pathname.startsWith("server/crates/ai/src/") ||
    pathname.startsWith("server/crates/sim/src/game/services/combat/") ||
    pathname.startsWith("server/crates/sim/src/game/services/commands") ||
    pathname.startsWith("server/crates/sim/src/game/services/world_query") ||
    pathname.startsWith("server/crates/sim/src/rules/projection") ||
    isProtocolShape(pathname) ||
    pathname.startsWith("plans/teams/") ||
    [
      "docs/context/client-ui.md",
      "docs/context/match-history.md",
      "docs/context/protocol.md",
      "docs/context/server-sim.md",
      "docs/context/testing.md",
      "docs/design/ai.md",
      "docs/design/client-ui.md",
      "docs/design/match-history.md",
      "docs/design/protocol.md",
      "docs/design/server-sim.md",
      "docs/design/testing.md",
    ].includes(pathname) ||
    pathname.includes("team")
  );
}

export function selectSuites(files) {
  const suites = new Set();
  let docsOnly = files.length > 0;

  for (const pathname of files) {
    const normalized = normalizeChangedPath(pathname);
    const rustCode = normalized.startsWith("server/") && normalized.endsWith(".rs");
    const ciOrScript =
      normalized.startsWith(".github/") ||
      (normalized.startsWith("scripts/") && !isFactionChecker(normalized)) ||
      normalized === "tests/run-all.sh";
    const clientArchitecturePolicy =
      normalized.startsWith("client/src/") ||
      normalized === "scripts/check-client-architecture.mjs" ||
      normalized === "tests/run-all.sh" ||
      normalized === "tests/select-suites.mjs" ||
      normalized.startsWith("plans/archive/client-arch/");
    docsOnly &&= normalized.startsWith("docs/") || normalized.startsWith("plans/") || normalized.endsWith(".md");

    if (isSourceFileSizePath(normalized)) {
      suites.add("source-file-sizes");
    }

    if (isInteractPath(normalized)) {
      addAll(suites, ["interact-contracts", "client-smoke"]);
    }

    if (rustCode || normalized.startsWith("server/Cargo.")) {
      addAll(suites, ["crate-boundaries", "cargo-fmt", "cargo-clippy"]);
    }

    if (ciOrScript) {
      addAll(suites, ["crate-boundaries", "cargo-fmt", "nextest-server", "cargo-clippy", "node-regression"]);
    }

    if (isPhaseRunnerWorkflowPath(normalized)) {
      suites.add("agent-workflow-phase-runner");
    }

    if (isQualityPassWorkflowPath(normalized)) {
      suites.add("agent-workflow-quality-pass");
    }

    if (clientArchitecturePolicy) {
      suites.add("client-architecture");
    }

    addAll(suites, configModulePolicies.get(normalized)?.suites || []);

    addFactionSuites(suites, normalized);

    if (normalized.startsWith("server/crates/contract/") || isProtocolShape(normalized)) {
      addAll(suites, [
        "nextest-contract-protocol",
        "js-protocol-contracts",
        "node-server-integration",
      ]);
    }

    if (normalized.startsWith("server/crates/rules/") || isRulesVisibleBalance(normalized)) {
      addAll(suites, ["nextest-rules", "nextest-sim"]);
      if (isRulesVisibleBalance(normalized)) {
        suites.add("js-protocol-contracts");
      }
    }

    if (normalized.startsWith("server/crates/sim/")) {
      addAll(suites, ["nextest-sim", "node-server-integration"]);
    }

    if (normalized.startsWith("server/crates/ai/")) {
      addAll(suites, ["nextest-ai", "node-ai-integration"]);
      if (
        normalized.includes("/ai_core/") ||
        normalized.includes("/selfplay/") ||
        normalized.includes("profiles")
      ) {
        suites.add("full-ai");
      }
    }

    if (normalized.startsWith("server/src/")) {
      addAll(suites, ["nextest-server", "node-server-integration", "node-regression"]);
      if (normalized.includes("lobby") || normalized.includes("main.rs")) {
        suites.add("node-ai-integration");
      }
      if (normalized.includes("snapshots") || normalized.includes("room_task")) {
        suites.add("client-smoke");
      }
    }

    if (isTeamRelated(normalized)) {
      suites.add("node-team-integration");
    }

    if (normalized.startsWith("client/")) {
      addAll(suites, ["client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]);
      if (normalized.includes("net") || normalized.includes("protocol")) {
        suites.add("node-server-integration");
      }
    }

    if (normalized.startsWith("tests/") && normalized !== "tests/select-suites.mjs") {
      addAll(suites, ["node-server-integration", "node-regression", "node-ai-integration"]);
      if (normalized.includes("client") || normalized.includes("minimap")) {
        addAll(suites, ["node-minimap-input-contracts", "client-smoke"]);
      }
    }

    if (normalized === "server/Cargo.toml" || normalized === "server/Cargo.lock") {
      addAll(suites, [
        "nextest-contract-protocol",
        "nextest-rules",
        "nextest-sim",
        "nextest-ai",
        "nextest-server",
      ]);
    }
  }

  if (files.length === 0) {
    return [];
  }
  if (suites.size === 0 && docsOnly) {
    suites.add("docs-only");
  }
  return suiteOrder.filter((suite) => suites.has(suite));
}

const normalRustContractFiles = [
  "CLAUDE.md",
  ".github/workflows/main-tests.yml",
  "docs/context/testing.md",
  "docs/design/testing.md",
  "docs/pr-first-workflow.md",
  "server/Cargo.toml",
  "tests/README.md",
  "tests/run-all.sh",
  "tools/context",
  "runaireplay.sh",
];

function verifyNextestPolicy(failures) {
  for (const suite of suiteOrder) {
    if (suite.startsWith("cargo-test")) {
      failures.push(`suite selector still exposes retired Rust suite name ${suite}`);
    }
  }

  const retiredCargoTimingScript = ["cargo", "test", "timed"].join("-");
  const retiredCargoTimingEnv = ["RTS", "CARGO", "PACKAGE", "TIMINGS"].join("_");
  const retiredPackageTiming = ["package", "by", "package"].join("-");
  const retiredPatterns = [
    [retiredCargoTimingScript, new RegExp(retiredCargoTimingScript)],
    [retiredCargoTimingEnv, new RegExp(retiredCargoTimingEnv)],
    [`${retiredPackageTiming} timing`, /package[- ]by[- ]package/i],
  ];
  const cargoTest = ["cargo", "test"].join(" ");
  const plainCargoTest = new RegExp(`(^|[^A-Za-z0-9_-])${cargoTest}(?!\\s+--doc\\b)`);

  for (const file of normalRustContractFiles) {
    const text = readFileSync(path.join(repoRoot, file), "utf8");
    const lines = text.split("\n");
    lines.forEach((line, index) => {
      for (const [label, pattern] of retiredPatterns) {
        if (pattern.test(line)) {
          failures.push(`${file}:${index + 1} mentions retired ${label}`);
        }
      }
      if (plainCargoTest.test(line)) {
        failures.push(`${file}:${index + 1} mentions built-in Rust test command outside doctest guidance`);
      }
    });
  }
}

function verify() {
  const cases = [
    [["server/crates/protocol/src/lib.rs"], ["nextest-contract-protocol", "js-protocol-contracts", "node-server-integration", "node-team-integration"]],
    [["server/crates/rules/src/balance.rs"], ["nextest-rules", "nextest-sim", "js-protocol-contracts"]],
    [["server/crates/sim/src/game/systems.rs"], ["nextest-sim", "node-server-integration"]],
    [["server/crates/sim/src/game/teams.rs"], ["nextest-sim", "node-server-integration", "node-team-integration"]],
    [["server/crates/sim/src/game/map/authored/assignment.rs"], ["nextest-sim", "node-server-integration", "node-team-integration"]],
    [["server/crates/ai/src/ai_core/profiles.rs"], ["nextest-ai", "node-ai-integration", "node-team-integration", "full-ai"]],
    [["server/src/lobby/room_task.rs"], ["nextest-server", "node-server-integration", "node-regression", "node-ai-integration", "node-team-integration", "client-smoke"]],
    [["client/src/match.js"], ["client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    [["client/src/config.js"], ["nextest-rules", "nextest-sim", "nextest-server", "faction-assumptions", "faction-catalog-parity", "js-protocol-contracts", "client-architecture", "client-smoke"]],
    [["client/src/config/rules_mirror.js"], ["nextest-rules", "nextest-sim", "js-protocol-contracts", "client-architecture", "client-smoke"]],
    [["client/src/config/factions.js"], ["nextest-rules", "nextest-sim", "faction-assumptions", "faction-catalog-parity", "js-protocol-contracts", "client-architecture", "client-smoke"]],
    [["client/src/config/timing.js"], ["nextest-rules", "nextest-sim", "js-protocol-contracts", "client-architecture", "client-smoke"]],
    [["client/src/config/player_palette_mirror.js"], ["nextest-server", "js-protocol-contracts", "client-architecture", "client-smoke"]],
    [["client/styles.css"], ["source-file-sizes", "client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    [["client/connection_lost.css"], ["source-file-sizes", "client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    [["client/src/state.js"], ["client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "node-team-integration", "client-smoke"]],
    [["scripts/check-client-architecture.mjs"], ["client-architecture"]],
    [["scripts/interact/driver.ts"], ["source-file-sizes", "interact-contracts", "client-smoke"]],
    [["scripts/interact/command_service.ts"], ["source-file-sizes", "interact-contracts", "client-smoke"]],
    [["scripts/interact/process_runner.ts"], ["source-file-sizes", "interact-contracts", "client-smoke"]],
    [["scripts/interact/private_server.ts"], ["source-file-sizes", "interact-contracts", "client-smoke"]],
    [["package.json"], ["interact-contracts", "client-smoke"]],
    [["scripts/check-interact-architecture.mjs"], ["interact-contracts", "client-smoke"]],
    [["client/src/interact_bridge.js"], ["interact-contracts", "client-smoke"]],
    [["server/src/interact_lab_artifacts.rs"], ["interact-contracts", "client-smoke"]],
    [["tests/client_contracts/interact_capture_contracts.mjs"], ["interact-contracts", "client-smoke"]],
    [["docs/interact-cli.md"], ["interact-contracts", "client-smoke"]],
    [[".agents/skills/interact/SKILL.md"], ["interact-contracts", "client-smoke"]],
    [["plans/archive/client-arch/phase-1.md"], ["client-architecture"]],
    [["plans/teams/phase-1.md"], ["node-team-integration"]],
    [["tests/team_harness.mjs"], ["node-server-integration", "node-regression", "node-ai-integration", "node-team-integration"]],
    [["scripts/phase-runner-agents.mjs"], ["agent-workflow-phase-runner"]],
    [["scripts/phase-runner-result.schema.json"], ["agent-workflow-phase-runner"]],
    [["scripts/adversarial-quality-pass.mjs"], ["agent-workflow-quality-pass"]],
    [["scripts/adversarial-quality-pass.schema.json"], ["agent-workflow-quality-pass"]],
    [["scripts/agent-pr-passes.mjs"], ["agent-workflow-quality-pass"]],
    [["scripts/agent-pr-passes.json"], ["agent-workflow-quality-pass"]],
    [["scripts/patch-note-pass.mjs"], ["agent-workflow-quality-pass"]],
    [["scripts/patch-note-pass.schema.json"], ["agent-workflow-quality-pass"]],
    [["scripts/agent-pr.sh"], ["agent-workflow-quality-pass"]],
    [["scripts/archive-completed-plans.mjs"], ["agent-workflow-quality-pass"]],
    [["scripts/plan-phase-status.mjs"], ["agent-workflow-phase-runner", "agent-workflow-quality-pass"]],
    [["scripts/format-touched-rust.sh"], ["agent-workflow-quality-pass"]],
    [["tests/run-all.sh"], ["agent-workflow-phase-runner", "agent-workflow-quality-pass"]],
    [["server/crates/rules/src/faction.rs"], ["nextest-rules", "nextest-sim", "faction-assumptions", "faction-catalog-parity"]],
    [["client/src/lobby_view.js"], ["client-architecture", "faction-assumptions", "faction-catalog-parity", "js-protocol-contracts"]],
    [["server/src/lobby/faction_validation.rs"], ["nextest-server", "faction-assumptions", "node-server-integration", "node-regression", "node-ai-integration"]],
    [["scripts/check-faction-assumptions.mjs"], ["faction-assumptions"]],
    [["scripts/check-faction-catalog-parity.mjs"], ["faction-assumptions", "faction-catalog-parity"]],
    [["docs/design/faction-architecture-inventory.md"], ["faction-assumptions"]],
    [["plans/factionguardrails/phase-6.md"], ["faction-assumptions"]],
    [["docs/design/architecture.md"], ["docs-only"]],
  ];

  const failures = [];
  for (const [files, expected] of cases) {
    const actual = selectSuites(files);
    for (const suite of expected) {
      if (!actual.includes(suite)) {
        failures.push(`${files.join(", ")} did not select ${suite}; got ${actual.join(", ")}`);
      }
    }
  }

  const exactConfigCases = [
    ["client/src/config.js", ["source-file-sizes", "nextest-rules", "nextest-sim", "nextest-server", "client-architecture", "faction-assumptions", "faction-catalog-parity", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    ["client/src/config/rules_mirror.js", ["source-file-sizes", "nextest-rules", "nextest-sim", "client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    ["client/src/config/factions.js", ["source-file-sizes", "nextest-rules", "nextest-sim", "client-architecture", "faction-assumptions", "faction-catalog-parity", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    ["client/src/config/timing.js", ["source-file-sizes", "nextest-rules", "nextest-sim", "client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    ["client/src/config/player_palette_mirror.js", ["source-file-sizes", "nextest-server", "client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
    ["client/src/config/presentation.js", ["source-file-sizes", "client-architecture", "js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]],
  ];
  for (const [file, expected] of exactConfigCases) {
    const actual = selectSuites([file]);
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
      failures.push(`${file} selected ${actual.join(", ")}; expected exactly ${expected.join(", ")}`);
    }
  }

  for (const files of [
    ["scripts/check-faction-assumptions.mjs"],
    ["scripts/check-faction-catalog-parity.mjs"],
    ["docs/design/faction-architecture-inventory.md"],
    ["plans/factionguardrails/phase-6.md"],
  ]) {
    const actual = selectSuites(files);
    for (const suite of ["node-server-integration", "node-regression", "node-ai-integration", "client-smoke"]) {
      if (actual.includes(suite)) {
        failures.push(`${files.join(", ")} should not select live-server suite ${suite}; got ${actual.join(", ")}`);
      }
    }
    if (actual.includes("docs-only")) {
      failures.push(`${files.join(", ")} should select faction guardrails instead of docs-only; got ${actual.join(", ")}`);
    }
  }

  const policyCases = [
    [[], { ci_class: "full", run_rust: true }],
    [["docs/design/architecture.md"], { ci_class: "docs_only", run_rust: false, run_server_build: false }],
    [["README.md", "--not-markdown.rs"], { ci_class: "full", run_rust: true }],
    [["client/src/match.js"], { ci_class: "client_only", run_rust: false, run_server_build: true }],
    [["client/src/renderer/units.js", "client/assets/sound/ui/ui_victory_01.mp3"], { ci_class: "client_only", run_rust: false }],
    [["client/src/config.js"], { ci_class: "full", run_rust: true }],
    [["client/src/protocol.js"], { ci_class: "full", run_rust: true }],
    [["client/src/net.js"], { ci_class: "full", run_rust: true }],
    [["client/src/lobby_view.js"], { ci_class: "full", run_rust: true }],
    [["client/src/config/rules_mirror.js"], { ci_class: "full", run_rust: true }],
    [["client/src/config/factions.js"], { ci_class: "full", run_rust: true }],
    [["client/src/config/timing.js"], { ci_class: "full", run_rust: true }],
    [["client/src/config/player_palette_mirror.js"], { ci_class: "full", run_rust: true }],
    [["client/src/config/presentation.js"], { ci_class: "client_only", run_rust: false }],
    [["client/styles.css"], { ci_class: "client_only", run_rust: false }],
    [["client/vendor/sim-wasm/rts_sim_wasm.js"], { ci_class: "full", run_rust: true }],
    [["client/src/match.js", "server/src/main.rs"], { ci_class: "full", run_rust: true }],
  ];

  for (const [files, expected] of policyCases) {
    const actual = ciPolicy(files);
    for (const [key, value] of Object.entries(expected)) {
      if (actual[key] !== value) {
        failures.push(`${files.join(", ") || "(no files)"} ci policy ${key}=${actual[key]}; expected ${value}`);
      }
    }
  }

  for (const [modulePath, policy] of configModulePolicies) {
    if (!["full", "client_only"].includes(policy.ciClass)) {
      failures.push(`${modulePath} has invalid config CI classification ${policy.ciClass}`);
    } else if (ciPolicy([modulePath]).ci_class !== policy.ciClass) {
      failures.push(`${modulePath} does not apply its declared ${policy.ciClass} config CI classification`);
    }
  }

  const classifiedConfigModules = new Set(configModulePolicies.keys());
  const productionConfigModules = readdirSync(path.join(repoRoot, "client/src/config"), { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".js"))
    .map((entry) => `client/src/config/${entry.name}`);
  for (const modulePath of productionConfigModules) {
    if (!classifiedConfigModules.has(modulePath)) {
      failures.push(`${modulePath} has no explicit mirror-or-client-owned config classification`);
    }
  }

  verifyNextestPolicy(failures);

  if (failures.length > 0) {
    console.error("test selector verification failed:");
    for (const failure of failures) {
      console.error(`  - ${failure}`);
    }
    process.exit(1);
  }
  console.log("test selector verification passed");
}

const args = process.argv.slice(2);
let files = [];

const ciPolicyIndex = args.indexOf("--ci-policy");
if (ciPolicyIndex >= 0) {
  printCiPolicy(ciPolicy(args.slice(ciPolicyIndex + 1)));
  process.exit(0);
}

if (args.includes("--help") || args.includes("-h")) {
  usage();
  process.exit(0);
}

if (args.includes("--verify")) {
  verify();
  process.exit(0);
}

const fromArg = args.find((arg) => arg.startsWith("--from="));
if (fromArg) {
  files = changedFrom(fromArg.slice("--from=".length));
} else if (args.includes("--staged")) {
  files = staged();
} else {
  files = args.filter((arg) => !arg.startsWith("--"));
}

for (const suite of selectSuites(files)) {
  console.log(suite);
}
