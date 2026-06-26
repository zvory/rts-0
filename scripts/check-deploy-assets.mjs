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
const cargoLock = read("server/Cargo.lock");
const mainTestWorkflow = read(".github/workflows/main-tests.yml");
const wasmBuildScript = read("scripts/build-sim-wasm.sh");
const wasmGitignore = read("client/vendor/sim-wasm/.gitignore");
const wasmBindgenLockVersion = cargoLock.match(
  /\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"/,
)?.[1];
const wasmBindgenDockerVersion = dockerfile.match(/ARG WASM_BINDGEN_CLI_VERSION=([^\s]+)/)?.[1];

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

console.log("deploy asset contract: ok");
