import { gfxCircle, gfxFill, gfxNoFill, gfxRect, gfxStroke, gfxStrokeLine, gfxStrokePaths } from "./native_graphics.js";

// Matches the Shield ability's 300-damage absorption pool.
const SHIELD_CAPACITY = 300;
const TAU = Math.PI * 2;

export function drawDiscipleShield(g, object, radius, now) {
  const time = now / 1000;
  const phase = time * 0.8 + (object.id || 0) * 2.4;
  const pulse = 0.5 + Math.sin(time * 2.85) * 0.5;
  gfxFill(g, 0x7eeaff, 0.055 + pulse * 0.035);
  gfxStroke(g, 5, 0x6eeaff, 0.12 + pulse * 0.08);
  gfxCircle(g, object.x, object.y, radius);
  gfxNoFill(g);
  gfxStroke(g, 1.5, 0xc4f8ff, 0.6 + pulse * 0.25);
  gfxCircle(g, object.x, object.y, radius - 3);

  // Three tapered ribbons sweep around the rim; each bright head carries a glint.
  for (let ribbon = 0; ribbon < 3; ribbon += 1) {
    const head = phase + ribbon * TAU / 3;
    for (let segment = 0; segment < 12; segment += 1) {
      const start = head - (12 - segment) * 0.065;
      const end = start + 0.065;
      const brightness = (segment + 1) / 12;
      const r = radius - 3 - Math.sin(start * 3 - time * 1.6) * 1.5;
      const points = [start, (start + end) / 2, end].map(angle => [
        object.x + Math.cos(angle) * r, object.y + Math.sin(angle) * r,
      ]);
      gfxStrokePaths(g, [points], 5, 0x56dfff, brightness * 0.24);
      gfxStrokePaths(g, [points], 1.8, 0xe3fcff, brightness * 0.9);
    }
    const x = object.x + Math.cos(head) * (radius - 3);
    const y = object.y + Math.sin(head) * (radius - 3);
    const size = 2 + Math.pow(0.5 + Math.sin(time * 4 + ribbon * 2) * 0.5, 3) * 3;
    gfxStrokeLine(g, x - size, y, x + size, y, 1.2, 0xe3fcff, 0.85);
    gfxStrokeLine(g, x, y - size, x, y + size, 1.2, 0xe3fcff, 0.85);
  }

  // HP is team-private: omit the bar when the authoritative view omits it.
  const hp = object.ownerState?.hp;
  if (!Number.isFinite(hp)) return;
  const fraction = Math.max(0, Math.min(1, hp / SHIELD_CAPACITY));
  const width = 44;
  const x = object.x - width / 2;
  const y = object.y - radius - 11;
  gfxStroke(g, 1, 0x071b26, 0.95);
  gfxFill(g, 0x071b26, 0.9);
  gfxRect(g, x - 1, y - 1, width + 2, 7);
  gfxStroke(g, 0);
  gfxFill(g, 0x79eaff, 0.95);
  if (fraction > 0) gfxRect(g, x, y, width * fraction, 5);
  gfxNoFill(g);
}
