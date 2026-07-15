#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const SHELL_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SHELL_DIR, "../..");
const TAURI_DIR = path.join(SHELL_DIR, "src-tauri");
const CONFIG_PATH = path.join(TAURI_DIR, "tauri.conf.json");
const CARGO_TOML_PATH = path.join(TAURI_DIR, "Cargo.toml");
const DEFAULT_OUTPUT_ROOT = path.join(TAURI_DIR, "target", "unsigned-playtest-windows");
const BUILD_CONFIG_OVERRIDE = { bundle: { active: true, targets: "nsis" } };
const RELEASE_PROFILES = [
  { id: "beta", label: "Beta", url: "https://rts-0-zvorygin-beta.fly.dev/" },
  { id: "mainline", label: "Mainline", url: "https://rts-0-zvorygin.fly.dev/" },
];

function usage() {
  return `Usage: node build-unsigned-windows.mjs [--output DIR]

Builds an unsigned x64 Windows NSIS installer for the thin Tauri shell and
writes release metadata beside it.
`;
}

function parseArgs(argv) {
  const options = { outputRoot: DEFAULT_OUTPUT_ROOT };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    }
    if (arg === "--output") {
      if (!argv[index + 1]) throw new Error("--output requires a directory");
      options.outputRoot = path.resolve(process.cwd(), argv[index + 1]);
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${arg}\n\n${usage()}`);
  }
  return options;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || SHELL_DIR,
    env: options.env || process.env,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
  if (result.status !== 0) {
    if (options.capture) {
      if (result.stdout) process.stdout.write(result.stdout);
      if (result.stderr) process.stderr.write(result.stderr);
    }
    throw new Error(`command failed: ${command} ${args.join(" ")}`);
  }
  return result;
}

function capture(command, args, options = {}) {
  return run(command, args, { ...options, capture: true }).stdout.trim();
}

function gitTreeIsDirty() {
  const status = capture(
    "git",
    ["-c", "core.filemode=false", "status", "--short", "--untracked-files=normal"],
    { cwd: REPO_ROOT },
  );
  if (!status) return false;

  for (const line of status.split(/\r?\n/)) {
    // Native Windows Git cannot represent a symlink in a WSL UNC worktree and
    // reports the clean entry as modified/type-changed. Ignore only tracked
    // symlink entries here; release verification also runs WSL-native git
    // status, which remains authoritative for the link target itself.
    if (line.startsWith(" T ") || line.startsWith(" M ")) {
      const relativePath = line.slice(3);
      const indexEntry = capture("git", ["ls-files", "-s", "--", relativePath], {
        cwd: REPO_ROOT,
      });
      if (indexEntry.startsWith("120000 ")) {
        continue;
      }
    }
    return true;
  }
  return false;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function listFiles(root) {
  const files = [];
  const visit = (current) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) visit(absolute);
      else if (entry.isFile()) files.push(absolute);
    }
  };
  visit(root);
  return files.sort();
}

function validateThinShellConfig(config) {
  const bundle = config.bundle || {};
  if (bundle.externalBin) throw new Error("thin shell must not configure bundle.externalBin");
  if (Array.isArray(bundle.resources) && bundle.resources.length > 0) {
    throw new Error("thin shell must not configure extra bundle resources");
  }
}

function forbiddenMatches(root) {
  const forbidden = /(^|[\\/])(rts-server(?:\.exe)?|client|maps|lab-scenarios|lab_scenarios|match-history)([\\/]|$)/i;
  return listFiles(root)
    .map((filePath) => path.relative(root, filePath))
    .filter((relativePath) => forbidden.test(relativePath));
}

function writeReadme(filePath, manifest) {
  fs.writeFileSync(
    filePath,
    `# Bewegungskrieg unsigned Windows playtest installer

Artifact: \`${manifest.artifact.installerName}\`
Built: ${manifest.createdAt}
Git SHA: \`${manifest.git.sha}\`
Architecture: \`${manifest.target.arch}\`
Shell version: \`${manifest.shell.version}\`
SHA-256: \`${manifest.artifact.sha256}\`

This first-playtest installer is **unsigned**. Windows SmartScreen may show
**Windows protected your PC**. Verify the SHA-256 value first, then choose
**More info > Run anyway** only if the checksum matches the GitHub Release.

## Install and run

1. Run \`${manifest.artifact.installerName}\`.
2. The default current-user install should not require administrator access.
3. Open **Bewegungskrieg** from Start.
4. Choose Beta for playtesting or Mainline for the public release channel.

The installer contains only the Tauri shell. It does not contain a game server,
browser client, maps, Lab scenarios, match history, or replay data. Game content
loads from the selected release website.

## Logs

Use **Copy log path** or **Reveal logs** on the startup screen. The default path is:

\`%LOCALAPPDATA%\\${manifest.shell.identifier}\\logs\\shell.log\`

## Uninstall

Open **Settings > Apps > Installed apps**, find **Bewegungskrieg**, choose the
menu beside it, and select **Uninstall**.
`,
  );
}

