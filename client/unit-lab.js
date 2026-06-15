import { hash2, terrainColor, terrainOverlayColor } from "./src/renderer/shared.js";

const canvas = document.getElementById("unit-lab-canvas");
const ctx = canvas.getContext("2d");
const refreshEl = document.getElementById("unit-lab-refresh");
const statusEl = document.getElementById("unit-lab-status");
const treeEl = document.getElementById("unit-lab-tree");
const nameEl = document.getElementById("unit-lab-name");
const roleEl = document.getElementById("unit-lab-role");
const sourceEl = document.getElementById("unit-lab-source");
const promptEl = document.getElementById("unit-lab-prompt");
const notesEl = document.getElementById("unit-lab-notes");

let files = new Map();
let catalogTree = [];
let selected = null;
let startedAt = performance.now();

refreshEl.addEventListener("click", () => {
  loadCatalog();
});
window.addEventListener("resize", draw);

loadCatalog();
requestAnimationFrame(tick);

async function loadCatalog() {
  setStatus("Loading generation files");
  try {
    const res = await fetch("/api/unit-designs");
    if (!res.ok) throw new Error(await res.text());
    const payload = await res.json();
    files = new Map((payload.attempts || []).map((file) => [file.path, file]));
    catalogTree = payload.tree || [];
    selected = files.get(selected?.path) || payload.attempts?.[0] || null;
    renderTree(catalogTree);
    renderDetails();
    setStatus(selected ? `${files.size} generation file${files.size === 1 ? "" : "s"}` : `No JSON files in ${payload.root}`);
  } catch (err) {
    selected = null;
    catalogTree = [];
    renderTree([]);
    renderDetails();
    setStatus(err.message || "Failed to load generation files", true);
  }
  draw();
}

function renderTree(tree) {
  treeEl.replaceChildren();
  if (tree.length === 0) {
    const empty = document.createElement("p");
    empty.className = "unit-lab-empty";
    empty.textContent = "No generation files found.";
    treeEl.append(empty);
    return;
  }
  treeEl.append(renderNodeList(tree, 0));
}

function renderNodeList(nodes, depth) {
  const list = document.createElement("div");
  list.className = "unit-lab-tree-list";
  list.style.setProperty("--depth", depth);
  for (const node of nodes) {
    list.append(renderNode(node, depth));
  }
  return list;
}

function renderNode(node, depth) {
  if (node.kind === "directory") {
    const details = document.createElement("details");
    details.className = "unit-lab-tree-dir";
    details.open = true;
    const summary = document.createElement("summary");
    summary.textContent = node.name;
    details.append(summary);
    details.append(renderNodeList(node.children || [], depth + 1));
    return details;
  }

  const file = files.get(node.path);
  const button = document.createElement("button");
  button.type = "button";
  button.className = `unit-lab-file${node.path === selected?.path ? " is-selected" : ""}`;
  button.style.setProperty("--depth", depth);
  button.innerHTML = `<strong>${escapeHtml(file?.attempt?.spec?.name || node.name)}</strong><span>${escapeHtml(node.path)}</span>`;
  button.addEventListener("click", () => {
    selected = file || null;
    renderTree(catalogTree);
    renderDetails();
    draw();
  });
  return button;
}

function renderDetails() {
  const attempt = selected?.attempt;
  const spec = attempt?.spec;
  nameEl.textContent = spec?.name || "No generation selected";
  roleEl.textContent = spec?.role || spec?.silhouette || "";
  sourceEl.textContent = selected
    ? [selected.path, attempt?.source, attempt?.model, attempt?.createdAt || attempt?.created_at]
      .filter(Boolean)
      .join(" / ")
    : "";
  promptEl.textContent = attempt?.prompt ? `Prompt: ${attempt.prompt}` : "";
  notesEl.replaceChildren();
  for (const note of spec?.animationNotes || spec?.animation_notes || []) {
    const li = document.createElement("li");
    li.textContent = note;
    notesEl.append(li);
  }
}

function tick(now) {
  draw(now);
  requestAnimationFrame(tick);
}

