#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const SHELL_DIR = path.dirname(SCRIPT_PATH);
const REPO_ROOT = path.resolve(SHELL_DIR, "../..");
const TAURI_DIR = path.join(SHELL_DIR, "src-tauri");
const TAURI_CONFIG_PATH = path.join(TAURI_DIR, "tauri.conf.json");
const CARGO_TOML_PATH = path.join(TAURI_DIR, "Cargo.toml");
const DEFAULT_OUTPUT_ROOT = path.join(TAURI_DIR, "target", "unsigned-playtest");
const BUILD_CONFIG_OVERRIDE = {
  bundle: {
    active: true,
    targets: "app",
  },
};

const RELEASE_PROFILES = [
  {
    id: "beta",
    label: "Beta",
    url: "https://rts-0-zvorygin-beta.fly.dev/",
  },
  {
    id: "mainline",
    label: "Mainline",
    url: "https://rts-0-zvorygin.fly.dev/",
  },
];

function usage() {
  return `Usage: ./build-unsigned.mjs [--output DIR]

Builds an unsigned macOS app bundle for the thin Tauri shell, writes manifest
metadata beside it, and creates a zip under the output directory.

Options:
  --output DIR   Output root. Defaults to src-tauri/target/unsigned-playtest.
  --help         Show this help.
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
      const value = argv[index + 1];
      if (!value) throw new Error("--output requires a directory");
      options.outputRoot = path.resolve(process.cwd(), value);
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${arg}\n\n${usage()}`);
  }
  return options;
}

function commandText(command, args) {
  return [command, ...args].join(" ");
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
    throw new Error(`command failed: ${commandText(command, args)}`);
  }
  return result;
}

