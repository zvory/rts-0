#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { TANK_RIG_SVG } from "../../client/src/renderer/rigs/tank_svg.js";
import { compileSvgRig } from "../../client/src/renderer/rigs/svg_importer.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../..");
const assetDir = path.join(repoRoot, "client/assets/rigs/tank-ps1");
const generatedDir = path.join(assetDir, "generated");
const metadataDir = path.join(assetDir, "metadata");
const sourceSvgPath = path.join(assetDir, "tank-contact-sheet.svg");
const sourcePngPath = path.join(assetDir, "tank-contact-sheet.png");
const atlasPath = path.join(assetDir, "tank-atlas.png");
const atlasModulePath = path.join(repoRoot, "client/src/renderer/rigs/tank_png_atlas.js");
const manifestPath = path.join(metadataDir, "manifest.json");
const promptPath = path.join(metadataDir, "prompt.md");

const VIEW_BOX = Object.freeze({ x: -40, y: -32, width: 80, height: 64 });
const DEFAULT_COLUMNS = 2;
const DEFAULT_SCALE = 3;
const DEFAULT_KEY = "#ff00ff";
const DEFAULT_LAYOUT = "tight";
const DEFAULT_PROFILE = "semantic";
const TIGHT_CELL_FILL = 0.72;
const GUIDE_SUBDIVISIONS = 8;
const GUIDE_BORDER_COLOR = "#00e5ff";
const GUIDE_MINOR_COLOR = "#80f6ff";
const GUIDE_CENTER_COLOR = "#fff4a3";
const GUIDE_MASK_COLORS = Object.freeze([GUIDE_BORDER_COLOR, GUIDE_MINOR_COLOR, GUIDE_CENTER_COLOR, DEFAULT_KEY]);
const DEFAULT_GUIDE_MASK_FUZZ = 12;
const DEFAULT_GUIDE_MASK_ALPHA_THRESHOLD = 20;
const DEFAULT_GUIDE_MASK_LINE_WIDTH = 12;
const DEFAULT_GUIDE_MASK_MORPHOLOGY = "Square:5";

function main() {
  const [command, ...rest] = process.argv.slice(2);
  const args = parseArgs(rest);
  if (command === "make-sheet") {
    makeSheet(args);
    return;
  }
  if (command === "write-atlas") {
    writeAtlas(args);
    return;
  }
  if (command === "write-prompt") {
    writePrompt();
    return;
  }
  usage();
  process.exitCode = 1;
}

