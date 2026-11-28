import { applyColorAdjustmentToRgba } from "./color_adjustment.js";
import { createWorkerSafeCanvas, fetchImageBitmap } from "../raster_primitives.js";
import { hexToInt } from "../shared.js";

const PALETTES = new WeakMap();

/** Composite a fixed-color layer and a shaded paint layer with ordinary source-over alpha. */
export function composeTeamPaintRgba(base, paint, teamColor, output = new Uint8ClampedArray(base.length)) {
  if (base.length !== paint.length || output.length !== base.length || base.length % 4) {
    throw new Error("Team paint layers must have identical RGBA dimensions");
  }
  const color = hexToInt(teamColor);
  const tint = [((color >> 16) & 255) / 255, ((color >> 8) & 255) / 255, (color & 255) / 255];
  for (let i = 0; i < base.length; i += 4) {
    const p = paint[i + 3] / 255, b = base[i + 3] / 255 * (1 - p), alpha = p + b;
    for (let channel = 0; channel < 3; channel += 1) {
      output[i + channel] = alpha ? Math.round((paint[i + channel] * tint[channel] * p + base[i + channel] * b) / alpha) : 0;
    }
    output[i + 3] = Math.round(alpha * 255);
  }
  return output;
}

/** Decode once; cache derived palettes per match so ordinary sprites remain batchable. */
export async function loadTeamPaintTexture(pixi, strip, adjustment) {
  const bitmap = await fetchImageBitmap(strip.image);
  try {
    const width = strip.frameWidth * strip.frameCount, height = strip.frameHeight;
    if (bitmap.width !== width || bitmap.height !== height * strip.sheetRows) {
      throw new Error(`Invalid team-paint atlas dimensions: ${strip.image}`);
    }
    const canvas = createWorkerSafeCanvas(width, bitmap.height);
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("Team paint requires a 2D asset canvas");
    context.drawImage(bitmap, 0, 0);
    const fixed = context.getImageData(0, height * strip.teamPaint.baseRow, width, height).data;
    const paint = context.getImageData(0, height * strip.teamPaint.paintRow, width, height).data;
    const renderRows = strip.teamPaint.renderRows ?? 2;
    if (!Number.isInteger(renderRows) || renderRows < 1 || renderRows > strip.sheetRows) {
      throw new Error(`Invalid team-paint rendered row count: ${strip.image}`);
    }
    const neutral = context.getImageData(0, 0, width, height * renderRows);
    const createTexture = (pixels) => {
      const target = createWorkerSafeCanvas(width, height * renderRows), ctx = target.getContext("2d");
      if (!ctx) throw new Error("Team paint requires a palette canvas");
      const frame = ctx.createImageData(width, height * renderRows);
      frame.data.set(pixels);
      applyColorAdjustmentToRgba(frame.data, adjustment);
      ctx.putImageData(frame, 0, 0);
      const texture = pixi.Texture.from(target);
      texture.rtsRendererOwnedTexture = true;
      texture.source.scaleMode = "nearest";
      return texture;
    };
    const texture = createTexture(neutral.data);
    const colors = new Map();
    PALETTES.set(texture, {
      colors,
      get(color) {
        const key = hexToInt(color) & 0xffffff;
        if (!colors.has(key)) {
          const pixels = new Uint8ClampedArray(neutral.data);
          composeTeamPaintRgba(fixed, paint, key, pixels.subarray(0, fixed.length));
          colors.set(key, createTexture(pixels));
        }
        return colors.get(key);
      },
    });
    canvas.width = canvas.height = 0;
    return texture;
  } finally {
    bitmap.close?.();
  }
}

export function teamPaintTextureFor(texture, color) {
  return PALETTES.get(texture)?.get(color) ?? texture;
}

export function destroyTeamPaintTextures(texture) {
  const palettes = PALETTES.get(texture);
  if (!palettes) return;
  PALETTES.delete(texture);
  for (const derived of palettes.colors.values()) derived.destroy(true);
  palettes.colors.clear();
}
