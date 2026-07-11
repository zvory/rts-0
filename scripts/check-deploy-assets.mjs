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

function assertMatches(text, pattern, message) {
  if (!pattern.test(text)) {
    throw new Error(message);
  }
}

const dockerfile = read("Dockerfile");
const dockerignore = read(".dockerignore");
const cargoLock = read("server/Cargo.lock");
const mainTestWorkflow = read(".github/workflows/main-tests.yml");
const wasmBuildScript = read("scripts/build-sim-wasm.sh");
const wasmGitignore = read("client/vendor/sim-wasm/.gitignore");
const deployScript = read("deploy.sh");
const mainlineFlyConfig = read("fly.mainline.toml");
const betaFlyConfig = read("fly.beta.toml");
const launcherFlyConfig = read("fly.launcher.toml");
const launcherServer = read("launcher/server.mjs");
const serverMain = read("server/src/main.rs");
const wasmBindgenLockVersion = cargoLock.match(
  /\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"/,
)?.[1];
const wasmBindgenDockerVersion = dockerfile.match(/ARG WASM_BINDGEN_CLI_VERSION=([^\s]+)/)?.[1];
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
assertIncludes(deployScript, 'config_file="fly.launcher.toml"', "launcher deploys must select the launcher config");
assertMatches(mainlineFlyConfig, /^app\s*=\s*"bewegungskrieg-mainline"/m, "mainline must target its named game app");
assertMatches(betaFlyConfig, /^app\s*=\s*"bewegungskrieg-beta"/m, "beta must target its named game app");
assertMatches(launcherFlyConfig, /^app\s*=\s*"rts-0-zvorygin"/m, "launcher must target the canonical-domain app");
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
assertMatches(launcherFlyConfig, /auto_stop_machines\s*=\s*"off"/, "launcher must remain always-on");
assertMatches(launcherFlyConfig, /min_machines_running\s*=\s*1/, "launcher must remain available while game apps stop");
assertIncludes(
  launcherServer,
  'mainline: "https://bewegungskrieg-mainline.fly.dev"',
  "launcher mainline origin must match the named mainline app",
);
assertIncludes(
  launcherServer,
  'beta: "https://bewegungskrieg-beta.fly.dev"',
  "launcher beta origin must match the named beta app",
);
assertIncludes(
  serverMain,
  '[(header::LOCATION, "https://bewegungskrieg-beta.fly.dev/")]',
  "the game server beta shortcut must target the named beta app",
);

console.log("deploy asset contract: ok");
