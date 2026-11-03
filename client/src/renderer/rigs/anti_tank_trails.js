import { smoothstep01 } from "../shared.js";

// Both trails remain attached while travelling. Swing halfway out, pause, then
// latch through the final ten degrees over the last 2% of the setup interval.
export function antiTankTrailAngle(progress) {
  const p = Math.max(0, Math.min(1, progress));
  const degrees = p < 0.82 ? 20 - 10 * smoothstep01(p / 0.82)
    : p < 0.98 ? 10 : 10 * (1 - smoothstep01((p - 0.98) / 0.02));
  return degrees * Math.PI / 180;
}

export function applyAntiTankTrailTransform(output, state, sprite, context) {
  if (!context) return;
  const left = sprite?.id === "sprite.at.leftTrail.deployed";
  if (!left && sprite?.id !== "sprite.at.rightTrail.deployed") return;
  const progress = context.setupVisual?.frameProgress ?? context.setupVisual?.prongFactor ?? 1;
  const sign = left ? -1 : 1;
  const angle = sign * antiTankTrailAngle(progress);
  if (angle === 0) return;
  // Rotate about the front attachment, rather than the atlas cell's centre.
  // Preserve the final authored pose and the carriage's facing/recoil translation.
  const a = state.transform.rotation;
  const hingeX = state.transform.x - 9 * Math.cos(a) - sign * 7 * Math.sin(a);
  const hingeY = state.transform.y - 9 * Math.sin(a) + sign * 7 * Math.cos(a);
  const dx = output.x - hingeX, dy = output.y - hingeY;
  const c = Math.cos(angle), s = Math.sin(angle);
  output.x = hingeX + dx * c - dy * s;
  output.y = hingeY + dx * s + dy * c;
  output.rotation += angle;
}
