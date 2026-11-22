import { gfxCircle, gfxEllipse, gfxFill, gfxNoFill, gfxPoly, gfxStroke, gfxStrokePaths } from "./native_graphics.js";

function noise(seed) {
  const value = Math.sin(seed * 127.1 + 311.7) * 43758.5453;
  return value - Math.floor(value);
}

// Visual time owns the continuous motion; the server's active trainee owns visibility.
export function drawTrainingLightning(g, entity, width, now) {
  const time = now / 1000;
  const phase = time + entity.id * 0.173;
  const radius = width * 0.21;
  const cx = entity.x;
  const cy = entity.y - width * 0.045 + Math.sin(phase * 2.8) * 1.1;
  const pulse = 0.5 + 0.5 * Math.sin(phase * 7.3);
  const flash = Math.floor(time * 14);
  gfxStroke(g, 0);
  // Uneven overlapping light fields keep the volume soft, without orbital rings.
  for (let halo = 7; halo >= 1; halo--) {
    gfxFill(g, halo > 3 ? 0x255dff : 0x65cfff, 0.025 + (8 - halo) * 0.009);
    gfxCircle(g, cx, cy, radius * (0.4 + halo * 0.15) * (0.96 + pulse * 0.09));
  }
  for (let cloud = 0; cloud < 7; cloud++) {
    const angle = cloud * 2.4 + phase * 0.2;
    gfxFill(g, 0x53baff, 0.05 + 0.025 * Math.sin(phase * 5 + cloud));
    gfxCircle(g, cx + Math.cos(angle) * radius * 0.5,
      cy + Math.sin(angle) * radius * 0.55, radius * 0.62);
  }

  // A hazy head, shoulders, hanging arms and split lower body suggest the trainee.
  // Layered translucent shapes avoid a sharp character outline inside the storm.
  const body = [[-0.18, -0.4], [-0.35, -0.32], [-0.48, 0.08], [-0.34, 0.18],
    [-0.23, -0.04], [-0.2, 0.25], [-0.26, 0.66], [-0.08, 0.7], [0, 0.35],
    [0.1, 0.71], [0.27, 0.65], [0.18, 0.22], [0.23, -0.04], [0.36, 0.18],
    [0.48, 0.08], [0.35, -0.32], [0.18, -0.4]];
  for (const [scale, color, alpha] of [[1.22, 0x8edfff, 0.09], [1.08, 0xb2efff, 0.14], [1, 0x102d65, 0.48]]) {
    gfxFill(g, color, alpha * (0.85 + pulse * 0.15));
    gfxEllipse(g, cx, cy - radius * 0.58, radius * 0.17 * scale, radius * 0.22 * scale);
    gfxPoly(g, body.flatMap(([x, y]) => [cx + x * radius * scale, cy + y * radius * scale]));
  }
  gfxNoFill(g);

  const paths = [];
  // Branching, discontinuous discharges radiate from around the figure. Every
  // flash chooses new endpoints rather than rotating a persistent geometric cage.
  for (let bolt = 0; bolt < 11; bolt++) {
    const seed = flash * 19.7 + bolt * 37.1 + entity.id;
    const angle = noise(seed) * Math.PI * 2;
    const start = radius * (0.2 + noise(seed + 2) * 0.32);
    const end = radius * (0.8 + noise(seed + 3) * 0.46);
    const points = [];
    for (let step = 0; step <= 7; step++) {
      const reach = start + (end - start) * step / 7;
      const jag = (noise(seed + step * 3.1) - 0.5) * radius * 0.4;
      points.push([cx + Math.cos(angle) * reach - Math.sin(angle) * jag,
        cy + Math.sin(angle) * reach + Math.cos(angle) * jag]);
    }
    paths.push(points);
    const fork = points[3];
    const forkAngle = angle + (noise(seed + 7) > 0.5 ? 0.8 : -0.8);
    paths.push([fork,
      [fork[0] + Math.cos(forkAngle) * radius * 0.22, fork[1] + Math.sin(forkAngle) * radius * 0.22],
      [fork[0] + Math.cos(forkAngle + 0.3) * radius * 0.45, fork[1] + Math.sin(forkAngle + 0.3) * radius * 0.45]]);
  }
  // Broken jagged surface flashes outline a ball without completing a ring.
  for (let arc = 0; arc < 5; arc++) {
    const points = [];
    const seed = flash * 11.3 + arc * 21.7 + entity.id;
    const angle = noise(seed) * Math.PI * 2;
    for (let step = 0; step < 7; step++) {
      const a = angle + step * 0.11;
      const r = radius * (0.75 + noise(seed + step * 2.7) * 0.27);
      points.push([cx + Math.cos(a) * r, cy + Math.sin(a) * r]);
    }
    paths.push(points);
  }
  gfxStrokePaths(g, paths, 4.4, 0x296bff, 0.14);
  gfxStrokePaths(g, paths, 1.7, 0x68caff, 0.68);
  gfxStrokePaths(g, paths, 0.6, 0xecfbff, 0.92);
}
