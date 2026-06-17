#!/usr/bin/env node
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";
import { SVG_MIGRATION_MANIFESTS } from "./fixtures/svg/unit_migration_manifests.mjs";
import { compareRgbaBuffers, makeDiffRgba } from "./visual_pixel_compare.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");
const baselinePath = path.join(repoRoot, "tests/fixtures/svg/legacy-unit-oracle.baseline.json");
const fixturePagePath = path.join(repoRoot, "tests/fixtures/svg/transparent-unit-pixels.html");
const artifactRoot = path.join(repoRoot, "tests/artifacts/transparent-unit-pixels");
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const fixedNow = 10_000;
const bufferSize = { width: 96, height: 96 };
const expectFailures = process.argv.includes("--expect-failures");
const noArtifacts = process.argv.includes("--no-artifacts");
const partsOnly = process.argv.includes("--parts-only");
const includePartComparisons = partsOnly || process.argv.includes("--parts");
const includeCompositionComparisons = !partsOnly;

const baseline = JSON.parse(fs.readFileSync(baselinePath, "utf8"));
const samples = migrationSamplesFromBaseline(baseline);
const thresholdsByKind = Object.fromEntries(
  SVG_MIGRATION_MANIFESTS.map((manifest) => [manifest.kind, manifest.compositionThresholds]),
);
const partMappingsByKind = Object.fromEntries(
  SVG_MIGRATION_MANIFESTS.map((manifest) => [manifest.kind, manifest.partMappings]),
);
const svgTextByKind = Object.fromEntries(
  SVG_MIGRATION_MANIFESTS.map((manifest) => [manifest.kind, fs.readFileSync(manifest.svgPath, "utf8")]),
);

const staticServer = await startStaticServer(repoRoot);
const chromeProfileDir = fs.mkdtempSync(path.join(os.tmpdir(), "rts-pixel-harness-chrome-"));
const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: "new",
  args: ["--no-sandbox", "--window-size=320,320", `--user-data-dir=${chromeProfileDir}`],
  defaultViewport: { width: 320, height: 320, deviceScaleFactor: 1 },
});

