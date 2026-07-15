#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { writeCpuFlameGraphArtifacts } from "./client-cpu-profile-to-flamegraph.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(HERE, "..");
const DEFAULT_OUTPUT_ROOT = path.join(REPO_ROOT, "target", "client-perf", "flamegraphs");
const DEFAULT_CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

export function parseClientFlameGraphArgs(argv) {
  const options = {
    workload: "supply-300-hellhole-stream",
    seconds: 15,
    intervalUs: 500,
    outputRoot: DEFAULT_OUTPUT_ROOT,
    chrome: process.env.CHROME || "",
    preview: false,
    previewTtl: "24h",
    cpuThrottle: 1,
    viewport: "1440x900",
    dpr: 1,
    baseUrl: "",
    port: 0,
    help: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = () => {
      index += 1;
      if (index >= argv.length) throw new Error(`${arg} requires a value`);
      return argv[index];
    };
    if (arg === "--workload") options.workload = value();
    else if (arg.startsWith("--workload=")) options.workload = arg.slice("--workload=".length);
    else if (arg === "--seconds") options.seconds = positiveInteger(value(), arg);
    else if (arg.startsWith("--seconds=")) {
      options.seconds = positiveInteger(arg.slice("--seconds=".length), "--seconds");
    }
    else if (arg === "--interval-us") options.intervalUs = positiveInteger(value(), arg);
    else if (arg.startsWith("--interval-us=")) {
      options.intervalUs = positiveInteger(arg.slice("--interval-us=".length), "--interval-us");
    }
    else if (arg === "--output-root") options.outputRoot = path.resolve(value());
    else if (arg.startsWith("--output-root=")) options.outputRoot = path.resolve(arg.slice("--output-root=".length));
    else if (arg === "--chrome") options.chrome = value();
    else if (arg.startsWith("--chrome=")) options.chrome = arg.slice("--chrome=".length);
    else if (arg === "--preview") options.preview = true;
    else if (arg === "--preview-ttl") options.previewTtl = value();
    else if (arg.startsWith("--preview-ttl=")) options.previewTtl = arg.slice("--preview-ttl=".length);
    else if (arg === "--cpu-throttle") options.cpuThrottle = positiveNumber(value(), arg);
    else if (arg.startsWith("--cpu-throttle=")) {
      options.cpuThrottle = positiveNumber(arg.slice("--cpu-throttle=".length), "--cpu-throttle");
    }
    else if (arg === "--viewport") options.viewport = value();
    else if (arg.startsWith("--viewport=")) options.viewport = arg.slice("--viewport=".length);
    else if (arg === "--dpr") options.dpr = positiveNumber(value(), arg);
    else if (arg.startsWith("--dpr=")) options.dpr = positiveNumber(arg.slice("--dpr=".length), "--dpr");
    else if (arg === "--base-url") options.baseUrl = value();
    else if (arg.startsWith("--base-url=")) options.baseUrl = arg.slice("--base-url=".length);
    else if (arg === "--port") options.port = positiveInteger(value(), arg);
    else if (arg.startsWith("--port=")) options.port = positiveInteger(arg.slice("--port=".length), "--port");
    else if (arg === "--help" || arg === "-h") options.help = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  if (!/^[1-9]\d*x[1-9]\d*$/.test(options.viewport)) throw new Error("--viewport must look like 1440x900");
  if (options.intervalUs < 100 || options.intervalUs > 100_000) {
    throw new Error("--interval-us must be between 100 and 100000");
  }
  return options;
}

export function printClientFlameGraphHelp() {
  console.log(`Usage: node scripts/client-flamegraph.mjs [options]

Captures a steady-state V8 CPU profile through the client performance harness, writes a ranked
JSON summary plus SVG/PNG flame graphs, and optionally publishes the PNG to Tailnet Preview.

Options:
  --workload <id>       Workload to profile. Default: supply-300-hellhole-stream.
  --seconds <n>         Steady-state sample duration. Default: 15.
  --interval-us <n>     V8 sampling interval, 100-100000 microseconds. Default: 500.
  --cpu-throttle <n>    Chrome CPU throttle factor. Default: 1.
  --viewport <WxH>      Browser viewport. Default: 1440x900.
  --dpr <n>             Browser device scale factor. Default: 1.
  --base-url <url>      Reuse a healthy local server.
  --port <n>            Port for a harness-started server.
  --chrome <path>       Chrome/Chromium executable. Defaults to CHROME or common paths.
  --output-root <path>  Artifact root. Default: target/client-perf/flamegraphs.
  --preview             Publish the PNG through scripts/tailnet-preview.
  --preview-ttl <ttl>   Tailnet Preview lifetime. Default: 24h.
`);
}

export async function runClientFlameGraph(options) {
  const runRoot = path.join(
    options.outputRoot,
    new Date().toISOString().replace(/[:.]/g, "-"),
  );
  fs.mkdirSync(runRoot, { recursive: true });
  const harnessArgs = [
    path.join(HERE, "client-perf-harness.mjs"),
    "--workload", options.workload,
    "--seconds", String(options.seconds),
    "--cpu-throttle", String(options.cpuThrottle),
    "--viewport", options.viewport,
    "--dpr", String(options.dpr),
    "--cpu-profile-interval-us", String(options.intervalUs),
    "--output-root", runRoot,
  ];
  if (options.baseUrl) harnessArgs.push("--base-url", options.baseUrl);
  if (options.port) harnessArgs.push("--port", String(options.port));
  if (options.chrome) harnessArgs.push("--chrome", options.chrome);
  runChecked(process.execPath, harnessArgs, {
    cwd: REPO_ROOT,
    stdio: "inherit",
    env: process.env,
  });

  const artifactDir = latestArtifactDirectory(path.join(runRoot, options.workload));
  const harnessSummaryPath = path.join(artifactDir, "summary.json");
  const harnessSummary = JSON.parse(fs.readFileSync(harnessSummaryPath, "utf8"));
  if (harnessSummary.status !== "passed") {
    throw new Error(`client performance harness did not pass; see ${harnessSummaryPath}`);
  }
  const profilePath = path.join(artifactDir, "cpu-profile.cpuprofile");
  if (!fs.existsSync(profilePath)) {
    throw new Error(`client performance harness did not write a CPU profile; see ${harnessSummaryPath}`);
  }

  const svgPath = path.join(artifactDir, "client-cpu-flamegraph.svg");
  const pngPath = path.join(artifactDir, "client-cpu-flamegraph.png");
  const title = `RTS ${options.workload} — steady-state CPU flame graph `
    + `(${options.seconds} s, ${options.intervalUs} µs samples)`;
  const result = writeCpuFlameGraphArtifacts({ profilePath, svgPath, title });
  await renderSvgToPng({ svg: result.svg, svgPath, pngPath, chrome: options.chrome });

  console.log(`CPU profile: ${profilePath}`);
  console.log(`Flame graph SVG: ${svgPath}`);
  console.log(`Flame graph PNG: ${pngPath}`);
  console.log(`Ranked summary: ${result.summaryPath}`);
  for (const row of result.analysis.summary.topSelfByFunction.slice(0, 15)) {
    console.log(`${row.selfPct.toFixed(1).padStart(5)}% self  ${row.label}`);
  }
  if (options.preview) {
    runChecked(path.join(HERE, "tailnet-preview"), ["--ttl", options.previewTtl, pngPath], {
      cwd: REPO_ROOT,
      stdio: "inherit",
    });
  }
  return {
    artifactDir,
    harnessSummaryPath,
    profilePath,
    svgPath,
    pngPath,
    rankedSummaryPath: result.summaryPath,
    summary: result.analysis.summary,
  };
}

async function renderSvgToPng({ svg, svgPath, pngPath, chrome }) {
  const match = /<svg[^>]+width="(\d+)"[^>]+height="(\d+)"/.exec(svg);
  if (!match) throw new Error(`could not read flame graph dimensions from ${svgPath}`);
  const puppeteer = await import("puppeteer-core");
  const browser = await (puppeteer.default || puppeteer).launch({
    executablePath: findChrome(chrome),
    headless: "new",
    args: ["--no-sandbox", "--hide-scrollbars"],
  });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: Number(match[1]), height: Number(match[2]), deviceScaleFactor: 1 });
    await page.goto(pathToFileURL(svgPath).href, { waitUntil: "load" });
    await page.screenshot({ path: pngPath, type: "png" });
  } finally {
    await browser.close();
  }
}