function makeSheet(args) {
  ensureDirs();
  const columns = positiveInteger(args.columns, DEFAULT_COLUMNS);
  const scale = positiveInteger(args.scale, DEFAULT_SCALE);
  const key = String(args.key || DEFAULT_KEY);
  const layout = layoutArg(args.layout);
  const profile = profileArg(args.profile);
  const compiled = compileTankRig();
  const partIds = compiled.definition.parts.map((part) => part.id);
  const sheet = sheetForProfile(compiled.definition, profile);
  const cells = sheet.cells;
  const rows = Math.ceil(cells.length / columns);
  const cellW = VIEW_BOX.width * scale;
  const cellH = VIEW_BOX.height * scale;
  const sheetW = columns * cellW;
  const sheetH = rows * cellH;
  const elementsById = sourceElementsById(TANK_RIG_SVG);
  const referencePartIds = referencePartsForProfile(partIds, profile);
  const partElements = referencePartIds.map((id) => elementsById.get(id)).filter(Boolean);

  const body = [];
  body.push(`<rect x="0" y="0" width="${sheetW}" height="${sheetH}" fill="${key}" />`);
  cells.forEach((_cellId, index) => {
    const col = index % columns;
    const row = Math.floor(index / columns);
    appendCellGuides(body, col * cellW, row * cellH, cellW, cellH);
  });
  cells.forEach((cellId, index) => {
    const col = index % columns;
    const row = Math.floor(index / columns);
    const x = col * cellW;
    const y = row * cellH;
    if (cellId === "reference.full") {
      body.push(
        `<g transform="translate(${round(x - VIEW_BOX.x * scale)} ${round(y - VIEW_BOX.y * scale)}) scale(${round(scale)} ${round(scale)})">`,
        ...partElements.map((element) => prepSourceElement(element, { stripTransform: false })),
        "</g>"
      );
      return;
    }
    const semanticElement = sheet.sheetElements.find((element) => element.id === cellId);
    if (semanticElement) {
      const parts = semanticElement.sourceParts
        .map((partId) => compiled.definition.parts.find((part) => part.id === partId))
        .filter(Boolean);
      const frame = groupFrameGeometry(parts, cellW, cellH, layout);
      const groupElements = (semanticElement.renderParts || semanticElement.sourceParts)
        .map((partId) => elementsById.get(partId))
        .filter(Boolean);
      body.push(
        `<g transform="translate(${round(x + frame.originX)} ${round(y + frame.originY)}) scale(${round(frame.pixelsPerUnitX)} ${round(frame.pixelsPerUnitY)})">`,
        ...groupElements.map((element) => prepSourceElement(element, { stripTransform: false })),
        "</g>",
      );
      return;
    }
    const part = compiled.definition.parts.find((candidate) => candidate.id === cellId);
    const element = elementsById.get(cellId);
    if (!part || !element) return;
    const frame = frameGeometry(part, cellW, cellH, layout);
    body.push(
      `<g transform="translate(${round(x + frame.originX)} ${round(y + frame.originY)}) scale(${round(frame.pixelsPerUnitX)} ${round(frame.pixelsPerUnitY)})">`,
      prepSourceElement(element, { stripTransform: true }),
      "</g>",
    );
  });

  const svg = [
    `<!-- Generated by scripts/art/tank-raster-pipeline.mjs make-sheet. -->`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="${sheetW}" height="${sheetH}" viewBox="0 0 ${sheetW} ${sheetH}" shape-rendering="crispEdges">`,
    ...body,
    "</svg>",
    "",
  ].join("\n");
  fs.writeFileSync(sourceSvgPath, svg);
  run("magick", [sourceSvgPath, sourcePngPath]);
  const grid = {
    unit: "tank",
    columns,
    rows,
    scale,
    key,
    layout,
    profile,
    viewBox: VIEW_BOX,
    cells,
    partIds,
    frameSources: sheet.frameSources,
    semanticSprites: sheet.sprites,
    semanticSheetElements: sheet.sheetElements,
    guides: {
      outerCellBox: true,
      subdivisions: GUIDE_SUBDIVISIONS,
      centerLines: true,
      borderColor: GUIDE_BORDER_COLOR,
      minorColor: GUIDE_MINOR_COLOR,
      centerColor: GUIDE_CENTER_COLOR,
    },
    sourceSvg: rel(sourceSvgPath),
    sourcePng: rel(sourcePngPath),
  };
  fs.writeFileSync(path.join(metadataDir, "source-grid.json"), `${JSON.stringify(grid, null, 2)}\n`);
  writePrompt();
  console.log(`wrote ${rel(sourcePngPath)}`);
}

function writeAtlas(args) {
  ensureDirs();
  const sheet = path.resolve(repoRoot, args.sheet || sourcePngPath);
  const columns = positiveInteger(args.columns, DEFAULT_COLUMNS);
  const layout = layoutArg(args.layout);
  const profile = profileArg(args.profile);
  const transparentKey = args["transparent-key"];
  const enabled = !args.disabled && args.enabled !== "false";
  const blankCells = parseListArg(args["blank-cells"]);
  const normalizeVisibleBounds = Boolean(args["normalize-visible-bounds"]);
  const clearCellEdgeAlpha = nonNegativeInteger(args["clear-cell-edge-alpha"], normalizeVisibleBounds ? 16 : 0);
  const visiblePadding = nonNegativeInteger(args["visible-padding"], normalizeVisibleBounds ? 2 : 0);
  const ignoreGuideBounds = Boolean(args["ignore-guide-bounds"]);
  const guideMaskFuzz = percentArg(args["guide-mask-fuzz"], DEFAULT_GUIDE_MASK_FUZZ);
  const guideMaskAlphaThreshold = percentArg(args["guide-mask-alpha-threshold"], DEFAULT_GUIDE_MASK_ALPHA_THRESHOLD);
  const guideMaskLineWidth = nonNegativeInteger(args["guide-mask-line-width"], DEFAULT_GUIDE_MASK_LINE_WIDTH);
  const guideMaskMorphology = morphologyArg(args["guide-mask-morphology"], DEFAULT_GUIDE_MASK_MORPHOLOGY);
  const imageVersion = safeVersionArg(args["image-version"]);
  const promptFile = args["prompt-file"] ? path.resolve(repoRoot, String(args["prompt-file"])) : promptPath;
  const brightness = percentArg(args.brightness, 100);
  const saturation = percentArg(args.saturation, 100);
  const hue = percentArg(args.hue, 100);
  const compiled = compileTankRig();
  const partIds = compiled.definition.parts.map((part) => part.id);
  const sheetSpec = sheetForProfile(compiled.definition, profile);
  const cells = sheetSpec.cells;
  const rows = Math.ceil(cells.length / columns);
  if (!fs.existsSync(sheet)) {
    throw new Error(`sheet not found: ${sheet}`);
  }
  if (transparentKey) {
    run("magick", [sheet, "-alpha", "set", "-transparent", String(transparentKey), atlasPath]);
  } else if (path.resolve(sheet) !== path.resolve(atlasPath)) {
    fs.copyFileSync(sheet, atlasPath);
  }

  const { width, height } = identifyImage(atlasPath);
  if (blankCells.length > 0) {
    clearAtlasCellAlpha(atlasPath, blankCells, cells, columns, rows, width, height);
  }
  if (clearCellEdgeAlpha > 0) {
    clearAtlasCellEdgeAlpha(atlasPath, cells, columns, rows, width, height, clearCellEdgeAlpha);
  }
  if (brightness !== 100 || saturation !== 100 || hue !== 100) {
    modulateAtlasColor(atlasPath, { brightness, saturation, hue });
  }
  const frames = {};
  const boundsMaskPath = normalizeVisibleBounds && ignoreGuideBounds
    ? makeGuideBoundsMask(atlasPath, cells, columns, rows, width, height, {
      fuzz: guideMaskFuzz,
      alphaThreshold: guideMaskAlphaThreshold,
      lineWidth: guideMaskLineWidth,
      morphology: guideMaskMorphology,
    })
    : null;
  const sprites = profile === "semantic"
    ? atlasSpritesForSemanticProfile(compiled.definition, cells, columns, rows, width, height, layout, {
      atlasPath,
      boundsAtlasPath: boundsMaskPath || atlasPath,
      normalizeVisibleBounds,
      visiblePadding,
    })
    : [];
  if (boundsMaskPath) fs.rmSync(path.dirname(boundsMaskPath), { recursive: true, force: true });
  if (profile !== "semantic") {
    const frameSources = sheetSpec.frameSources;
    for (const partId of partIds) {
      const cellId = frameSources[partId];
      const index = cells.indexOf(cellId);
      if (index < 0) continue;
      const frame = cellFrame(index, columns, rows, width, height);
      const part = compiled.definition.parts.find((candidate) => candidate.id === partId);
      const geometry = frameGeometry(part, frame.w, frame.h, layout);
      frames[partId] = {
        ...frame,
        originX: geometry.originX,
        originY: geometry.originY,
        pixelsPerUnitX: geometry.pixelsPerUnitX,
        pixelsPerUnitY: geometry.pixelsPerUnitY,
        sourceCell: cellId,
      };
    }
  }

  const atlas = {
    enabled,
    unit: "tank",
    image: `/assets/rigs/tank-ps1/tank-atlas.png${imageVersion ? `?v=${imageVersion}` : ""}`,
    viewBox: VIEW_BOX,
    grid: {
      columns,
      rows,
      layout,
      profile,
      width,
      height,
      sourceSheet: rel(sheet),
      cells,
      frameSources: sheetSpec.frameSources,
      normalization: {
        visibleBounds: normalizeVisibleBounds,
        visiblePadding,
        clearCellEdgeAlpha,
        blankCells,
        ignoreGuideBounds,
        guideMaskFuzz: ignoreGuideBounds ? guideMaskFuzz : null,
        guideMaskAlphaThreshold: ignoreGuideBounds ? guideMaskAlphaThreshold : null,
        guideMaskLineWidth: ignoreGuideBounds ? guideMaskLineWidth : null,
        guideMaskMorphology: ignoreGuideBounds ? guideMaskMorphology : null,
      },
      imageVersion,
      colorAdjustment: {
        brightness,
        saturation,
        hue,
      },
    },
    frames,
    sprites,
  };
  writeAtlasModule(atlas);
  writeManifest({
    columns,
    rows,
    layout,
    profile,
    width,
    height,
    sourceSheet: rel(sheet),
    atlas: rel(atlasPath),
    atlasModule: rel(atlasModulePath),
    promptFile: rel(promptFile),
    prompt: rel(promptFile),
    model: args.model || "built-in image_gen",
    notes: args.notes || "Prototype tank-only PNG rig atlas. SVG rig remains the source for anchors and animation.",
    enabled,
  });
  console.log(`wrote ${rel(atlasPath)} and ${rel(atlasModulePath)}`);
}

function writePrompt() {
  ensureDirs();
  const prompt = `Use case: stylized-concept
Asset type: top-down RTS unit raster atlas for Bewegungskrieg
Primary request: Restyle this guided semantic tank contact sheet into a coherent, strict top-down 1940s German Tiger I heavy tank, Panzerkampfwagen VI Tiger Ausf. E, with every grouped component preserved in its same grid cell.
Input image role: edit target and layout reference. The sheet has exactly six boxed cells in a 2x3 grid: assembled no-track reference tank, empty no-track placeholder, hull assembly, turret/coax assembly, separate main barrel, and one unused empty cell. The visible guide boxes, subgrid lines, and center marks are alignment guides only.
Style/medium: clean RTS-readable PlayStation 1 era raster art, low-resolution textured/pixely surfaces, grounded Tiger I proportions, not cartoonish. Simpler than concept art: broad slabs, very few small hatches, very few bolts, no dense top-deck clutter.
Composition/framing: preserve the exact 2x3 grid, exact six-cell count, exact cell order, boxed cell boundaries, centered component framing, relative component silhouette, and top-down orientation. Use the smaller guide grid inside each cell to keep scale and center alignment stable. Keep each component isolated inside its original cell.
Color/materials: bright neutral gray-blue team-colorable armor on hull, turret, and barrel; the barrel should be the same tintable team-color armor value as the hull, not black fixed metal. Subtle broad panel shading, very light grime, no glossy modern materials.
Outline: add a clean dark/black RTS-readable outline around the hull, turret, coax detail, and separate main barrel. Keep the outline crisp and simple, not a thick cartoon border.
Background: perfectly flat solid ${DEFAULT_KEY} chroma-key background in every cell.
Empty cells: leave the no-track placeholder cell and the unused final cell as flat ${DEFAULT_KEY} only, with no tank art.
Hull cell: generate only the no-track Tiger I hull/body armor silhouette, centered and matching the reference scale. The hull must not be a perfect rectangle: use a simple Tiger I top-view plan with stepped/chamfered front corners, slight inset/stepped rear plate, and subtle side skirt/block breaks. Keep it simplified and readable, with no tracks.
Turret cell: generate only the Tiger I turret body and tiny coax detail, with no main cannon barrel attached; compact angular turret with a clear black outline, not a modern rounded turret.
Barrel cell: generate only the separate Tiger I 88mm main cannon barrel as a straight centered component, long axis left-to-right, same team-color neutral gray-blue as the hull, readable thickness, crisp black outline, no turret attached.
Constraints: strict top-down orthographic view; no tracks anywhere; no perspective tilt; no drop shadow; no text, labels, numbers, watermarks, arrows, Balkenkreuz, swastikas, insignia, or extra UI; no merged cells; no fuel warning/no-oil indicator; do not add missing parts; do not remove any required hull/turret/barrel art part; leave empty background areas empty. Keep guide lines thin and separate from the sprite art; do not turn the guide grid into armor seams or detail.
Avoid: perfect rectangular hull, tracks, treads, wheels, sprockets, gears, dense top-deck detail, extra hatches, extra barrels, fuel icons, warning symbols, thick cartoon outlines, toy proportions, oversized turret/barrel, painterly blur, photorealistic perspective, dramatic lighting, gradients in the background, camouflage that hides the silhouette.
`;
  fs.writeFileSync(promptPath, prompt);
  console.log(`wrote ${rel(promptPath)}`);
}

function writeAtlasModule(atlas) {
  const js = `// Generated by scripts/art/tank-raster-pipeline.mjs write-atlas.
// The SVG rig remains the source of animation bindings and anchors; this atlas
// only replaces tank part pixels for the prototype raster art path.
function deepFreeze(value) {
  if (!value || typeof value !== "object") return value;
  Object.freeze(value);
  for (const child of Object.values(value)) deepFreeze(child);
  return value;
}

export const TANK_PNG_RIG_ATLAS = deepFreeze(${JSON.stringify(atlas, null, 2)});
`;
  fs.writeFileSync(atlasModulePath, js);
}

function writeManifest(details) {
  const manifest = {
    pipelineVersion: 1,
    generatedAt: new Date().toISOString(),
    sourceSvgModule: "client/src/renderer/rigs/tank_svg.js",
    sourceSvgRuntimeKind: "tank",
    promptFile: rel(promptPath),
    ...details,
  };
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

function compileTankRig() {
  const compiled = compileSvgRig(TANK_RIG_SVG, { expectedKind: "tank" });
  if (!compiled.ok) {
    throw new Error(`tank SVG rig failed to compile: ${JSON.stringify(compiled.errors)}`);
  }
  return compiled;
}

function sourceElementsById(svgText) {
  const elements = new Map();
  const elementRe = /<(?<tag>path|polygon|polyline|rect|circle|ellipse|line)\b(?=[^>]*\bid="(?<id>[^"]+)")[^>]*\/>/g;
  for (const match of svgText.matchAll(elementRe)) {
    elements.set(match.groups.id, match[0]);
  }
  return elements;
}

function prepSourceElement(element, { stripTransform }) {
  const sourceId = element.match(/\sid="([^"]+)"/)?.[1] || "";
  let out = element
    .replace(/\sdata-rts-[a-z-]+="[^"]*"/g, "")
    .replace(/\sid="([^"]+)"/, ' id="source.$1"');
  if (sourceId === "part.barrel") {
    out = out.replace(/\sstroke="[^"]*"/, ' stroke="#d8d0b0"');
  }
  if (stripTransform) out = out.replace(/\stransform="[^"]*"/g, "");
  return out;
}

function frameGeometry(part, cellW, cellH, layout) {
  if (layout === "full") {
    return {
      originX: -VIEW_BOX.x * (cellW / VIEW_BOX.width),
      originY: -VIEW_BOX.y * (cellH / VIEW_BOX.height),
      pixelsPerUnitX: cellW / VIEW_BOX.width,
      pixelsPerUnitY: cellH / VIEW_BOX.height,
    };
  }
  const bounds = partBounds(part);
  const width = Math.max(1, bounds.maxX - bounds.minX);
  const height = Math.max(1, bounds.maxY - bounds.minY);
  const fit = Math.min((cellW * TIGHT_CELL_FILL) / width, (cellH * TIGHT_CELL_FILL) / height);
  const cx = (bounds.minX + bounds.maxX) * 0.5;
  const cy = (bounds.minY + bounds.maxY) * 0.5;
  return {
    originX: cellW * 0.5 - cx * fit,
    originY: cellH * 0.5 - cy * fit,
    pixelsPerUnitX: fit,
    pixelsPerUnitY: fit,
  };
}

function groupFrameGeometry(parts, cellW, cellH, layout) {
  if (layout === "full") {
    return {
      originX: -VIEW_BOX.x * (cellW / VIEW_BOX.width),
      originY: -VIEW_BOX.y * (cellH / VIEW_BOX.height),
      pixelsPerUnitX: cellW / VIEW_BOX.width,
      pixelsPerUnitY: cellH / VIEW_BOX.height,
    };
  }
  const bounds = unionBounds(parts.map(partBounds));
  const width = Math.max(1, bounds.maxX - bounds.minX);
  const height = Math.max(1, bounds.maxY - bounds.minY);
  const fit = Math.min((cellW * TIGHT_CELL_FILL) / width, (cellH * TIGHT_CELL_FILL) / height);
  const cx = (bounds.minX + bounds.maxX) * 0.5;
  const cy = (bounds.minY + bounds.maxY) * 0.5;
  return {
    originX: cellW * 0.5 - cx * fit,
    originY: cellH * 0.5 - cy * fit,
    pixelsPerUnitX: fit,
    pixelsPerUnitY: fit,
  };
}

function appendCellGuides(body, x, y, cellW, cellH) {
  body.push(`<g fill="none" shape-rendering="crispEdges">`);
  for (let i = 1; i < GUIDE_SUBDIVISIONS; i += 1) {
    const tx = round(x + (cellW * i) / GUIDE_SUBDIVISIONS);
    const ty = round(y + (cellH * i) / GUIDE_SUBDIVISIONS);
    const major = i === GUIDE_SUBDIVISIONS / 2;
    const stroke = major ? GUIDE_CENTER_COLOR : GUIDE_MINOR_COLOR;
    const opacity = major ? 0.66 : 0.32;
    body.push(
      `<line x1="${tx}" y1="${y}" x2="${tx}" y2="${round(y + cellH)}" stroke="${stroke}" stroke-width="1" opacity="${opacity}" />`,
      `<line x1="${x}" y1="${ty}" x2="${round(x + cellW)}" y2="${ty}" stroke="${stroke}" stroke-width="1" opacity="${opacity}" />`,
    );
  }
  body.push(
    `<rect x="${round(x + 1)}" y="${round(y + 1)}" width="${round(cellW - 2)}" height="${round(cellH - 2)}" stroke="${GUIDE_BORDER_COLOR}" stroke-width="2" opacity="0.9" />`,
    `<rect x="${round(x + cellW * 0.25)}" y="${round(y + cellH * 0.25)}" width="${round(cellW * 0.5)}" height="${round(cellH * 0.5)}" stroke="${GUIDE_CENTER_COLOR}" stroke-width="1" opacity="0.45" />`,
    "</g>",
  );
}

function sheetForProfile(definition, profile) {
  const partIds = definition.parts.map((part) => part.id);
  if (profile === "semantic") {
    const sprites = semanticSprites(partIds);
    const sheetElements = semanticSheetElements(partIds);
    return {
      cells: ["reference.full", ...sheetElements.map((element) => element.id), "unused.blank"],
      frameSources: semanticFrameSources(sprites),
      sprites,
      sheetElements,
    };
  }
  const frameSources = frameSourcesForProfile(partIds, profile);
  return {
    cells: cellsForFrameSources(frameSources),
    frameSources,
    sprites: [],
    sheetElements: [],
  };
}

function referencePartsForProfile(partIds, profile) {
  if (profile === "semantic") {
    return partIds.filter((partId) => (
      partId !== "part.shadow" &&
      !partId.startsWith("part.fuelCue.") &&
      !partId.startsWith("part.tank.flash") &&
      !partId.startsWith("part.track.") &&
      !partId.startsWith("part.tread.")
    ));
  }
  return partIds;
}

function semanticSprites(partIds) {
  const leftTreads = partIds.filter((partId) => /^part\.tread\.left\./.test(partId));
  const rightTreads = partIds.filter((partId) => /^part\.tread\.right\./.test(partId));
  return [
    {
      id: "sprite.track.left",
      sourceCell: "sprite.track",
      animationPart: "part.track.left",
      sourceParts: ["part.track.left", ...leftTreads],
      tintSlot: "fixed",
      drawOrder: 10,
    },
    {
      id: "sprite.track.right",
      sourceCell: "sprite.track",
      animationPart: "part.track.right",
      sourceParts: ["part.track.right", ...rightTreads],
      tintSlot: "fixed",
      drawOrder: 11,
    },
    {
      id: "sprite.hull",
      animationPart: "part.hull",
      sourceParts: ["part.hull", "part.hull.shadow", "part.hull.nose", "part.hull.noseShadow", "part.noseTick"],
      tintSlot: "team",
      drawOrder: 20,
    },
    {
      id: "sprite.turret",
      animationPart: "part.turret",
      sourceParts: ["part.coaxBarrel", "part.turret"],
      tintSlot: "team-light",
      drawOrder: 30,
    },
    {
      id: "sprite.barrel",
      animationPart: "part.barrel",
      sourceParts: ["part.barrel"],
      tintSlot: "team",
      drawOrder: 29,
    },
  ].map((sprite) => ({
    ...sprite,
    sourceParts: sprite.sourceParts.filter((partId) => partIds.includes(partId)),
  })).filter((sprite) => sprite.sourceParts.length > 0);
}

function semanticSheetElements(partIds) {
  const leftTreads = partIds.filter((partId) => /^part\.tread\.left\./.test(partId));
  const trackSourceParts = ["part.track.left", ...leftTreads].filter((partId) => partIds.includes(partId));
  return [
    {
      id: "sprite.track",
      sourceParts: trackSourceParts,
      renderParts: [],
      description: "empty no-track placeholder used only to suppress SVG track parts",
    },
    {
      id: "sprite.hull",
      sourceParts: ["part.hull", "part.hull.shadow", "part.hull.nose", "part.hull.noseShadow", "part.noseTick"],
      description: "hull assembly",
    },
    {
      id: "sprite.turret",
      sourceParts: ["part.coaxBarrel", "part.turret"],
      description: "turret and coax assembly, excluding the main barrel",
    },
    {
      id: "sprite.barrel",
      sourceParts: ["part.barrel"],
      description: "separate main cannon barrel",
    },
  ].map((element) => ({
    ...element,
    sourceParts: element.sourceParts.filter((partId) => partIds.includes(partId)),
    renderParts: element.renderParts?.filter((partId) => partIds.includes(partId)),
  })).filter((element) => element.sourceParts.length > 0);
}

function semanticFrameSources(sprites) {
  const sources = {};
  for (const sprite of sprites) {
    const cellId = sprite.sourceCell || sprite.id;
    for (const partId of sprite.sourceParts) sources[partId] = cellId;
  }
  return sources;
}

function atlasSpritesForSemanticProfile(definition, cells, columns, rows, width, height, layout, options = {}) {
  return semanticSprites(definition.parts.map((part) => part.id)).map((sprite) => {
    const cellId = sprite.sourceCell || sprite.id;
    const index = cells.indexOf(cellId);
    if (index < 0) return null;
    const cell = cellFrame(index, columns, rows, width, height);
    const parts = sprite.sourceParts
      .map((partId) => definition.parts.find((part) => part.id === partId))
      .filter(Boolean);
    const visibleFrame = options.normalizeVisibleBounds
      ? visibleFrameForCell(options.boundsAtlasPath || options.atlasPath, cell, options.visiblePadding)
      : null;
    const frame = visibleFrame || cell;
    const geometry = visibleFrame
      ? normalizedVisibleFrameGeometry(parts, visibleFrame)
      : groupFrameGeometry(parts, frame.w, frame.h, layout);
    return {
      ...sprite,
      sourceCell: cellId,
      frame: {
        ...frame,
        originX: geometry.originX,
        originY: geometry.originY,
        pixelsPerUnitX: geometry.pixelsPerUnitX,
        pixelsPerUnitY: geometry.pixelsPerUnitY,
      },
    };
  }).filter(Boolean);
}

function makeGuideBoundsMask(file, cells, columns, rows, width, height, { fuzz, alphaThreshold, lineWidth, morphology }) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "tank-raster-bounds-"));
  const out = path.join(dir, "guide-bounds-mask.png");
  const args = [
    file,
    "-fuzz",
    `${fuzz}%`,
    ...GUIDE_MASK_COLORS.flatMap((color) => ["-transparent", color]),
    "+fuzz",
    "-channel",
    "A",
    "-threshold",
    `${alphaThreshold}%`,
    "+channel",
    out,
  ];
  run("magick", args);
  clearAtlasGuideAlpha(out, cells, columns, rows, width, height, lineWidth);
  morphAtlasAlpha(out, { alphaThreshold, morphology });
  return out;
}

function visibleFrameForCell(file, cell, padding = 0) {
  if (!file) return null;
  const result = run("magick", [
    file,
    "-crop",
    `${cell.w}x${cell.h}+${cell.x}+${cell.y}`,
    "+repage",
    "-alpha",
    "extract",
    "-trim",
    "-format",
    "%w %h %X %Y",
    "info:",
  ], { capture: true, allowFailure: true });
  if (result.status !== 0 || /geometry does not contain image/i.test(result.stderr || "")) return null;
  const [w, h, offsetX, offsetY] = result.stdout.trim().split(/\s+/).map(Number);
  if (!Number.isFinite(w) || !Number.isFinite(h) || !Number.isFinite(offsetX) || !Number.isFinite(offsetY)) return null;
  if (w <= 0 || h <= 0 || offsetX < 0 || offsetY < 0) return null;
  const pad = Math.max(0, Math.floor(padding));
  const x = Math.max(cell.x, cell.x + offsetX - pad);
  const y = Math.max(cell.y, cell.y + offsetY - pad);
  const right = Math.min(cell.x + cell.w, cell.x + offsetX + w + pad);
  const bottom = Math.min(cell.y + cell.h, cell.y + offsetY + h + pad);
  return {
    x,
    y,
    w: Math.max(1, right - x),
    h: Math.max(1, bottom - y),
    visibleBounds: {
      x: cell.x + offsetX,
      y: cell.y + offsetY,
      w,
      h,
    },
  };
}

function normalizedVisibleFrameGeometry(parts, frame) {
  const bounds = unionBounds(parts.map(partBounds));
  const targetW = Math.max(1, bounds.maxX - bounds.minX);
  const targetH = Math.max(1, bounds.maxY - bounds.minY);
  const visible = frame.visibleBounds || frame;
  const pixelsPerUnitX = Math.max(1, visible.w) / targetW;
  const pixelsPerUnitY = Math.max(1, visible.h) / targetH;
  return {
    originX: visible.x - frame.x - bounds.minX * pixelsPerUnitX,
    originY: visible.y - frame.y - bounds.minY * pixelsPerUnitY,
    pixelsPerUnitX,
    pixelsPerUnitY,
  };
}

function cellFrame(index, columns, rows, width, height) {
  const col = index % columns;
  const row = Math.floor(index / columns);
  const x0 = Math.round((col * width) / columns);
  const x1 = Math.round(((col + 1) * width) / columns);
  const y0 = Math.round((row * height) / rows);
  const y1 = Math.round(((row + 1) * height) / rows);
  return {
    x: x0,
    y: y0,
    w: x1 - x0,
    h: y1 - y0,
  };
}

function cellsForFrameSources(frameSources) {
  return ["reference.full", ...new Set(Object.values(frameSources))];
}

function frameSourcesForProfile(partIds, profile) {
  const sources = {};
  for (const partId of partIds) sources[partId] = compactSourceForPart(partId, profile);
  return sources;
}

function compactSourceForPart(partId, profile) {
  if (profile === "full") return partId;
  if (/^part\.track\./.test(partId)) return "part.track.left";
  if (/^part\.tread\./.test(partId)) return "part.tread.left.0";
  return partId;
}

function partBounds(part) {
  const geometry = part?.geometry || {};
  const points = [];
  if (geometry.type === "rect") {
    points.push([geometry.x, geometry.y], [geometry.x + geometry.width, geometry.y + geometry.height]);
  } else if (geometry.type === "line") {
    points.push([geometry.from.x, geometry.from.y], [geometry.to.x, geometry.to.y]);
  } else if (geometry.type === "polygon" || geometry.type === "polyline") {
    for (const point of geometry.points || []) points.push([point.x, point.y]);
  } else if (geometry.type === "circle") {
    points.push([geometry.cx - geometry.r, geometry.cy - geometry.r], [geometry.cx + geometry.r, geometry.cy + geometry.r]);
  } else if (geometry.type === "ellipse") {
    points.push([geometry.cx - geometry.rx, geometry.cy - geometry.ry], [geometry.cx + geometry.rx, geometry.cy + geometry.ry]);
  } else if (geometry.bounds) {
    points.push([geometry.bounds.minX, geometry.bounds.minY], [geometry.bounds.maxX, geometry.bounds.maxY]);
  }
  if (points.length === 0) {
    return { minX: VIEW_BOX.x, minY: VIEW_BOX.y, maxX: VIEW_BOX.x + VIEW_BOX.width, maxY: VIEW_BOX.y + VIEW_BOX.height };
  }
  const strokePad = Math.max(part?.paint?.strokeWidth || 0, geometry.strokeWidth || 0, 1) * 0.5 + 0.5;
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  return {
    minX: Math.min(...xs) - strokePad,
    minY: Math.min(...ys) - strokePad,
    maxX: Math.max(...xs) + strokePad,
    maxY: Math.max(...ys) + strokePad,
  };
}

function unionBounds(boundsList) {
  const bounds = boundsList.filter(Boolean);
  if (bounds.length === 0) {
    return { minX: VIEW_BOX.x, minY: VIEW_BOX.y, maxX: VIEW_BOX.x + VIEW_BOX.width, maxY: VIEW_BOX.y + VIEW_BOX.height };
  }
  return {
    minX: Math.min(...bounds.map((bound) => bound.minX)),
    minY: Math.min(...bounds.map((bound) => bound.minY)),
    maxX: Math.max(...bounds.map((bound) => bound.maxX)),
    maxY: Math.max(...bounds.map((bound) => bound.maxY)),
  };
}

function layoutArg(value) {
  const layout = String(value || DEFAULT_LAYOUT);
  if (layout !== "tight" && layout !== "full") {
    throw new Error(`unsupported layout ${JSON.stringify(layout)}; expected tight or full`);
  }
  return layout;
}

function profileArg(value) {
  const profile = String(value || DEFAULT_PROFILE);
  if (profile !== "semantic" && profile !== "compact" && profile !== "full") {
    throw new Error(`unsupported profile ${JSON.stringify(profile)}; expected semantic, compact, or full`);
  }
  return profile;
}

function round(value) {
  return Number(value.toFixed(6));
}

function identifyImage(file) {
  const result = run("magick", ["identify", "-format", "%w %h", file], { capture: true });
  const [width, height] = result.stdout.trim().split(/\s+/).map(Number);
  if (!Number.isFinite(width) || !Number.isFinite(height)) {
    throw new Error(`failed to identify image dimensions for ${file}: ${result.stdout}`);
  }
  return { width, height };
}

function clearAtlasCellAlpha(file, cellIds, cells, columns, rows, width, height) {
  const wanted = new Set(cellIds);
  const drawOps = [];
  cells.forEach((cellId, index) => {
    if (!wanted.has(cellId)) return;
    const frame = cellFrame(index, columns, rows, width, height);
    drawOps.push(rectangleDrawOp(frame.x, frame.y, frame.x + frame.w, frame.y + frame.h));
  });
  if (drawOps.length === 0) return;
  clearAtlasAlpha(file, drawOps);
}

function clearAtlasCellEdgeAlpha(file, cells, columns, rows, width, height, inset) {
  const amount = Math.max(0, Math.floor(inset));
  if (amount <= 0) return;
  const drawOps = [];
  cells.forEach((_cellId, index) => {
    const frame = cellFrame(index, columns, rows, width, height);
    const right = frame.x + frame.w;
    const bottom = frame.y + frame.h;
    const x2 = Math.min(right, frame.x + amount);
    const y2 = Math.min(bottom, frame.y + amount);
    const x1 = Math.max(frame.x, right - amount);
    const y1 = Math.max(frame.y, bottom - amount);
    drawOps.push(
      rectangleDrawOp(frame.x, frame.y, x2, bottom),
      rectangleDrawOp(x1, frame.y, right, bottom),
      rectangleDrawOp(frame.x, frame.y, right, y2),
      rectangleDrawOp(frame.x, y1, right, bottom),
    );
  });
  clearAtlasAlpha(file, drawOps);
}

function clearAtlasGuideAlpha(file, cells, columns, rows, width, height, lineWidth) {
  const amount = Math.max(0, Math.floor(lineWidth));
  if (amount <= 0) return;
  const drawOps = [];
  cells.forEach((_cellId, index) => {
    const frame = cellFrame(index, columns, rows, width, height);
    for (let i = 1; i < GUIDE_SUBDIVISIONS; i += 1) {
      const x = frame.x + (frame.w * i) / GUIDE_SUBDIVISIONS;
      const y = frame.y + (frame.h * i) / GUIDE_SUBDIVISIONS;
      drawOps.push(verticalLineDrawOp(x, frame.y, frame.y + frame.h, amount));
      drawOps.push(horizontalLineDrawOp(y, frame.x, frame.x + frame.w, amount));
    }

    drawOps.push(
      verticalLineDrawOp(frame.x + 1, frame.y, frame.y + frame.h, amount),
      verticalLineDrawOp(frame.x + frame.w - 1, frame.y, frame.y + frame.h, amount),
      horizontalLineDrawOp(frame.y + 1, frame.x, frame.x + frame.w, amount),
      horizontalLineDrawOp(frame.y + frame.h - 1, frame.x, frame.x + frame.w, amount),
    );

    const insetX = frame.w * 0.25;
    const insetY = frame.h * 0.25;
    const x0 = frame.x + insetX;
    const x1 = frame.x + frame.w - insetX;
    const y0 = frame.y + insetY;
    const y1 = frame.y + frame.h - insetY;
    drawOps.push(
      verticalLineDrawOp(x0, y0, y1, amount),
      verticalLineDrawOp(x1, y0, y1, amount),
      horizontalLineDrawOp(y0, x0, x1, amount),
      horizontalLineDrawOp(y1, x0, x1, amount),
    );
  });
  clearAtlasAlpha(file, drawOps);
}

function clearAtlasAlpha(file, drawOps) {
  run("magick", [
    file,
    "(",
    "+clone",
    "-alpha",
    "extract",
    "-fill",
    "black",
    "-draw",
    drawOps.join(" "),
    ")",
    "-alpha",
    "off",
    "-compose",
    "CopyOpacity",
    "-composite",
    file,
  ]);
}

function morphAtlasAlpha(file, { alphaThreshold, morphology }) {
  if (!morphology) return;
  run("magick", [
    file,
    "(",
    "+clone",
    "-alpha",
    "extract",
    "-threshold",
    `${alphaThreshold}%`,
    "-morphology",
    "Open",
    morphology,
    ")",
    "-alpha",
    "off",
    "-compose",
    "CopyOpacity",
    "-composite",
    file,
  ]);
}

function modulateAtlasColor(file, { brightness, saturation, hue }) {
  run("magick", [
    file,
    "-modulate",
    `${brightness},${saturation},${hue}`,
    file,
  ]);
}

function rectangleDrawOp(x0, y0, x1, y1) {
  return `rectangle ${Math.round(x0)},${Math.round(y0)} ${Math.round(x1)},${Math.round(y1)}`;
}

function verticalLineDrawOp(x, y0, y1, width) {
  const half = Math.max(0.5, width / 2);
  return rectangleDrawOp(x - half, y0, x + half, y1);
}

function horizontalLineDrawOp(y, x0, x1, width) {
  const half = Math.max(0.5, width / 2);
  return rectangleDrawOp(x0, y - half, x1, y + half);
}

function ensureDirs() {
  fs.mkdirSync(assetDir, { recursive: true });
  fs.mkdirSync(generatedDir, { recursive: true });
  fs.mkdirSync(metadataDir, { recursive: true });
}

function positiveInteger(value, fallback) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`expected positive integer, got ${value}`);
  }
  return parsed;
}

function nonNegativeInteger(value, fallback) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`expected non-negative integer, got ${value}`);
  }
  return parsed;
}

function percentArg(value, fallback) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 400) {
    throw new Error(`expected percentage in (0, 400], got ${value}`);
  }
  return parsed;
}

function morphologyArg(value, fallback) {
  if (value == null || value === true) return fallback;
  const out = String(value).trim();
  if (!out || out === "none") return "";
  if (!/^[A-Za-z]+:[0-9]+$/.test(out)) {
    throw new Error(`invalid morphology ${JSON.stringify(value)}; expected Kernel:N such as Square:5, or none`);
  }
  return out;
}

function parseListArg(value) {
  if (value == null || value === true) return [];
  return String(value)
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function safeVersionArg(value) {
  if (value == null || value === true) return "";
  const version = String(value).trim();
  if (!version) return "";
  if (!/^[A-Za-z0-9._-]{1,80}$/.test(version)) {
    throw new Error(`invalid image version ${JSON.stringify(value)}; use letters, numbers, dot, underscore, or dash`);
  }
  return version;
}

function parseArgs(args) {
  const out = {};
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = args[i + 1];
    if (!next || next.startsWith("--")) {
      out[key] = true;
    } else {
      out[key] = next;
      i += 1;
    }
  }
  return out;
}

function run(cmd, args, options = {}) {
  const result = spawnSync(cmd, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
  });
  if (result.status !== 0 && !options.allowFailure) {
    throw new Error(`${cmd} ${args.join(" ")} failed\n${result.stderr || ""}`);
  }
  return result;
}

function rel(file) {
  return path.relative(repoRoot, file);
}

function usage() {
  console.error(`Usage:
  node scripts/art/tank-raster-pipeline.mjs make-sheet [--scale 3] [--columns 2] [--layout tight] [--profile semantic] [--key #ff00ff]
  node scripts/art/tank-raster-pipeline.mjs write-atlas --sheet <png> [--columns 2] [--layout tight] [--profile semantic] [--transparent-key #ff00ff] [--disabled] [--blank-cells cell[,cell]] [--normalize-visible-bounds] [--ignore-guide-bounds] [--guide-mask-fuzz 12] [--guide-mask-alpha-threshold 20] [--guide-mask-line-width 12] [--guide-mask-morphology Square:5] [--clear-cell-edge-alpha 16] [--visible-padding 2] [--brightness 120] [--saturation 100] [--hue 100] [--image-version pass-id] [--prompt-file metadata/prompt.md]
  node scripts/art/tank-raster-pipeline.mjs write-prompt`);
}

main();