function writeContents(artifactDir) {
  const contentsPath = path.join(artifactDir, "contents.txt");
  const rows = listFiles(artifactDir)
    .filter((filePath) => filePath !== contentsPath)
    .map((filePath) => {
      const stat = fs.statSync(filePath);
      return `FILE ${sha256File(filePath)} ${String(stat.size).padStart(10)} ${path.relative(artifactDir, filePath)}`;
    });
  fs.writeFileSync(contentsPath, ["# TYPE SHA256 SIZE RELATIVE_PATH", ...rows, ""].join("\n"));
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (process.platform !== "win32") throw new Error("Windows artifacts must be built on Windows");

  const config = readJson(CONFIG_PATH);
  validateThinShellConfig(config);
  const metadata = JSON.parse(
    capture("cargo", ["metadata", "--manifest-path", CARGO_TOML_PATH, "--format-version", "1", "--no-deps"]),
  );
  const pkg = metadata.packages.find((candidate) => candidate.name === "maccursor-shell");
  if (!pkg) throw new Error("failed to resolve maccursor-shell Cargo metadata");
  if (pkg.version !== config.version) throw new Error("Cargo and Tauri versions do not match");

  const gitSha = capture("git", ["rev-parse", "HEAD"], { cwd: REPO_ROOT });
  const shortSha = gitSha.slice(0, 12);
  const dirty = gitTreeIsDirty();
  const cargoTauriVersion = capture("cargo", ["tauri", "--version"]);
  const rustcVersion = capture("rustc", ["--version"]);
  const targetDirectory = path.resolve(metadata.target_directory);
  const nsisDirectory = path.join(targetDirectory, "release", "bundle", "nsis");
  const outputRoot = path.resolve(options.outputRoot);
  const artifactName = `bewegungskrieg-v${pkg.version}-${shortSha}-x64`;
  const artifactDir = path.join(outputRoot, artifactName);
  const installerName = `${artifactName}-setup.exe`;
  const installerPath = path.join(artifactDir, installerName);
  const buildStartedAt = new Date().toISOString();

  run(
    "cargo",
    [
      "tauri",
      "build",
      "--bundles",
      "nsis",
      "--ci",
      "--config",
      JSON.stringify(BUILD_CONFIG_OVERRIDE),
    ],
    {
      env: {
        ...process.env,
        GITHUB_SHA: process.env.GITHUB_SHA || gitSha,
        RTS_DESKTOP_BUILD_ID: process.env.RTS_DESKTOP_BUILD_ID || gitSha,
      },
    },
  );

  const builtInstallers = fs.existsSync(nsisDirectory)
    ? fs.readdirSync(nsisDirectory).filter((name) => name.toLowerCase().endsWith(".exe"))
    : [];
  if (builtInstallers.length !== 1) {
    throw new Error(`expected one NSIS installer in ${nsisDirectory}, found ${builtInstallers.length}`);
  }

  fs.rmSync(artifactDir, { recursive: true, force: true });
  fs.mkdirSync(artifactDir, { recursive: true });
  fs.copyFileSync(path.join(nsisDirectory, builtInstallers[0]), installerPath);
  const installerSha256 = sha256File(installerPath);
  const matches = forbiddenMatches(artifactDir);
  const manifest = {
    schemaVersion: 1,
    createdAt: buildStartedAt,
    artifact: {
      kind: "unsigned-windows-nsis-playtest",
      name: artifactName,
      installerName,
      sha256: installerSha256,
    },
    shell: {
      packageName: pkg.name,
      productName: config.productName,
      identifier: config.identifier,
      version: pkg.version,
    },
    git: { sha: gitSha, shortSha, dirty },
    target: { platform: "windows", arch: os.arch(), installer: "nsis" },
    build: {
      command: "node build-unsigned-windows.mjs",
      configOverride: BUILD_CONFIG_OVERRIDE,
      cargoTauriVersion,
      rustcVersion,
      targetDirectory,
      unsigned: true,
    },
    releaseProfiles: RELEASE_PROFILES,
    thinShell: {
      forbiddenRuntimeAssetMatches: matches,
      checkedForbiddenAssets: ["rts-server.exe", "client", "maps", "lab-scenarios", "match-history"],
      bundlesExternalBins: false,
      bundlesExtraResources: false,
    },
  };

  writeReadme(path.join(artifactDir, "README.md"), manifest);
  fs.writeFileSync(path.join(artifactDir, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  fs.writeFileSync(path.join(artifactDir, `${installerName}.sha256`), `${installerSha256}  ${installerName}\n`);
  writeContents(artifactDir);
  if (matches.length > 0) throw new Error(`artifact contains forbidden runtime assets: ${matches.join(", ")}`);

  console.log(`unsigned Windows artifact: ${artifactDir}`);
  console.log(`NSIS installer: ${installerPath}`);
  console.log(`installer sha256: ${installerSha256}`);
  console.log(`manifest: ${path.join(artifactDir, "manifest.json")}`);
}

try {
  main();
} catch (error) {
  console.error(error?.message || String(error));
  process.exit(1);
}
