import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MAX_RUNTIME_RIG_TEXTURE_DIMENSION = 2048;

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function filesUnder(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(entryPath) : [entryPath];
  });
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

function readPngHeader(filePath) {
  const buffer = fs.readFileSync(filePath);
  const hasPngSignature = buffer.subarray(0, 8).toString("hex") === "89504e470d0a1a0a";
  const hasIhdr = buffer.length >= 29 &&
    buffer.readUInt32BE(8) === 13 &&
    buffer.subarray(12, 16).toString("ascii") === "IHDR";
  if (!hasPngSignature || !hasIhdr) {
    throw new Error(`${path.relative(repoRoot, filePath)} must be a valid PNG runtime rig texture`);
  }
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
    bitDepth: buffer[24],
  };
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
const dockerignoreEntries = new Set(
  dockerignore
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#")),
);

const dockerBuildRoots = new Set([
  ".dockerignore",
  "Dockerfile",
  "client",
  "docs",
  "scripts",
  "server",
]);
for (const entry of fs.readdirSync(repoRoot, { withFileTypes: true })) {
  if (
    entry.isDirectory() &&
    !dockerBuildRoots.has(entry.name) &&
    !dockerignoreEntries.has(entry.name)
  ) {
    throw new Error(`.dockerignore must exclude top-level non-build directory ${entry.name}`);
  }
}

for (const excludedPath of [
  ".git",
  ".docdrift",
  "desktop",
  "node_modules",
  "plans",
  "target",
  "tests",
  "server/target",
  "docs/*",
  "scripts/*",
  "client/vendor/sim-wasm/rts_sim_wasm.js",
  "client/vendor/sim-wasm/rts_sim_wasm_bg.wasm",
  "client/assets/rigs/**/*.*",
]) {
  if (!dockerignoreEntries.has(excludedPath)) {
    throw new Error(`.dockerignore must exclude deploy-irrelevant path ${excludedPath}`);
  }
}
for (const buildInput of ["!docs/context", "!docs/design", "!scripts/build-sim-wasm.sh"]) {
  if (!dockerignoreEntries.has(buildInput)) {
    throw new Error(`.dockerignore must retain Docker build input ${buildInput}`);
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
const generatedWasmAssets = [
  "./client/vendor/sim-wasm/rts_sim_wasm.js",
  "./client/vendor/sim-wasm/rts_sim_wasm_bg.wasm",
];
for (const asset of generatedWasmAssets) {
  assertIncludes(
    dockerfile,
    `test -s ${asset}`,
    `Dockerfile must fail the image build when ${asset} is missing or empty`,
  );
}

const clientRuntimeSourceFiles = [
  path.join(repoRoot, "client/index.html"),
  path.join(repoRoot, "client/manifest.webmanifest"),
  path.join(repoRoot, "client/styles.css"),
  ...filesUnder(path.join(repoRoot, "client/src")),
];
const runtimeRigAssets = new Set();
for (const sourceFile of clientRuntimeSourceFiles) {
  const source = fs.readFileSync(sourceFile, "utf8");
  for (const match of source.matchAll(/["'`(](\/assets\/rigs\/[^"'`)\s?#]+)/g)) {
    runtimeRigAssets.add(`./client${match[1]}`);
  }
}
const checkedInRuntimeAssets = [
  "./client/assets/snapshot-streams/supply-300-hellhole.rtsstream",
  ...Array.from(runtimeRigAssets).sort(),
];
for (const asset of checkedInRuntimeAssets) {
  const localAsset = path.join(repoRoot, asset);
  const assetStat = fs.statSync(localAsset);
  if (!assetStat.isFile() || assetStat.size === 0) {
    throw new Error(`${asset} must be a non-empty checked-in runtime asset`);
  }
  assertIncludes(
    dockerfile,
    `test -s ${asset}`,
    `Dockerfile must fail the image build when ${asset} is absent from the filtered context`,
  );
  if (asset.includes("/assets/rigs/")) {
    const png = readPngHeader(localAsset);
    if (png.width > MAX_RUNTIME_RIG_TEXTURE_DIMENSION || png.height > MAX_RUNTIME_RIG_TEXTURE_DIMENSION) {
      throw new Error(
        `${asset} is ${png.width}x${png.height}; runtime rig textures must fit within ` +
          `${MAX_RUNTIME_RIG_TEXTURE_DIMENSION}x${MAX_RUNTIME_RIG_TEXTURE_DIMENSION}`,
      );
    }
    if (png.bitDepth !== 8) {
      throw new Error(
        `${asset} uses ${png.bitDepth}-bit PNG channels; runtime rig textures must use 8-bit channels`,
      );
    }
    const allowlistEntry = `!${asset.slice(2)}`;
    if (!dockerignoreEntries.has(allowlistEntry)) {
      throw new Error(`.dockerignore must retain runtime rig texture ${allowlistEntry}`);
    }
  }
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
