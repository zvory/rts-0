import { CARRIED_ITEMS, ITEM_ICON_SHAPES } from "./config.js";
import { ABILITY_OBJECT_KIND } from "./protocol.js";

/** Only consume the server's fog-filtered delivery objects; never infer a recipient. */
export function drawMinimapItems(ctx, objects, project, scale = 1) {
  for (const object of objects || []) {
    if (object.kind !== ABILITY_OBJECT_KIND.LINE_PROJECTILE) continue;
    const item = CARRIED_ITEMS.find((item) => item.ability === object.ability);
    if (!item) continue;
    const { x, y } = project(object.x, object.y);
    ctx.save();
    ctx.translate(x, y);
    ctx.scale(scale * 0.9, scale * 0.9);
    const points = ITEM_ICON_SHAPES[item.shape];
    ctx.beginPath();
    ctx.moveTo(points[0], points[1]);
    for (let i = 2; i < points.length; i += 2) ctx.lineTo(points[i], points[i + 1]);
    ctx.closePath();
    ctx.fillStyle = `#${item.color.toString(16).padStart(6, "0")}`;
    ctx.fill();
    ctx.strokeStyle = "#09121c";
    ctx.lineWidth = 0.7;
    ctx.lineJoin = "round";
    ctx.stroke();
    if (item.shape === "return") {
      ctx.beginPath();
      ctx.moveTo(2.5, 1.7); ctx.lineTo(-1.7, 1.7);
      ctx.lineTo(-1.7, -1.5); ctx.lineTo(1.5, -1.5);
      ctx.strokeStyle = "#29153e"; ctx.lineWidth = 1.3; ctx.stroke();
      ctx.beginPath(); ctx.moveTo(0, -3.2); ctx.lineTo(2, -1.5);
      ctx.lineTo(0, 0.2); ctx.closePath(); ctx.fillStyle = "#29153e"; ctx.fill();
    }
    ctx.restore();
  }
}
