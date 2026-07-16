import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function assertIncludes(text, needle, message) {
  if (!text.includes(needle)) {
    throw new Error(message);
  }
}

function assertNotIncludes(text, needle, message) {
  if (text.includes(needle)) {
    throw new Error(message);
  }
}

function assertNotMatches(text, pattern, message) {
  if (pattern.test(text)) {
    throw new Error(message);
  }
}

function assertMatches(text, pattern, message) {
  if (!pattern.test(text)) {
    throw new Error(message);
  }
}

const dockerfile = read("Dockerfile");
const dockerignore = read(".dockerignore");
const cargoLock = read("server/Cargo.lock");
const mainTestWorkflow = read(".github/workflows/main-tests.yml");
const betaDeployWorkflow = read(".github/workflows/deploy-beta.yml");
const wasmBuildScript = read("scripts/build-sim-wasm.sh");
const wasmGitignore = read("client/vendor/sim-wasm/.gitignore");
const deployScript = read("deploy.sh");
const clientIndex = read("client/index.html");
const mainlineFlyConfig = read("fly.mainline.toml");
const betaFlyConfig = read("fly.beta.toml");
const serverMain = read("server/src/main.rs");
const wasmBindgenLockVersion = cargoLock.match(
  /\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"/,
)?.[1];
const wasmBindgenDockerVersion = dockerfile.match(/ARG WASM_BINDGEN_CLI_VERSION=([^\s]+)/)?.[1];
const runtimeStage = dockerfile.slice(dockerfile.indexOf("FROM debian:bookworm-slim AS runtime"));
const runtimePackages = runtimeStage.match(/apt-get install -y --no-install-recommends([\s\S]*?)&&/)?.[1] || "";
const dockerignoreEntries = new Set(
  dockerignore
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#")),
);

for (const localOnlyPath of [".git", ".docdrift", "desktop"]) {
  if (!dockerignoreEntries.has(localOnlyPath)) {
    throw new Error(`.dockerignore must exclude local-only ${localOnlyPath} from Fly build contexts`);
  }
}

if (!wasmBindgenLockVersion) {
  throw new Error("server/Cargo.lock must include a wasm-bindgen package entry");
}
if (wasmBindgenDockerVersion !== wasmBindgenLockVersion) {
  throw new Error(
    `Dockerfile WASM_BINDGEN_CLI_VERSION=${wasmBindgenDockerVersion || "<missing>"} must match Cargo.lock wasm-bindgen ${wasmBindgenLockVersion}`,
  );
}

assertIncludes(
  wasmGitignore,
  "*",
  "client/vendor/sim-wasm keeps generated assets ignored, so Docker must build them",
);
assertIncludes(
  dockerfile,
  "rustup target add wasm32-unknown-unknown",
  "Dockerfile must install the wasm32 target before building prediction WASM assets",
);
assertMatches(
  dockerfile,
  /cargo install wasm-bindgen-cli --version "\$\{WASM_BINDGEN_CLI_VERSION\}" --locked/,
  "Dockerfile must install the pinned wasm-bindgen CLI with --locked",
);
assertIncludes(
  dockerfile,
  "COPY scripts/build-sim-wasm.sh ./scripts/build-sim-wasm.sh",
  "Dockerfile must copy the WASM build script into the build context",
);
assertIncludes(
  dockerfile,
  "RUN ./scripts/build-sim-wasm.sh",
  "Dockerfile must generate browser-loadable prediction WASM assets during the image build",
);
for (const asset of [
  "./client/vendor/sim-wasm/rts_sim_wasm.js",
  "./client/vendor/sim-wasm/rts_sim_wasm_bg.wasm",
]) {
  assertIncludes(
    dockerfile,
    `test -s ${asset}`,
    `Dockerfile must fail the image build when ${asset} is missing or empty`,
  );
}
assertMatches(
  wasmBuildScript,
  /cargo build [^\n]*-p rts-sim-wasm[^\n]*--locked/,
  "scripts/build-sim-wasm.sh must use Cargo.lock when generating deploy assets",
);
assertIncludes(
  mainTestWorkflow,
  "name: rts-sim-wasm-assets",
  "main test workflow must publish prediction WASM assets for split browser jobs",
);
assertIncludes(
  mainTestWorkflow,
  "run: scripts/build-sim-wasm.sh",
  "main test workflow must generate prediction WASM assets before browser suites",
);
assertIncludes(
  mainTestWorkflow,
  "path: client/vendor/sim-wasm",
  "browser job must download prediction WASM assets into the served client tree",
);

assertIncludes(deployScript, 'config_file="fly.mainline.toml"', "mainline deploys must select the mainline config");
assertIncludes(deployScript, 'config_file="fly.beta.toml"', "beta deploys must select the beta config");
assertIncludes(
  betaDeployWorkflow,
  "./deploy.sh beta",
  "beta workflow must select the beta deployment channel",
);
assertNotIncludes(
  betaDeployWorkflow,
  "RTS_SCENARIO_PR_",
  "beta workflow must not restore scenario-submission configuration or credentials",
);
assertNotMatches(
  betaDeployWorkflow,
  /\bfly(?:ctl)?\s+secrets\s+(?:import|set)\b/i,
  "beta workflow must not mutate application secrets during deploy",
);
assertNotMatches(
  runtimePackages,
  /\b(?:git|gh)\b/,
  "runtime image must not install Git/GitHub tooling for removed scenario submission",
);
assertMatches(mainlineFlyConfig, /^app\s*=\s*"rts-0-zvorygin"/m, "mainline must target the canonical-domain app");
assertMatches(betaFlyConfig, /^app\s*=\s*"rts-0-zvorygin-beta"/m, "beta must target the beta app");
assertMatches(mainlineFlyConfig, /auto_stop_machines\s*=\s*"stop"/, "mainline must stop rather than suspend when idle");
assertMatches(mainlineFlyConfig, /min_machines_running\s*=\s*0/, "mainline must allow zero running Machines");
assertMatches(mainlineFlyConfig, /cpu_kind\s*=\s*"performance"/, "mainline must use performance CPU kind");
assertMatches(mainlineFlyConfig, /cpus\s*=\s*1/, "mainline must use one performance CPU");
assertMatches(mainlineFlyConfig, /memory\s*=\s*"2gb"/, "mainline must use 2 GB memory");
assertMatches(betaFlyConfig, /auto_stop_machines\s*=\s*"stop"/, "beta must stop rather than suspend when idle");
assertMatches(betaFlyConfig, /min_machines_running\s*=\s*0/, "beta must allow zero running Machines");
assertMatches(betaFlyConfig, /cpu_kind\s*=\s*"performance"/, "beta must use performance CPU kind");
assertMatches(betaFlyConfig, /cpus\s*=\s*1/, "beta must use one performance CPU");
assertMatches(betaFlyConfig, /memory\s*=\s*"2gb"/, "beta must use 2 GB memory");
assertIncludes(
  clientIndex,
  'href="https://rts-0-zvorygin-beta.fly.dev/"',
  "the game client beta shortcut must target the beta app",
);
assertIncludes(
  serverMain,
  '[(header::LOCATION, "https://rts-0-zvorygin-beta.fly.dev/")]',
  "the game server beta shortcut must target the beta app",
);

console.log("deploy asset contract: ok");
