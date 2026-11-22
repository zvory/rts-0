import { gfxCircle, gfxFill, gfxNoFill, gfxStroke, gfxStrokePaths } from "./native_graphics.js";

// Visual time owns the continuous motion; the server's active trainee owns visibility.
export function drawTrainingLightning(g, entity, width, now) {
  const time = now / 1000;
  const phase = time + entity.id * 0.173;
  const radius = width * 0.19;
  const cx = entity.x;
  const cy = entity.y - width * 0.045 + Math.sin(phase * 2.8) * 1.1;
  const pulse = 0.5 + 0.5 * Math.sin(phase * 7.3);
  gfxStroke(g, 0);
  for (let halo = 6; halo >= 1; halo--) {
    gfxFill(g, halo > 3 ? 0x3479ff : 0x8bddff, 0.025 + (7 - halo) * 0.012);
    gfxCircle(g, cx, cy, radius * (0.5 + halo * 0.19) * (0.96 + pulse * 0.06));
  }
  gfxNoFill(g);

  const paths = [];
  const flicker = Math.floor(time * 18);
  // Three differently tilted great circles give the electricity a spherical volume.
  for (let arc = 0; arc < 3; arc++) {
    const points = [];
    const tilt = phase * (arc % 2 ? -0.65 : 0.48) + arc * 2.1;
    const depth = 0.35 + 0.22 * Math.sin(phase * 1.6 + arc);
    for (let step = 0; step <= 40; step++) {
      const angle = step / 40 * Math.PI * 2;
      const jitter = Math.sin(step * 12.73 + flicker * 7.91 + arc * 4.3) * radius * 0.10;
      const x = Math.cos(angle) * (radius + jitter);
      const y = Math.sin(angle) * (radius + jitter) * depth;
      points.push([cx + x * Math.cos(tilt) - y * Math.sin(tilt),
        cy + x * Math.sin(tilt) + y * Math.cos(tilt)]);
    }
    paths.push(points);
  }
  // Short-lived branching discharges connect the ball to the training gate.
  for (let bolt = 0; bolt < 3; bolt++) {
    const angle = phase * 1.2 + bolt * Math.PI * 2 / 3 + Math.sin(flicker + bolt) * 0.6;
    const points = [[cx, cy]];
    for (let step = 1; step <= 6; step++) {
      const reach = radius * step / 6 * (bolt === flicker % 3 ? 1.65 : 1);
      const jag = Math.sin(flicker * 3.7 + step * 9.3 + bolt) * radius * 0.15;
      points.push([cx + Math.cos(angle) * reach - Math.sin(angle) * jag,
        cy + Math.sin(angle) * reach + Math.cos(angle) * jag]);
    }
    paths.push(points);
    const fork = points[3];
    paths.push([fork, [fork[0] + Math.cos(angle + 0.8) * radius * 0.45,
      fork[1] + Math.sin(angle + 0.8) * radius * 0.45]]);
  }
  gfxStrokePaths(g, paths, 4.2, 0x347eff, 0.16);
  gfxStrokePaths(g, paths, 1.7, 0x73cfff, 0.8);
  gfxStrokePaths(g, paths, 0.65, 0xe8faff, 0.95);
  gfxFill(g, 0xe5faff, 0.65 + pulse * 0.2);
  gfxCircle(g, cx, cy, radius * (0.16 + pulse * 0.05));
  gfxNoFill(g);
}
