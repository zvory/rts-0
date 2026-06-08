#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import path from "node:path";

const suiteOrder = [
  "crate-boundaries",
  "cargo-fmt",
  "cargo-test-contract-protocol",
  "cargo-test-rules",
  "cargo-test-sim",
  "cargo-test-ai",
  "cargo-test-server",
  "cargo-clippy",
  "js-protocol-contracts",
  "node-server-integration",
  "node-regression",
  "node-ai-integration",
  "node-minimap-input-contracts",
  "client-smoke",
  "full-ai",
  "docs-only",
];

function usage() {
  console.log(`usage:
  node tests/select-suites.mjs --from=<git-ref>
  node tests/select-suites.mjs --staged
  node tests/select-suites.mjs <file>...
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
    pathname === "docs/design/balance.md" ||
    pathname.startsWith("docs/context/balance")
  );
}

export function selectSuites(files) {
  const suites = new Set();
  let docsOnly = files.length > 0;

  for (const pathname of files) {
    const normalized = pathname.split(path.sep).join("/");
    const rustCode = normalized.startsWith("server/") && normalized.endsWith(".rs");
    const ciOrScript = normalized.startsWith(".github/") || normalized.startsWith("scripts/") || normalized === "tests/run-all.sh";
    docsOnly &&= normalized.startsWith("docs/") || normalized.startsWith("plans/") || normalized.endsWith(".md");

    if (rustCode || normalized.startsWith("server/Cargo.")) {
      addAll(suites, ["crate-boundaries", "cargo-fmt", "cargo-clippy"]);
    }

    if (ciOrScript) {
      addAll(suites, ["crate-boundaries", "cargo-fmt", "cargo-test-server", "cargo-clippy", "node-regression"]);
    }

    if (normalized.startsWith("server/crates/contract/") || isProtocolShape(normalized)) {
      addAll(suites, [
        "cargo-test-contract-protocol",
        "js-protocol-contracts",
        "node-server-integration",
      ]);
    }

    if (normalized.startsWith("server/crates/rules/") || isRulesVisibleBalance(normalized)) {
      addAll(suites, ["cargo-test-rules", "cargo-test-sim"]);
      if (isRulesVisibleBalance(normalized)) {
        suites.add("js-protocol-contracts");
      }
    }

    if (normalized.startsWith("server/crates/sim/")) {
      addAll(suites, ["cargo-test-sim", "node-server-integration"]);
    }

    if (normalized.startsWith("server/crates/ai/")) {
      addAll(suites, ["cargo-test-ai", "node-ai-integration"]);
      if (
        normalized.includes("/ai_core/") ||
        normalized.includes("/selfplay/") ||
        normalized.includes("profiles")
      ) {
        suites.add("full-ai");
      }
    }

    if (normalized.startsWith("server/src/")) {
      addAll(suites, ["cargo-test-server", "node-server-integration", "node-regression"]);
      if (normalized.includes("lobby") || normalized.includes("main.rs")) {
        suites.add("node-ai-integration");
      }
      if (normalized.includes("snapshots") || normalized.includes("room_task")) {
        suites.add("client-smoke");
      }
    }

    if (normalized.startsWith("client/")) {
      addAll(suites, ["js-protocol-contracts", "node-minimap-input-contracts", "client-smoke"]);
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
        "cargo-test-contract-protocol",
        "cargo-test-rules",
        "cargo-test-sim",
        "cargo-test-ai",
        "cargo-test-server",
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

function verify() {
  const cases = [
    [["server/crates/protocol/src/lib.rs"], ["cargo-test-contract-protocol", "js-protocol-contracts", "node-server-integration"]],
    [["server/crates/rules/src/balance.rs"], ["cargo-test-rules", "cargo-test-sim", "js-protocol-contracts"]],
    [["server/crates/sim/src/game/systems.rs"], ["cargo-test-sim", "node-server-integration"]],
    [["server/crates/ai/src/ai_core/profiles.rs"], ["cargo-test-ai", "node-ai-integration", "full-ai"]],
    [["server/src/lobby/room_task.rs"], ["cargo-test-server", "node-server-integration", "node-regression", "node-ai-integration", "client-smoke"]],
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