try {
  const page = await browser.newPage();
  const consoleErrors = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => consoleErrors.push(error.message));
  page.on("requestfailed", (request) => consoleErrors.push(`request failed: ${request.url()} ${request.failure()?.errorText || ""}`));

  await page.goto(new URL("/tests/fixtures/svg/transparent-unit-pixels.html", staticServer.url).href, { waitUntil: "networkidle2", timeout: 20_000 });
  await page.waitForFunction(() => !!window.PIXI, { timeout: 10_000 });
  const rendered = await page.evaluate(renderSamplesInBrowser, {
    samples,
    bufferSize,
    fixedNow,
    thresholdsByKind,
    partMappingsByKind: includePartComparisons ? partMappingsByKind : {},
    includeCompositionComparisons,
    svgTextByKind,
  });
  if (consoleErrors.length > 0) {
    throw new Error(`browser errors during transparent pixel harness:\n${consoleErrors.join("\n")}`);
  }

  const compositionReports = [];
  const partReports = [];
  const failures = [];
  for (const renderedSample of rendered.compositionSamples) {
    const report = compareRgbaBuffers(renderedSample.legacy, renderedSample.rig, renderedSample.thresholds);
    const entry = {
      comparison: "composition",
      label: renderedSample.label,
      kind: renderedSample.kind,
      teamColor: renderedSample.teamColor,
      facing: renderedSample.facing,
      state: renderedSample.state,
      busy: renderedSample.busy,
      ...report,
    };
    compositionReports.push(entry);
    if (!report.passed) {
      failures.push({ renderedSample, entry });
    }
  }
  for (const renderedSample of rendered.partSamples) {
    const report = compareRgbaBuffers(renderedSample.legacy, renderedSample.rig, renderedSample.thresholds);
    const entry = {
      comparison: "part",
      label: renderedSample.label,
      kind: renderedSample.kind,
      unit: renderedSample.kind,
      part: renderedSample.legacyPart,
      rigParts: renderedSample.rigParts,
      missingRigParts: renderedSample.missingRigParts,
      teamColor: renderedSample.teamColor,
      facing: renderedSample.facing,
      state: renderedSample.state,
      busy: renderedSample.busy,
      ...report,
    };
    partReports.push(entry);
    if (!report.passed) {
      failures.push({ renderedSample, entry });
    }
  }

  if (failures.length > 0 && !noArtifacts) {
    fs.rmSync(artifactRoot, { recursive: true, force: true });
    for (const failure of failures) writeFailureArtifacts(failure);
  }

  const summary = {
    compositionSamples: rendered.compositionSamples.length,
    partSamples: rendered.partSamples.length,
    comparisons: rendered.compositionSamples.length + rendered.partSamples.length,
    passed: rendered.compositionSamples.length + rendered.partSamples.length - failures.length,
    failed: failures.length,
    thresholdsByKind,
    partMappingsByKind: includePartComparisons ? partMappingsByKind : {},
    artifactRoot: failures.length > 0 && !noArtifacts ? path.relative(repoRoot, artifactRoot) : null,
  };
  console.log(JSON.stringify({ summary, compositionReports, partReports }, null, 2));

  if (expectFailures) {
    if (failures.length === 0) throw new Error("expected current Worker rig mismatches, but every sample passed");
  } else if (failures.length > 0) {
    const failedNames = failures.slice(0, 8).map(({ entry }) => {
      const suffix = entry.comparison === "part" ? ` part=${entry.part}` : " composition";
      return `${entry.kind}/${entry.label}${suffix}`;
    }).join(", ");
    throw new Error(`${failures.length} transparent pixel comparison sample(s) failed: ${failedNames}`);
  }
} finally {
  await browser.close();
  await staticServer.close();
  fs.rmSync(chromeProfileDir, { recursive: true, force: true });
}

function migrationSamplesFromBaseline(oracle) {
  const sampleByLabel = new Map(oracle.samples.map((sample) => [sample.label, sample]));
  const samples = [];
  for (const manifest of SVG_MIGRATION_MANIFESTS) {
    for (const label of manifest.requiredSamples) {
      const sample = sampleByLabel.get(label);
      if (!sample) throw new Error(`missing legacy oracle sample required by ${manifest.kind} manifest: ${label}`);
      samples.push({
        ...browserSampleFromOracle(sample),
        thresholds: manifest.compositionThresholds,
      });
    }
  }
  return samples;
}

function browserSampleFromOracle(sample) {
  return {
    label: sample.label,
    kind: sample.kind,
    teamColor: sample.teamColor,
    facing: sample.facing,
    weaponFacing: sample.weaponFacing,
    recoilProgress: sample.recoilProgress,
    state: sample.state,
    setupState: sample.setupState,
    resources: sample.resources,
    busy: sample.label.includes("busy"),
    fuelCue: sample.label.includes("low-oil") || sample.label.includes("oil-starved"),
    latchedNode: sample.label.includes("latched-node") ? 9001 : null,
    breakthroughTicks: sample.breakthroughTicks ?? 0,
  };
}

function writeFailureArtifacts({ renderedSample, entry }) {
  const sampleDir = path.join(artifactRoot, safeName(`${entry.label}/${entry.part || "composition"}`));
  fs.mkdirSync(sampleDir, { recursive: true });
  fs.writeFileSync(path.join(sampleDir, "legacy.png"), decodePngDataUrl(renderedSample.legacyPng));
  fs.writeFileSync(path.join(sampleDir, "rig.png"), decodePngDataUrl(renderedSample.rigPng));
  const diff = makeDiffRgba(renderedSample.legacy, renderedSample.rig, renderedSample.thresholds);
  fs.writeFileSync(path.join(sampleDir, "diff.png"), decodePngDataUrl(renderedSample.diffPng));
  fs.writeFileSync(path.join(sampleDir, "report.json"), `${JSON.stringify({
    ...entry,
    diffPixelCount: diff.data.reduce((count, value, index) => index % 4 === 3 && value > 0 ? count + 1 : count, 0),
  }, null, 2)}\n`);
}

