import { hash2, terrainColor, terrainOverlayColor } from "./src/renderer/shared.js";

const canvas = document.getElementById("unit-lab-canvas");
const ctx = canvas.getContext("2d");
const promptEl = document.getElementById("unit-lab-prompt");
const baseEl = document.getElementById("unit-lab-base");
const generateEl = document.getElementById("unit-lab-generate");
const statusEl = document.getElementById("unit-lab-status");
const attemptsEl = document.getElementById("unit-lab-attempts");
const nameEl = document.getElementById("unit-lab-name");
const roleEl = document.getElementById("unit-lab-role");
const sourceEl = document.getElementById("unit-lab-source");
const notesEl = document.getElementById("unit-lab-notes");

let attempts = [];
let selected = null;
let startedAt = performance.now();

generateEl.addEventListener("click", () => {
  generateAttempt();
});

promptEl.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    event.preventDefault();
    generateAttempt();
  }
});

window.addEventListener("resize", draw);

loadAttempts();
requestAnimationFrame(tick);

async function loadAttempts() {
  setStatus("Loading versions");
  try {
    const res = await fetch("/api/unit-designs");
    if (!res.ok) throw new Error(await res.text());
    const payload = await res.json();
    attempts = payload.attempts || [];
    selected = attempts[0] || null;
    renderHistory();
    renderDetails();
    setStatus(selected ? "Loaded latest version" : "Ready");
  } catch (err) {
    setStatus(err.message || "Failed to load versions", true);
  }
  draw();
}

async function generateAttempt() {
  const prompt = promptEl.value.trim();
  if (!prompt) {
    setStatus("Prompt is required", true);
    return;
  }
  generateEl.disabled = true;
  setStatus("Generating");
  try {
    const res = await fetch("/api/unit-designs", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ prompt, baseKind: baseEl.value }),
    });
    if (!res.ok) throw new Error(await res.text());
    const payload = await res.json();
    selected = payload.attempt;
    attempts = [selected, ...attempts.filter((attempt) => attempt.id !== selected.id)];
    renderHistory();
    renderDetails();
    setStatus(payload.warning || "Generated version");
  } catch (err) {
    setStatus(err.message || "Generation failed", true);
  } finally {
    generateEl.disabled = false;
  }
}

function renderHistory() {
  attemptsEl.replaceChildren();
  for (const attempt of attempts) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `unit-lab-attempt${attempt.id === selected?.id ? " is-selected" : ""}`;
    button.innerHTML = `<strong>${escapeHtml(attempt.spec?.name || attempt.id)}</strong><span>${escapeHtml(attempt.id)}</span>`;
    button.addEventListener("click", () => {
      selected = attempt;
      renderHistory();
      renderDetails();
      draw();
    });
    attemptsEl.append(button);
  }
}

function renderDetails() {
  const spec = selected?.spec;
  nameEl.textContent = spec?.name || "No attempt selected";
  roleEl.textContent = spec?.role || "";
  sourceEl.textContent = selected
    ? `${selected.source}${selected.model ? ` / ${selected.model}` : ""} / ${selected.createdAt}`
    : "";
  notesEl.replaceChildren();
  for (const note of spec?.animationNotes || []) {
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
  if (selected) {
    drawUnit(selected.spec, cssW * 0.5, cssH * 0.48, 2.15, now);
    drawUnit(selected.spec, cssW * 0.22, cssH * 0.32, 1.0, now);
    drawUnit(selected.spec, cssW * 0.78, cssH * 0.66, 0.62, now);
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
