import { CARRIED_ITEMS, carriedItemMask, ITEM_ICON_SHAPES } from "../config.js";

const ICON_SCALE = 0.9;
const OUTLINE_WIDTH = 0.7 * ICON_SCALE;


// Geometry is independent of the catalog; additional items need no inventory-slot layout.
export function itemRosettePositions(count, centerIndex = count - 1) {
  if (count <= 0) return [];
  if (count <= 2) return Array.from({ length: count }, (_, i) => [(i - (count - 1) / 2) * 10, 0]);
  if (count === 3) return [[0, -5], [-5, 5], [5, 5]];
  if (count === 4) return [[-6, -6], [6, -6], [-6, 6], [6, 6]];
  if (count === 5) return [[-7, -7], [7, -7], [-7, 7], [7, 7], [0, 0]];
  const radius = count === 6 ? 10.5 : count === 7 ? 10.2 : count === 8 ? 12 : count === 9
    ? 13.3 : Math.max(10, 10 / (2 * Math.sin(Math.PI / (count - 1))));
  let ringIndex = 0;
  return Array.from({ length: count }, (_, i) => {
    if (i === centerIndex) return [0, 0];
    const angle = (-(count === 9 ? 67.5 : 90) + ringIndex++ * 360 / (count - 1)) * Math.PI / 180;
    return [Math.cos(angle) * radius, Math.sin(angle) * radius];
  });
}

export function drawItemIcon(g, item, x, y) {
  const points = ITEM_ICON_SHAPES[item.shape].map((value, i) => value * ICON_SCALE + (i % 2 ? y : x));
  g.poly(points).fill(item.color).stroke({ color: 0x09121c, width: OUTLINE_WIDTH, join: "round" });
  if (item.shape === "return") {
    const point = (a, b) => [x + a * ICON_SCALE, y + b * ICON_SCALE];
    g.moveTo(...point(2.5, 1.7)).lineTo(...point(-1.7, 1.7))
      .lineTo(...point(-1.7, -1.5)).lineTo(...point(1.5, -1.5))
      .stroke({ color: 0x29153e, width: 1.3 * ICON_SCALE });
    g.poly([0, -3.2, 2, -1.5, 0, 0.2].map((value, i) =>
      value * ICON_SCALE + (i % 2 ? y : x))).fill(0x29153e);
  }
}

export function drawCarriedItemRosette(renderer, entity, poolName = "carriedItems") {
  const mask = carriedItemMask(entity);
  if (!mask) return;
  const g = renderer._staticSlot(poolName, entity.id, mask);
  // World-center anchoring is deliberately independent of HP/Insight bar placement.
  g.position.set(entity.x, entity.y);
  if (!g.rtsStaticRedraw) return;
  const items = CARRIED_ITEMS.filter((_, i) => mask & (1 << i));
  const escapeIndex = items.findIndex((item) => item.shape === "return");
  const positions = itemRosettePositions(items.length, escapeIndex < 0 ? items.length - 1 : escapeIndex);
  items.forEach((item, i) => drawItemIcon(g, item, ...positions[i]));
  g.rtsStaticRenderKey = mask;
}
