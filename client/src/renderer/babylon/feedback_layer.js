import { worldPointToScene } from "./coordinates.js";

const MAX_FEEDBACK_MESHES = 256;

export class BabylonFeedbackLayer {
  constructor(scene, parent, { Babylon }) {
    this._Babylon = Babylon;
    this._scene = scene;
    this._parent = parent;
    this._meshes = [];
    this._marquee = document.createElement("div");
    this._marquee.className = "rts-babylon-marquee";
    this._marquee.hidden = true;
    parent.appendChild(this._marquee);
    this._diagnostics = { worldMarkers: 0, marquee: false };
  }

  sync(frame) {
    this._clearMeshes();
    const tactical = frame?.layers?.tacticalFeedback || [];
    const entities = frame?.layers?.fogGatedWorld || [];
    const mapBounds = frame?.projection?.mapBounds;
    const tileSizePx = frame?.visible?.width > 0 && mapBounds
      ? (mapBounds.maxX - mapBounds.minX) / frame.visible.width
      : 32;

    for (const record of tactical) {
      if (this._meshes.length >= MAX_FEEDBACK_MESHES) break;
      if (record?.type === "command" && finitePoint(record)) {
        this._ring(record.x, record.y, record.kind === "attack" ? "#ff6b5f" : "#73ddff", 13);
      } else if (record?.type === "attackTargetPreview" && finitePoint(record)) {
        this._ring(record.x, record.y, "#ff6b5f", 18);
      } else if (record?.type === "placement") {
        this._placement(record, tileSizePx);
      }
    }
    for (const entity of entities) {
      if (this._meshes.length >= MAX_FEEDBACK_MESHES || entity?.type !== "entity" || !entity.selected) continue;
      let from = { x: entity.x, y: entity.y };
      for (const marker of entity.orderPlan || []) {
        if (!finitePoint(marker) || this._meshes.length >= MAX_FEEDBACK_MESHES) continue;
        this._line([from, marker], marker.kind === "attack" ? "#ff6b5f" : "#73ddff");
        this._ring(marker.x, marker.y, marker.kind === "attack" ? "#ff6b5f" : "#73ddff", 10);
        from = marker;
      }
    }

    const marquee = (frame?.layers?.screenOverlay || []).find((record) => record?.type === "marquee")?.rect;
    this._syncMarquee(marquee);
    this._diagnostics = { worldMarkers: this._meshes.length, marquee: !this._marquee.hidden };
  }

  diagnostics() {
    return Object.freeze({ ...this._diagnostics });
  }

  destroy() {
    this._clearMeshes();
    this._marquee?.remove();
    this._marquee = null;
  }

  _placement(record, fallbackTileSizePx) {
    const tileSizePx = positive(record?.footprint?.tileSizePx, fallbackTileSizePx);
    const footW = positive(record?.footprint?.footW, 1);
    const footH = positive(record?.footprint?.footH, 1);
    const sites = Array.isArray(record.lineSites) && record.lineSites.length > 0
      ? record.lineSites
      : [record];
    for (const site of sites) {
      if (!Number.isFinite(site?.tileX) || !Number.isFinite(site?.tileY)) continue;
      const x0 = site.tileX * tileSizePx;
      const y0 = site.tileY * tileSizePx;
      const x1 = x0 + footW * tileSizePx;
      const y1 = y0 + footH * tileSizePx;
      this._line([
        { x: x0, y: y0 }, { x: x1, y: y0 }, { x: x1, y: y1 }, { x: x0, y: y1 }, { x: x0, y: y0 },
      ], (site.valid ?? record.valid) ? "#71e69a" : "#ff665c");
    }
  }

  _ring(x, y, color, radiusPx) {
    const points = [];
    for (let index = 0; index <= 24; index += 1) {
      const angle = index / 24 * Math.PI * 2;
      points.push({ x: x + Math.cos(angle) * radiusPx, y: y + Math.sin(angle) * radiusPx });
    }
    this._line(points, color);
  }

  _line(points, color) {
    if (this._meshes.length >= MAX_FEEDBACK_MESHES) return;
    const B = this._Babylon;
    const vectors = points.map((point) => {
      const scene = worldPointToScene({ x: point.x, y: point.y, heightPx: 2 });
      return new B.Vector3(scene.x, scene.y, scene.z);
    });
    if (vectors.length < 2) return;
    const mesh = B.MeshBuilder.CreateLines(`feedback-${this._meshes.length}`, { points: vectors }, this._scene);
    mesh.color = B.Color3.FromHexString(color);
    mesh.alpha = 0.95;
    mesh.isPickable = false;
    mesh.renderingGroupId = 3;
    this._meshes.push(mesh);
  }

  _syncMarquee(rect) {
    const normalized = normalizeRect(rect);
    if (!normalized) {
      this._marquee.hidden = true;
      return;
    }
    this._marquee.hidden = false;
    this._marquee.style.left = `${normalized.x}px`;
    this._marquee.style.top = `${normalized.y}px`;
    this._marquee.style.width = `${normalized.width}px`;
    this._marquee.style.height = `${normalized.height}px`;
  }

  _clearMeshes() {
    for (const mesh of this._meshes) mesh.dispose();
    this._meshes.length = 0;
  }
}

function normalizeRect(rect) {
  if (!rect) return null;
  const x0 = Number.isFinite(rect.x0) ? rect.x0 : rect.x;
  const y0 = Number.isFinite(rect.y0) ? rect.y0 : rect.y;
  const x1 = Number.isFinite(rect.x1) ? rect.x1 : x0 + (rect.width ?? rect.w);
  const y1 = Number.isFinite(rect.y1) ? rect.y1 : y0 + (rect.height ?? rect.h);
  if (![x0, y0, x1, y1].every(Number.isFinite)) return null;
  return { x: Math.min(x0, x1), y: Math.min(y0, y1), width: Math.abs(x1 - x0), height: Math.abs(y1 - y0) };
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y);
}

function positive(value, fallback) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : fallback;
}