function capture(command, args, options = {}) {
  return run(command, args, { ...options, capture: true }).stdout.trim();
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function cargoPackage() {
  const metadata = JSON.parse(
    capture("cargo", [
      "metadata",
      "--manifest-path",
      CARGO_TOML_PATH,
      "--format-version",
      "1",
      "--no-deps",
    ]),
  );
  return metadata.packages.find((pkg) => pkg.name === "maccursor-shell") || metadata.packages[0];
}

function gitOutput(args) {
  return capture("git", args, { cwd: REPO_ROOT });
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function listArtifactEntries(root) {
  const entries = [];

  function visit(absPath) {
    const stats = fs.lstatSync(absPath);
    const relPath = path.relative(root, absPath) || ".";
    if (stats.isDirectory()) {
      entries.push({ type: "dir", relPath });
      for (const dirent of fs
        .readdirSync(absPath, { withFileTypes: true })
        .sort((left, right) => left.name.localeCompare(right.name))) {
        visit(path.join(absPath, dirent.name));
      }
      return;
    }
    if (stats.isSymbolicLink()) {
      entries.push({
        type: "symlink",
        relPath,
        target: fs.readlinkSync(absPath),
      });
      return;
    }
    if (stats.isFile()) {
      entries.push({
        type: "file",
        relPath,
        size: stats.size,
        sha256: sha256File(absPath),
      });
    }
  }

  visit(root);
  return entries.filter((entry) => entry.relPath !== ".");
}

function writeContentsListing(artifactDir, listingPath) {
  const rows = listArtifactEntries(artifactDir)
    .filter((entry) => entry.relPath !== path.basename(listingPath))
    .map((entry) => {
      if (entry.type === "dir") return `DIR       -                                                                  ${entry.relPath}/`;
      if (entry.type === "symlink") {
        return `SYMLINK   -                                                                  ${entry.relPath} -> ${entry.target}`;
      }
      return `FILE      ${entry.sha256} ${String(entry.size).padStart(10, " ")} ${entry.relPath}`;
    });
  fs.writeFileSync(
    listingPath,
    [
      "# RTS Mac Cursor Shell unsigned artifact contents",
      "# Format: TYPE SHA256 SIZE_OR_TARGET RELATIVE_PATH",
      "# This listing covers payload files and excludes this contents file.",
      ...rows,
      "",
    ].join("\n"),
  );
}

function forbiddenBundleAssetMatches(appPath) {
  const forbidden = [];
  const rules = [
    {
      label: "rts-server binary or source path",
      matches: (parts) => parts.some((part) => part === "rts-server" || part.startsWith("rts-server-")),
    },
    {
      label: "browser game client asset directory",
      matches: (parts) => parts.includes("client"),
    },
    {
      label: "map asset directory",
      matches: (parts) => parts.includes("maps"),
    },
    {
      label: "lab scenario asset directory",
      matches: (parts) => parts.includes("lab-scenarios") || parts.includes("lab_scenarios"),
    },
    {
      label: "match-history runtime data",
      matches: (parts) => parts.some((part) => part.includes("match-history")),
    },
  ];

  for (const entry of listArtifactEntries(appPath)) {
    const parts = entry.relPath.split(path.sep).filter(Boolean);
    for (const rule of rules) {
      if (rule.matches(parts)) {
        forbidden.push({ rule: rule.label, path: entry.relPath });
      }
    }
  }
  return forbidden;
}

function validateThinShellConfig(tauriConfig) {
  const bundle = tauriConfig.bundle || {};
  if (bundle.externalBin) {
    throw new Error("tauri.conf.json must not include externalBin for the thin shell artifact");
  }
  if (Array.isArray(bundle.resources) && bundle.resources.length > 0) {
    throw new Error("tauri.conf.json must not bundle extra resources for the thin shell artifact");
  }
  if (bundle.macOS?.signingIdentity) {
    throw new Error("tauri.conf.json must not configure a signing identity for this unsigned artifact");
  }
}

function writeArtifactReadme(filePath, manifest) {
  fs.writeFileSync(
    filePath,
    `# RTS Mac Cursor Shell unsigned macOS artifact

Artifact: \`${manifest.artifact.name}\`
Built: ${manifest.createdAt}
Git SHA: \`${manifest.git.sha}\`
Architecture: \`${manifest.target.arch}\`
Shell version: \`${manifest.shell.version}\`

## Open the unsigned app

This artifact is built with Tauri's \`--no-sign\` flag. It is not Developer ID
signed, notarized, or stapled.

1. Unzip \`${manifest.artifact.zipName}\` if you received the zip.
2. Move \`${manifest.artifact.appBundleName}\` anywhere writable, such as Downloads or Applications.
3. Control-click the app and choose **Open**. If macOS blocks it, open **System Settings > Privacy & Security** and choose **Open Anyway** for this app.

## Choose a server

The startup screen offers only the built-in release channels:

- Beta: \`${RELEASE_PROFILES[0].url}\`
- Mainline: \`${RELEASE_PROFILES[1].url}\`

The shell does not include or start \`rts-server\`, the browser client, maps, lab scenarios, match-history databases, or other game runtime assets. After a channel is selected, game content loads from the selected website.

## Find logs

Shell logs are local JSONL files at:

\`\`\`text
~/Library/Logs/dev.bewegungskrieg.MacCursorShell/shell.log
\`\`\`

The startup screen also has **Copy log path** and **Reveal logs** actions.

## Included files

- \`${manifest.artifact.appBundleName}\` - unsigned macOS app bundle.
- \`manifest.json\` - build metadata, git SHA, architecture, shell version, and thin-shell checks.
- \`contents.txt\` - file listing with SHA-256 hashes and byte sizes for the artifact payload.
`,
  );
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (process.platform !== "darwin") {
    throw new Error("unsigned maccursor-shell artifacts must be built on macOS");
  }

  const tauriConfig = readJson(TAURI_CONFIG_PATH);
  validateThinShellConfig(tauriConfig);
  const pkg = cargoPackage();
  if (!pkg) throw new Error("failed to resolve Cargo package metadata");
  if (tauriConfig.version !== pkg.version) {
    throw new Error(
      `Tauri config version ${tauriConfig.version} does not match Cargo version ${pkg.version}`,
    );
  }

  run("cargo", ["--version"], { capture: true });
  const cargoTauriVersion = capture("cargo", ["tauri", "--version"]);
  const rustcVersion = capture("rustc", ["--version"]);
  const xcodeSelectPath = capture("/usr/bin/xcode-select", ["-p"]);
  if (!fs.existsSync("/usr/bin/ditto")) {
    throw new Error("macOS ditto is required to create the artifact zip");
  }

  const gitSha = gitOutput(["rev-parse", "HEAD"]);
  const shortSha = gitSha.slice(0, 12);
  const dirty = gitOutput(["status", "--short", "--untracked-files=normal"]).length > 0;
  const arch = os.arch();
  const artifactName = `maccursor-shell-v${pkg.version}-${shortSha}-${arch}`;
  const outputRoot = path.resolve(options.outputRoot);
  const artifactDir = path.join(outputRoot, artifactName);
  const appBundleName = `${tauriConfig.productName}.app`;
  const builtAppPath = path.join(TAURI_DIR, "target", "release", "bundle", "macos", appBundleName);
  const appDestPath = path.join(artifactDir, appBundleName);
  const manifestPath = path.join(artifactDir, "manifest.json");
  const readmePath = path.join(artifactDir, "README.md");
  const contentsPath = path.join(artifactDir, "contents.txt");
  const zipName = `${artifactName}.zip`;
  const zipPath = path.join(outputRoot, zipName);
  const checksumPath = path.join(outputRoot, `${zipName}.sha256`);
  const buildStartedAt = new Date().toISOString();

  fs.mkdirSync(outputRoot, { recursive: true });

  run(
    "cargo",
    [
      "tauri",
      "build",
      "--bundles",
      "app",
      "--no-sign",
      "--ci",
      "--config",
      JSON.stringify(BUILD_CONFIG_OVERRIDE),
    ],
    {
      cwd: SHELL_DIR,
      env: {
        ...process.env,
        GITHUB_SHA: process.env.GITHUB_SHA || gitSha,
        RTS_DESKTOP_BUILD_ID: process.env.RTS_DESKTOP_BUILD_ID || gitSha,
      },
    },
  );

  if (!fs.existsSync(builtAppPath)) {
    throw new Error(`expected Tauri app bundle was not found at ${builtAppPath}`);
  }

  fs.rmSync(artifactDir, { recursive: true, force: true });
  fs.mkdirSync(artifactDir, { recursive: true });
  fs.cpSync(builtAppPath, appDestPath, { recursive: true, preserveTimestamps: true });

  const forbiddenMatches = forbiddenBundleAssetMatches(appDestPath);
  const manifest = {
    schemaVersion: 1,
    createdAt: buildStartedAt,
    artifact: {
      kind: "unsigned-macos-playtest",
      name: artifactName,
      directory: path.relative(REPO_ROOT, artifactDir),
      appBundleName,
      zipName,
    },
    shell: {
      packageName: pkg.name,
      productName: tauriConfig.productName,
      identifier: tauriConfig.identifier,
      version: pkg.version,
      minimumSystemVersion: tauriConfig.bundle?.macOS?.minimumSystemVersion || null,
    },
    git: {
      sha: gitSha,
      shortSha,
      dirty,
    },
    target: {
      platform: "macos",
      arch,
    },
    build: {
      command: "./build-unsigned.mjs",
      configOverride: BUILD_CONFIG_OVERRIDE,
      cargoTauriVersion,
      rustcVersion,
      xcodeSelectPath,
      noSign: true,
    },
    releaseProfiles: RELEASE_PROFILES,
    thinShell: {
      forbiddenRuntimeAssetMatches: forbiddenMatches,
      checkedForbiddenAssets: [
        "rts-server",
        "client",
        "maps",
        "lab-scenarios",
        "match-history",
      ],
      bundlesExternalBins: false,
      bundlesExtraResources: false,
    },
  };

  writeArtifactReadme(readmePath, manifest);
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  writeContentsListing(artifactDir, contentsPath);

  if (forbiddenMatches.length > 0) {
    throw new Error(
      `artifact bundle contains forbidden game runtime assets:\n${forbiddenMatches
        .map((match) => `- ${match.rule}: ${match.path}`)
        .join("\n")}`,
    );
  }

  fs.rmSync(zipPath, { force: true });
  run("/usr/bin/ditto", ["-c", "-k", "--keepParent", artifactName, zipPath], { cwd: outputRoot });
  const zipSha256 = sha256File(zipPath);
  fs.writeFileSync(checksumPath, `${zipSha256}  ${zipName}\n`);

  console.log(`unsigned artifact directory: ${artifactDir}`);
  console.log(`unsigned artifact zip: ${zipPath}`);
  console.log(`zip sha256: ${zipSha256}`);
  console.log(`manifest: ${manifestPath}`);
}

try {
  main();
} catch (err) {
  console.error(err?.message || String(err));
  process.exit(1);
}
