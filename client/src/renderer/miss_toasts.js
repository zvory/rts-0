import { clamp01, finiteNumber, rendererVisualNow, smoothstep01 } from "./shared.js";

const MISS_TOAST_TEXT = "Miss!";
const MISS_TOAST_TTL_MS = 760;
const MISS_TOAST_FONT_SIZE = 6.75;
const MISS_TOAST_STROKE_THICKNESS = 1.5;
const MISS_TOAST_OFFSET_X = 2;
const MISS_TOAST_OFFSET_Y = 3;
const MISS_TOAST_FLOAT_Y = 5;

export function _drawMissToasts(state) {
  if (!state || typeof state.liveMissToasts !== "function") return;
  if (!this._missToastPool) this._missToastPool = new Map();
  const now = rendererVisualNow(this);
  const toasts = state.liveMissToasts(now);
  const live = new Set();

  for (const toast of toasts) {
    const key = toastKey(toast);
    if (key == null) continue;
    const target = state.entityById?.(toast.to);
    if (!target || !finiteNumber(target.x) || !finiteNumber(target.y)) continue;
    live.add(key);

    let label = this._missToastPool.get(key);
    if (!label) {
      label = new PIXI.Text(MISS_TOAST_TEXT, {
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
        fontSize: MISS_TOAST_FONT_SIZE,
        fill: 0xfff1a8,
        align: "center",
        fontWeight: "700",
        stroke: 0x20160c,
        strokeThickness: MISS_TOAST_STROKE_THICKNESS,
      });
      label.anchor.set(0, 0.5);
      this._missToastPool.set(key, label);
      this.layers.feedback.addChild(label);
      this._recordRenderDiagnostic?.("renderer.pixi.displayObject.created.missToastText");
    } else {
      this._recordRenderDiagnostic?.("renderer.pixi.displayObject.reused.missToastText");
    }

    const age = Math.max(0, now - toast.createdAt);
    const t = clamp01(age / MISS_TOAST_TTL_MS);
    const fade = 1 - smoothstep01(t);
    const ring = typeof this._ringRadius === "function"
      ? this._ringRadius(target)
      : { rx: 12, ry: 9, cy: 3 };
    const rx = finiteNumber(ring?.rx) ? ring.rx : 12;
    const ry = finiteNumber(ring?.ry) ? ring.ry : 9;
    const cy = finiteNumber(ring?.cy) ? ring.cy : 3;

    label.text = MISS_TOAST_TEXT;
    label.visible = true;
    label.alpha = 0.95 * fade;
    label.scale.set(1);
    label.position.set(
      target.x + rx + MISS_TOAST_OFFSET_X,
      target.y + cy - ry - MISS_TOAST_OFFSET_Y - t * MISS_TOAST_FLOAT_Y,
    );
  }

  for (const [key, label] of this._missToastPool) {
    if (live.has(key)) continue;
    label.parent?.removeChild?.(label);
    label.destroy();
    this._missToastPool.delete(key);
  }
}

function toastKey(toast) {
  if (Number.isFinite(toast?.id)) return toast.id;
  if (Number.isFinite(toast?.to) && Number.isFinite(toast?.createdAt)) {
    return `${toast.to}:${toast.createdAt}`;
  }
  return null;
}