function decodePngDataUrl(value) {
  const marker = "base64,";
  const index = value.indexOf(marker);
  if (index < 0) throw new Error("invalid PNG data URL");
  return Buffer.from(value.slice(index + marker.length), "base64");
}

function safeName(label) {
  return label.replace(/[^a-z0-9_.-]+/gi, "_").replace(/^_+|_+$/g, "");
}

function startStaticServer(root) {
  const server = http.createServer((request, response) => {
    const requestUrl = new URL(request.url || "/", "http://127.0.0.1");
    const decoded = decodeURIComponent(requestUrl.pathname);
    if (decoded === "/favicon.ico") {
      response.writeHead(204).end();
      return;
    }
    const relative = decoded.replace(/^\/+/, "") || "tests/fixtures/svg/transparent-unit-pixels.html";
    const filePath = path.resolve(root, relative);
    if (!filePath.startsWith(`${root}${path.sep}`)) {
      response.writeHead(403).end("Forbidden");
      return;
    }
    fs.stat(filePath, (statError, stat) => {
      if (statError || !stat.isFile()) {
        response.writeHead(404).end("Not found");
        return;
      }
      response.writeHead(200, { "Content-Type": contentType(filePath) });
      fs.createReadStream(filePath).pipe(response);
    });
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", reject);
      const address = server.address();
      resolve({
        url: `http://127.0.0.1:${address.port}/`,
        close: () => new Promise((closeResolve, closeReject) => {
          server.close((error) => error ? closeReject(error) : closeResolve());
        }),
      });
    });
  });
}

function contentType(filePath) {
  switch (path.extname(filePath)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
    case ".mjs":
      return "text/javascript; charset=utf-8";
    case ".svg":
      return "image/svg+xml; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    default:
      return "application/octet-stream";
  }
}

