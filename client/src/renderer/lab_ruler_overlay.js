import { gfxCircle, gfxReset, gfxStroke, gfxStrokeLine } from "./native_graphics.js";

const RULER_COLOR = 0xffd36a;
const RULER_POINT_COLOR = 0xfff1ba;
const LABEL_STYLE = Object.freeze({
  fontFamily: "Inter, system-ui, sans-serif",
  fontSize: 12,
  fontWeight: "700",
  fill: 0xfff1ba,
  stroke: { color: 0x111418, width: 4 },
});

/** Persistent, browser-local Lab measurement overlay. */
export class LabRulerOverlay {
  constructor({ layer, pixi }) {
    this.graphics = new pixi.Graphics();
    this.labels = Array.from({ length: 4 }, () => {
      const label = new pixi.Text({ text: "", style: LABEL_STYLE });
      label.visible = false;
      label.anchor?.set?.(0.5, 0);
      return label;
    });
    layer.addChild(this.graphics, ...this.labels);
  }

  render(ruler, { tileSize = 32, zoom = 1 } = {}) {
    gfxReset(this.graphics.clear());
    this.hideLabels();
    const safeZoom = Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
    const safeTileSize = Number.isFinite(tileSize) && tileSize > 0 ? tileSize : 32;
    const start = finitePoint(ruler?.start);
    const end = finitePoint(ruler?.end);
    const cursor = finitePoint(ruler?.cursor);
    const scale = 1 / safeZoom;
    const labelOffset = 10 * scale;

    if (start) {
      drawPoint(this.graphics, start, scale, RULER_POINT_COLOR);
      this.showLabel(0, coordinateText(start, safeTileSize), start.x, start.y + labelOffset, scale);
    }

    const segmentEnd = end || (start ? cursor : null);
    if (start && segmentEnd) {
      gfxStrokeLine(
        this.graphics,
        start.x,
        start.y,
        segmentEnd.x,
        segmentEnd.y,
        2 * scale,
        RULER_COLOR,
        0.98,
      );
      drawPoint(this.graphics, segmentEnd, scale, RULER_POINT_COLOR);
      this.showLabel(1, coordinateText(segmentEnd, safeTileSize), segmentEnd.x, segmentEnd.y + labelOffset, scale);

      const dx = segmentEnd.x - start.x;
      const dy = segmentEnd.y - start.y;
      const length = Math.hypot(dx, dy);
      const perpendicularX = length > 0 ? -dy / length : 0;
      const perpendicularY = length > 0 ? dx / length : -1;
      this.showLabel(
        2,
        `${formatTileValue(length / safeTileSize)} tiles`,
        (start.x + segmentEnd.x) / 2 + perpendicularX * 12 * scale,
        (start.y + segmentEnd.y) / 2 + perpendicularY * 12 * scale,
        scale,
      );
    }

    if (cursor && (!segmentEnd || !samePoint(cursor, segmentEnd))) {
      drawPoint(this.graphics, cursor, scale, RULER_POINT_COLOR);
      this.showLabel(3, coordinateText(cursor, safeTileSize), cursor.x, cursor.y + labelOffset, scale);
    }
  }

  showLabel(index, text, x, y, scale) {
    const label = this.labels[index];
    label.text = text;
    label.position.set(x, y);
    label.scale.set(scale);
    label.visible = true;
  }

  hideLabels() {
    for (const label of this.labels) label.visible = false;
  }

  destroy() {
    this.graphics.destroy();
    for (const label of this.labels) label.destroy();
  }
}

function drawPoint(graphics, point, scale, color) {
  gfxStroke(graphics, 2 * scale, color, 1);
  gfxCircle(graphics, point.x, point.y, 4 * scale);
}

function coordinateText(point, tileSize) {
  return `X ${formatTileValue(point.x / tileSize)}  Y ${formatTileValue(point.y / tileSize)}`;
}

function formatTileValue(value) {
  return Number(value.toFixed(2)).toString();
}

function finitePoint(point) {
  return Number.isFinite(point?.x) && Number.isFinite(point?.y)
    ? { x: point.x, y: point.y }
    : null;
}

function samePoint(a, b) {
  return a.x === b.x && a.y === b.y;
}
