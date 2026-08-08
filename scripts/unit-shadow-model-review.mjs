#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";
import {
  PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES,
  PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET,
} from "../client/src/renderer/projected_unit_shadow_model_candidates.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDir = path.join(root, "target", "unit-shadow-model-review");
const chrome = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const views = ["isometric", "top", "front", "side"];

validateCandidates();
await fs.mkdir(outputDir, { recursive: true });

const browser = await puppeteer.launch({
  executablePath: chrome,
  headless: true,
  args: ["--disable-gpu-sandbox", "--no-sandbox"],
});

try {
  const page = await browser.newPage();
  await page.setViewport({ width: 3000, height: 500, deviceScaleFactor: 1 });
  for (const view of views) {
    await page.setContent(renderDocument(view), { waitUntil: "load" });
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({
      path: path.join(outputDir, `unit-shadow-models-${view}.png`),
      type: "png",
      fullPage: false,
    });
  }
} finally {
  await browser.close();
}

await fs.writeFile(
  path.join(outputDir, "manifest.json"),
  `${JSON.stringify({
    generatedAt: new Date().toISOString(),
    views,
    units: PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES.map((entry) => ({
      kind: entry.kind,
      spriteEnvelope: entry.spriteEnvelope,
      candidates: entry.candidates.map((candidate) => ({
        id: candidate.id,
        shapes: candidate.parts.length,
        rationale: candidate.rationale,
      })),
    })),
  }, null, 2)}\n`,
);

console.log(outputDir);

function validateCandidates() {
  if (PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES.length !== 5) {
    throw new Error("The review sheet must contain the five requested unit kinds.");
  }
  const kinds = new Set();
  for (const entry of PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES) {
    if (["scout_plane", "ekat", "worker", "golem"].includes(entry.kind)) {
      throw new Error(`${entry.kind} is deliberately excluded.`);
    }
    if (kinds.has(entry.kind)) throw new Error(`Duplicate unit kind ${entry.kind}.`);
    kinds.add(entry.kind);
    if (entry.candidates.length !== 2) throw new Error(`${entry.kind} must have exactly two candidates.`);
    for (const candidate of entry.candidates) {
      if (candidate.parts.length > PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET) {
        throw new Error(`${candidate.id} exceeds the ${PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET}-shape budget.`);
      }
      for (const part of candidate.parts) {
        if (part.size.length !== 3 || part.center.length !== 3 || part.size.some((value) => !(value > 0))) {
          throw new Error(`${candidate.id}/${part.id} has invalid box geometry.`);
        }
      }
    }
  }
}

function renderDocument(view) {
  const cards = PROJECTED_UNIT_SHADOW_MODEL_CANDIDATES.flatMap((entry) =>
    entry.candidates.map((candidate, index) => renderCard(entry, candidate, index, view)),
  ).join("");
  const viewLabel = view === "isometric" ? "Orthographic isometric" : `${capitalize(view)} orthographic`;
  return `<!doctype html>
  <html><head><meta charset="utf-8"><style>
    * { box-sizing: border-box; }
    html, body { width: 3000px; height: 500px; margin: 0; overflow: hidden; }
    body { font-family: Inter, ui-sans-serif, system-ui, sans-serif; color: #23251f; background: #d8d3c2; }
    header { height: 88px; padding: 17px 28px 12px; border-bottom: 2px solid #777464; background: #ece8d9; display:flex; align-items:flex-start; justify-content:space-between; }
    h1 { margin: 0; font-size: 27px; letter-spacing: .055em; text-transform: uppercase; }
    .subtitle { margin-top: 5px; color: #626354; font-size: 15px; }
    .legend { margin-top: 5px; font-size: 14px; color:#4f5147; text-align:right; line-height:1.45; }
    main { display:grid; grid-template-columns: repeat(10, 300px); grid-template-rows: 412px; }
    article { position:relative; height:412px; border-right:1px solid #a39f8e; border-bottom:1px solid #a39f8e; background:rgba(248,246,237,.66); overflow:hidden; }
    article.variant-b { background:rgba(231,235,227,.73); }
    .unit { position:absolute; top:15px; left:16px; right:16px; font-weight:750; font-size:17px; letter-spacing:.035em; text-transform:uppercase; }
    .candidate { position:absolute; top:43px; left:16px; font-size:14px; color:#596052; }
    .count { position:absolute; top:15px; right:15px; border:1px solid #74776b; border-radius:12px; padding:3px 8px; font:700 12px/1 system-ui; color:#4e5149; }
    svg { position:absolute; left:6px; top:70px; width:288px; height:252px; }
    .rationale { position:absolute; left:16px; right:16px; bottom:14px; min-height:50px; font-size:12.5px; line-height:1.35; color:#5c5c51; }
    .axis { stroke:#8b8c7f; stroke-width:.55; opacity:.55; }
    .ground { fill:#cbc7b6; opacity:.7; }
    .envelope { fill:#6f7467; fill-opacity:.16; stroke:#676b61; stroke-width:1; stroke-dasharray:4 3; }
    .face { stroke:#30352f; stroke-width:.75; stroke-linejoin:round; }
    .part-line { fill:none; stroke:#f2f0e8; stroke-opacity:.35; stroke-width:.55; }
  </style></head><body>
    <header><div><h1>Unit shadow-model candidates · ${viewLabel}</h1>
      <div class="subtitle">Two candidates per requested unit · forward axis points right</div></div>
      <div class="legend">Every component is one rectangular prism · hard limit ${PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET} shapes/unit<br>${view === "top" ? "Dashed gray envelope = approximate current sprite footprint target" : "All models share one world-pixel scale for direct comparison"}</div>
    </header><main>${cards}</main>
  </body></html>`;
}