async function renderSamplesInBrowser({
  samples: browserSamples,
  bufferSize: size,
  fixedNow: now,
  thresholdsByKind: browserThresholdsByKind,
  partMappingsByKind: browserPartMappingsByKind,
  includeCompositionComparisons: browserIncludeCompositionComparisons,
  svgTextByKind: browserSvgTextByKind,
}) {
  const [
    protocol,
    entities,
    units,
    importer,
    runtime,
    animation,
  ] = await Promise.all([
    import(new URL("../../../client/src/protocol.js", window.location.href).href),
    import(new URL("../../../client/src/renderer/entities.js", window.location.href).href),
    import(new URL("../../../client/src/renderer/units.js", window.location.href).href),
    import(new URL("../../../client/src/renderer/rigs/svg_importer.js", window.location.href).href),
    import(new URL("../../../client/src/renderer/rigs/runtime.js", window.location.href).href),
    import(new URL("../../../client/src/renderer/rigs/animation.js", window.location.href).href),
  ]);
  const definitionsByKind = new Map();
  for (const [kind, svgText] of Object.entries(browserSvgTextByKind)) {
    const expectedKind = protocol.KIND[kind.toUpperCase()];
    const compiled = importer.compileSvgRig(svgText, { expectedKind });
    if (!compiled.ok) throw new Error(`failed to compile ${kind} rig: ${JSON.stringify(compiled.errors)}`);
    definitionsByKind.set(kind, compiled.definition);
  }

  const app = new PIXI.Application({
    width: size.width,
    height: size.height,
    antialias: false,
    resolution: 1,
    autoDensity: false,
    backgroundAlpha: 0,
    preserveDrawingBuffer: true,
    clearBeforeRender: true,
  });
  PIXI.settings.SCALE_MODE = PIXI.SCALE_MODES.NEAREST;
  app.renderer.roundPixels = false;
  document.body.appendChild(app.view);

  try {
    const compositionSamples = [];
    const partSamples = [];
    for (const sample of browserSamples) {
      if (!browserIncludeCompositionComparisons) continue;
      const legacy = renderLegacySample(sample);
      const rig = renderRigSample(sample);
      compositionSamples.push({
        label: sample.label,
        kind: sample.kind,
        teamColor: sample.teamColor,
        facing: sample.facing,
        state: sample.state,
        busy: sample.busy,
        thresholds: sample.thresholds ?? browserThresholdsByKind[sample.kind],
        legacy,
        rig,
        legacyPng: pngDataUrl(legacy),
        rigPng: pngDataUrl(rig),
        diffPng: pngDataUrl(diffBuffer(legacy, rig, (sample.thresholds ?? browserThresholdsByKind[sample.kind]).perChannelTolerance)),
      });
    }
    for (const sample of browserSamples) {
      const definition = definitionsByKind.get(sample.kind);
      for (const mapping of browserPartMappingsByKind[sample.kind] || []) {
        if (mapping.busyOnly && !sample.busy) continue;
        if (mapping.fuelOnly && !sample.fuelCue) continue;
        const legacy = renderLegacySample(sample, { legacyParts: [mapping.legacyPart] });
        const rig = renderRigSample(sample, { rigParts: mapping.rigParts });
        partSamples.push({
          label: sample.label,
          kind: sample.kind,
          legacyPart: mapping.legacyPart,
          rigParts: mapping.rigParts,
          missingRigParts: mapping.rigParts.filter((partId) => !definition.parts.some((part) => part.id === partId)),
          thresholds: mapping.thresholds,
          teamColor: sample.teamColor,
          facing: sample.facing,
          state: sample.state,
          busy: sample.busy,
          legacy,
          rig,
          legacyPng: pngDataUrl(legacy),
          rigPng: pngDataUrl(rig),
          diffPng: pngDataUrl(diffBuffer(legacy, rig, mapping.thresholds.perChannelTolerance)),
        });
      }
    }
    return { compositionSamples, partSamples };
  } finally {
    app.destroy(true, { children: true });
  }

  function renderLegacySample(sample, { legacyParts = null } = {}) {
    app.stage.removeChildren();
    const renderer = makeUnitRenderer();
    for (const name of ["unitShadows", "units"]) app.stage.addChild(renderer.layers[name]);
    const entity = makeEntity(sample);
    const colorByOwner = new Map([[entity.owner, parseInt(sample.teamColor.slice(1), 16)]]);
    const state = makeState(sample);
    const partCapture = legacyParts == null ? null : units.createLegacyUnitPartCapture({ includeParts: legacyParts });
    renderer._drawUnit(entity, colorByOwner, state, partCapture ? { partCapture } : {});
    app.renderer.render(app.stage);
    return readPixels();
  }

  function renderRigSample(sample, { rigParts = null } = {}) {
    app.stage.removeChildren();
    const entity = makeEntity(sample);
    const colorByOwner = new Map([[entity.owner, parseInt(sample.teamColor.slice(1), 16)]]);
    const state = makeState(sample);
    const definition = definitionsByKind.get(sample.kind);
    const instance = runtime.createUnitRigInstance(sample.kind, definition);
    const context = animation.createRigRenderContext(entity, {
      now,
      state,
      colorByOwner,
      map: { tileSize: 32 },
    });
    instance.update(entity, context, rigParts == null ? {} : { includeParts: rigParts });
    app.stage.addChild(instance.container);
    app.renderer.render(app.stage);
    const pixels = readPixels();
    instance.destroy();
    return pixels;
  }

  function makeUnitRenderer() {
    const layers = {
      unitShadows: new PIXI.Container(),
      units: new PIXI.Container(),
    };
    return {
      _pools: { unitShadows: new Map(), units: new Map() },
      _seen: { unitShadows: new Set(), units: new Set() },
      _setupVisuals: new Map(),
      _tankMotion: new Map(),
      _map: { tileSize: 32 },
      layers,
      _slot: entities._slot,
      _shadow: entities._shadow,
      _vehicleShadow: entities._vehicleShadow,
      _tintFor: entities._tintFor,
      _deployedWeaponSetupVisual: units._deployedWeaponSetupVisual,
      _tankMotionVisual: units._tankMotionVisual,
      _rigRenderContextFor: units._rigRenderContextFor,
      _drawUnit: units._drawUnit,
    };
  }

  function makeEntity(sample) {
    return {
      id: 100 + Math.abs(hashLabel(sample.label) % 10_000),
      kind: protocol.KIND[sample.kind.toUpperCase()],
      owner: 1,
      teamColor: sample.teamColor,
      x: size.width / 2,
      y: size.height / 2,
      hp: 32,
      maxHp: 50,
      state: sample.state,
      setupState: sample.setupState,
      facing: sample.facing,
      weaponFacing: sample.weaponFacing,
      recoilProgress: sample.recoilProgress,
      latchedNode: sample.latchedNode,
      breakthroughTicks: sample.breakthroughTicks,
    };
  }

  function makeState(sample) {
    return {
      playerId: 1,
      resources: sample.resources ?? { oil: 40 },
      weaponRecoil: () => sample.recoilProgress ?? 0,
      isOwnOwner: (owner) => owner === 1,
      isAllyOwner: () => false,
      isNeutralOwner: (owner) => owner === 0,
    };
  }

  function readPixels() {
    const gl = app.renderer.gl;
    const raw = new Uint8Array(size.width * size.height * 4);
    gl.readPixels(0, 0, size.width, size.height, gl.RGBA, gl.UNSIGNED_BYTE, raw);
    const flipped = new Uint8Array(raw.length);
    const rowBytes = size.width * 4;
    for (let y = 0; y < size.height; y += 1) {
      const sourceStart = (size.height - 1 - y) * rowBytes;
      flipped.set(raw.subarray(sourceStart, sourceStart + rowBytes), y * rowBytes);
    }
    return { width: size.width, height: size.height, data: Array.from(flipped) };
  }

  function pngDataUrl(buffer) {
    const canvas = document.createElement("canvas");
    canvas.width = buffer.width;
    canvas.height = buffer.height;
    const ctx = canvas.getContext("2d");
    ctx.putImageData(new ImageData(new Uint8ClampedArray(buffer.data), buffer.width, buffer.height), 0, 0);
    return canvas.toDataURL("image/png");
  }

  function diffBuffer(legacy, rig, tolerance) {
    const out = new Uint8Array(legacy.data.length);
    for (let i = 0; i < out.length; i += 4) {
      const dr = Math.abs(legacy.data[i] - rig.data[i]);
      const dg = Math.abs(legacy.data[i + 1] - rig.data[i + 1]);
      const db = Math.abs(legacy.data[i + 2] - rig.data[i + 2]);
      const da = Math.abs(legacy.data[i + 3] - rig.data[i + 3]);
      if (dr > tolerance || dg > tolerance || db > tolerance || da > tolerance) {
        out[i] = 255;
        out[i + 1] = Math.min(255, Math.max(dr, dg, db) * 3);
        out[i + 2] = 0;
        out[i + 3] = Math.max(160, da);
      }
    }
    return { width: legacy.width, height: legacy.height, data: Array.from(out) };
  }

  function hashLabel(label) {
    let hash = 0;
    for (let i = 0; i < label.length; i += 1) hash = ((hash << 5) - hash + label.charCodeAt(i)) | 0;
    return hash;
  }
}
