import { COLORS } from "./config.js";
const hex = value => `#${value.toString(16).padStart(6, "0")}`;

export function renderMinimap(frameViews = null, { profiler = null, capturePresentation = false } = {}) {
  this._profiler = profiler || null;
  const browserWindow = this.canvas.ownerDocument?.defaultView;
  if (!capturePresentation && browserWindow) {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = Math.max(2, Number(browserWindow.devicePixelRatio) || 1);
    const pixels = Math.ceil(rect.width * dpr);
    if (pixels > 0 && this.canvas.width !== pixels) {
      this.canvas.width = pixels;
      this.canvas.height = pixels;
    }
  }
  this._syncCanvasSize();
  this._gpuBridge?.begin(this, capturePresentation);
  try {
    const ctx = this.ctx;
    if (!ctx) return;
    // Void background (outside the map / before a map exists).
    ctx.fillStyle = hex(COLORS.bgVoid);
    ctx.fillRect(0, 0, this.size, this.size);

    if (!this._ensureTransform()) return;
    const entities = this._minimapEntities(frameViews);
    const now = performance.now();
    if (!this._gpuBridge?.active) this._unitIcons.beginFrame(entities, this.state, this.state?.visualNow?.() ?? now);
    const attackFlashIds = this._underAttackFlashEntityIds(entities, now);
    this._drawTerrainLayer();
    this._forestLayer.draw({
      ctx: this.ctx, map: this._renderMap(), size: this.size,
      scale: this._scale, offX: this._offX, offY: this._offY,
      presentationScale: this._presentationScale(),
      presentation: this._canvasPresentationSignature(),
    });
    this._drawEntities(entities, { deferForegroundPlayer: true, attackFlashIds });
    this._drawFog();
    this._roadMarkingLayer.draw({
      ctx: this.ctx, map: this._renderMap(), size: this.size,
      scale: this._scale, offX: this._offX, offY: this._offY,
      presentation: this._canvasPresentationSignature() });
    this._drawResourceLayer();
    this._drawPlayerOwnedEntityOutline(entities);
    this._drawEntities(entities, { foregroundPlayerOnly: true, attackFlashIds });
    this._drawEntities(entities.filter((entity) => entity.aboveFogReveal), { attackFlashIds });
    if (!capturePresentation) this._drawArtilleryFiringMarkers(now);
    if (!capturePresentation) this._drawViewport();
    if (!capturePresentation) this._drawPings(now);
    if (!capturePresentation) this._formation.draw();
  } finally {
    this._gpuBridge?.end(this, this.state?.visualNow?.() ?? performance.now());
  }
}