function latestArtifactDirectory(workloadRoot) {
  const directories = fs.readdirSync(workloadRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
  if (directories.length === 0) throw new Error(`no workload artifact directory under ${workloadRoot}`);
  return path.join(workloadRoot, directories.at(-1));
}

function findChrome(explicit) {
  const candidates = [
    explicit,
    DEFAULT_CHROME,
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    which("google-chrome-stable"),
    which("google-chrome"),
    which("chromium-browser"),
    which("chromium"),
  ].filter(Boolean);
  const found = candidates.find((candidate) => fs.existsSync(candidate));
  if (!found) throw new Error("Chrome/Chromium not found; set CHROME=/path/to/chrome or pass --chrome");
  return found;
}

function which(command) {
  const result = spawnSync("which", [command], { encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : "";
}

function runChecked(command, args, options) {
  const result = spawnSync(command, args, options);
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed with exit ${result.status}`);
}

function positiveInteger(raw, label) {
  const value = Number(raw);
  if (!Number.isInteger(value) || value <= 0) throw new Error(`${label} must be a positive integer`);
  return value;
}

function positiveNumber(raw, label) {
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) throw new Error(`${label} must be a positive number`);
  return value;
}

async function main() {
  const options = parseClientFlameGraphArgs(process.argv.slice(2));
  if (options.help) {
    printClientFlameGraphHelp();
    return;
  }
  await runClientFlameGraph(options);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error.stack || error.message);
    process.exit(1);
  });
}