function renderCard(entry, candidate, variantIndex, view) {
  const projected = projectCandidate(candidate, view);
  const envelope = view === "top" ? renderEnvelope(entry.spriteEnvelope, projected.transform) : "";
  return `<article class="variant-${variantIndex === 0 ? "a" : "b"}">
    <div class="unit">${escapeHtml(entry.label)}</div>
    <div class="candidate">${escapeHtml(candidate.label)}</div>
    <div class="count">${candidate.parts.length}/${PROJECTED_UNIT_SHADOW_MODEL_SHAPE_BUDGET}</div>
    <svg viewBox="0 0 288 252" aria-label="${escapeHtml(candidate.label)} ${view} view">
      ${renderGround(view)}${renderAxes(view)}${envelope}${projected.faces}
    </svg>
    <div class="rationale">${escapeHtml(candidate.rationale)}</div>
  </article>`;
}

function projectCandidate(candidate, view) {
  const camera = cameraFor(view);
  const allPoints = candidate.parts.flatMap((part) => boxCorners(part).map((point) => project(point, camera)));
  const bounds = projectedBounds(allPoints);
  const maxW = view === "isometric" ? 218 : 224;
  const maxH = view === "isometric" ? 180 : 176;
  const scale = Math.min(3.15, maxW / Math.max(1, bounds.width), maxH / Math.max(1, bounds.height));
  const transform = {
    scale,
    x: 144 - ((bounds.minX + bounds.maxX) * 0.5) * scale,
    y: 141 - ((bounds.minY + bounds.maxY) * 0.5) * scale,
    camera,
  };
  const faces = candidate.parts.flatMap((part, partIndex) => boxFaces(part).map((face) => ({
    ...face,
    partIndex,
    depth: face.points.reduce((sum, point) => sum + dot(point, camera.w), 0) / face.points.length,
  }))).sort((a, b) => a.depth - b.depth || a.partIndex - b.partIndex);
  return {
    transform,
    faces: faces.map((face) => {
      const points = face.points.map((point) => screenPoint(point, transform)).map(([x, y]) => `${round(x)},${round(y)}`).join(" ");
      const light = 44 + Math.max(0, dot(face.normal, normalize([-.5, -.65, 1]))) * 23;
      const hue = face.partIndex % 2 === 0 ? 203 : 195;
      return `<polygon class="face" points="${points}" fill="hsl(${hue} 38% ${round(light)}%)"/>`;
    }).join(""),
  };
}