function draw(now = performance.now()) {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const w = Math.max(640, Math.floor(rect.width * dpr));
  const h = Math.max(480, Math.floor(rect.height * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const cssW = w / dpr;
  const cssH = h / dpr;
  drawGrass(cssW, cssH);
  drawRangeGrid(cssW, cssH);
  if (selected?.attempt?.spec) {
    drawUnit(selected.attempt.spec, cssW * 0.5, cssH * 0.48, 2.15, now);
    drawUnit(selected.attempt.spec, cssW * 0.22, cssH * 0.32, 1.0, now);
    drawUnit(selected.attempt.spec, cssW * 0.78, cssH * 0.66, 0.62, now);
  }
}

function drawGrass(w, h) {
  const tile = 32;
  for (let y = 0; y < h; y += tile) {
    for (let x = 0; x < w; x += tile) {
      const tx = Math.floor(x / tile);
      const ty = Math.floor(y / tile);
      const code = hash2(tx, ty) > 0.86 ? 3 : 0;
      ctx.fillStyle = colorCss(terrainColor(code, tx, ty));
      ctx.fillRect(x, y, tile, tile);
      for (let by = 0; by < 4; by++) {
        for (let bx = 0; bx < 4; bx++) {
          const n = hash2(tx * 17 + bx, ty * 17 + by);
          if (n < 0.42) continue;
          ctx.fillStyle = colorCss(terrainOverlayColor(code, n), 0.16);
          ctx.fillRect(x + bx * 8, y + by * 8, 8, 8);
        }
      }
    }
  }
}

function drawRangeGrid(w, h) {
  ctx.save();
  ctx.strokeStyle = "rgba(231, 223, 197, 0.08)";
  ctx.lineWidth = 1;
  for (let x = 0; x < w; x += 96) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, h);
    ctx.stroke();
  }
  for (let y = 0; y < h; y += 96) {
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
    ctx.stroke();
  }
  ctx.restore();
}

function drawUnit(spec, x, y, scale, now) {
  const pulse = Math.sin((now - startedAt) / 260) * 0.8;
  const shapes = [...(spec.shapes || [])].sort((a, b) => (a.layer || 0) - (b.layer || 0));
  ctx.save();
  ctx.translate(x, y + pulse);
  ctx.scale(scale, scale);
  for (const shape of shapes) {
    drawShape(shape);
  }
  ctx.restore();
}

function drawShape(shape) {
  ctx.save();
  ctx.globalAlpha = clamp(Number(shape.alpha ?? 1), 0.12, 1);
  ctx.translate(Number(shape.x || 0), Number(shape.y || 0));
  ctx.rotate(Number(shape.rotation || 0));
  ctx.fillStyle = shape.color || "#697256";
  ctx.strokeStyle = "rgba(14, 14, 12, 0.9)";
  ctx.lineWidth = 2;
  const w = Number(shape.w || 10);
  const h = Number(shape.h || 10);
  if (shape.kind === "ellipse") {
    ctx.beginPath();
    ctx.ellipse(0, 0, w * 0.5, h * 0.5, 0, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
  } else if (shape.kind === "triangle") {
    ctx.beginPath();
    ctx.moveTo(w * 0.5, 0);
    ctx.lineTo(-w * 0.5, -h * 0.5);
    ctx.lineTo(-w * 0.5, h * 0.5);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
  } else {
    const radius = shape.kind === "barrel" ? Math.min(3, h * 0.5) : 2;
    roundRect(-w * 0.5, -h * 0.5, w, h, radius);
    ctx.fill();
    ctx.stroke();
    if (shape.kind === "track") {
      ctx.strokeStyle = "rgba(231, 223, 197, 0.22)";
      ctx.lineWidth = 1;
      for (let x = -w * 0.38; x < w * 0.42; x += 8) {
        ctx.beginPath();
        ctx.moveTo(x, -h * 0.35);
        ctx.lineTo(x + 3, h * 0.35);
        ctx.stroke();
      }
    }
  }
  ctx.restore();
}

function roundRect(x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + r);
  ctx.lineTo(x + w, y + h - r);
  ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  ctx.lineTo(x + r, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

function colorCss(color, alpha = 1) {
  const r = (color >> 16) & 0xff;
  const g = (color >> 8) & 0xff;
  const b = color & 0xff;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function setStatus(message, error = false) {
  statusEl.textContent = message;
  statusEl.classList.toggle("is-error", error);
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (ch) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  })[ch]);
}