function renderEnvelope(envelope, transform) {
  const x0 = -envelope.length / 2;
  const x1 = envelope.length / 2;
  const y0 = -envelope.width / 2;
  const y1 = envelope.width / 2;
  const points = [[x0, y0, 0], [x1, y0, 0], [x1, y1, 0], [x0, y1, 0]]
    .map((point) => screenPoint(point, transform))
    .map(([x, y]) => `${round(x)},${round(y)}`).join(" ");
  return `<polygon class="envelope" points="${points}"/>`;
}

function renderGround(view) {
  if (view !== "isometric") return "";
  return `<polygon class="ground" points="38,192 144,224 250,192 144,160"/>`;
}

function renderAxes(view) {
  if (view !== "top" && view !== "isometric") return "";
  return `<line class="axis" x1="30" y1="226" x2="77" y2="226"/><path class="axis" d="M77 226l-7-4v8z"/><text x="31" y="244" font-size="9" fill="#77796f">forward</text>`;
}

function cameraFor(view) {
  if (view === "top") return { u: [1, 0, 0], v: [0, 1, 0], w: [0, 0, 1] };
  if (view === "front") return { u: [0, 1, 0], v: [0, 0, 1], w: [1, 0, 0] };
  if (view === "side") return { u: [1, 0, 0], v: [0, 0, 1], w: [0, -1, 0] };
  const u = normalize([1, 1, 0]);
  const w = normalize([1, -1, .8]);
  return { u, v: normalize(cross(w, u)), w };
}

function boxCorners(part) {
  const [cx, cy, cz] = part.center;
  const [lx, ly, lz] = part.size;
  const result = [];
  for (const dx of [-lx / 2, lx / 2]) for (const dy of [-ly / 2, ly / 2]) for (const dz of [-lz / 2, lz / 2]) {
    const pitchCos = Math.cos(part.pitch || 0);
    const pitchSin = Math.sin(part.pitch || 0);
    const pitchedX = dx * pitchCos - dz * pitchSin;
    const pitchedZ = dx * pitchSin + dz * pitchCos;
    const cos = Math.cos(part.yaw);
    const sin = Math.sin(part.yaw);
    result.push([cx + pitchedX * cos - dy * sin, cy + pitchedX * sin + dy * cos, cz + pitchedZ]);
  }
  return result;
}

function boxFaces(part) {
  const corners = boxCorners(part);
  const indices = [
    [[0, 2, 6, 4], [0, 0, -1]], [[1, 5, 7, 3], [0, 0, 1]],
    [[0, 4, 5, 1], [0, -1, 0]], [[2, 3, 7, 6], [0, 1, 0]],
    [[0, 1, 3, 2], [-1, 0, 0]], [[4, 6, 7, 5], [1, 0, 0]],
  ];
  const cos = Math.cos(part.yaw);
  const sin = Math.sin(part.yaw);
  const pitchCos = Math.cos(part.pitch || 0);
  const pitchSin = Math.sin(part.pitch || 0);
  return indices.map(([face, normal]) => ({
    points: face.map((index) => corners[index]),
    normal: [
      (normal[0] * pitchCos - normal[2] * pitchSin) * cos - normal[1] * sin,
      (normal[0] * pitchCos - normal[2] * pitchSin) * sin + normal[1] * cos,
      normal[0] * pitchSin + normal[2] * pitchCos,
    ],
  }));
}

function project(point, camera) {
  return [dot(point, camera.u), -dot(point, camera.v)];
}

function screenPoint(point, transform) {
  const [x, y] = project(point, transform.camera);
  return [transform.x + x * transform.scale, transform.y + y * transform.scale];
}

function projectedBounds(points) {
  const xs = points.map(([x]) => x);
  const ys = points.map(([, y]) => y);
  const minX = Math.min(...xs); const maxX = Math.max(...xs);
  const minY = Math.min(...ys); const maxY = Math.max(...ys);
  return { minX, maxX, minY, maxY, width: maxX - minX, height: maxY - minY };
}

function normalize(vector) {
  const length = Math.hypot(...vector);
  return vector.map((value) => value / length);
}

function cross(a, b) {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

function dot(a, b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
function round(value) { return Math.round(value * 100) / 100; }
function capitalize(value) { return value[0].toUpperCase() + value.slice(1); }
function escapeHtml(value) { return String(value).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;"); }
