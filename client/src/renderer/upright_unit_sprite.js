import { tiltScreen } from "../camera_tilt.js";

// Prewarp only the body quad. It remains in its original world-Y-sorted layer,
// so fog, canopy occlusion, contact shadows, selection rings and teardown keep their owners.
export function uprightBodyCorners(view, anchor, offsets) {
  const foot = tiltScreen(view, anchor);
  if (foot.depth <= 0) return null;
  return offsets.map((offset) => tiltScreen(view, {
    x: foot.x + offset.x / foot.depth,
    y: foot.y + offset.y / foot.depth,
  }, true));
}

// Return a world-plane point whose final projection matches an upright body offset.
// Used by muzzle effects and HP anchors as well as the body's quad.
export function uprightWorldPoint(renderer, anchor, point) {
  if (!renderer._mapTiltDegrees) return point;
  const w = renderer.app.renderer.screen.width, h = renderer.app.renderer.screen.height;
  const zoom = renderer.world.scale.x;
  const rawAnchor = { x: anchor.x * zoom + renderer.world.x - w / 2,
    y: anchor.y * zoom + renderer.world.y - h / 2 };
  const corner = uprightBodyCorners({ viewW: w, viewH: h, mapTilt: renderer._mapTiltDegrees },
    rawAnchor, [{ x: (point.x - anchor.x) * zoom, y: (point.y - anchor.y) * zoom }])?.[0];
  return corner ? { x: anchor.x + (corner.x - rawAnchor.x) / zoom,
    y: anchor.y + (corner.y - rawAnchor.y) / zoom } : point;
}

export function updateUprightUnitSprite(renderer, instance) {
  const enabled = renderer._mapTiltDegrees
    && instance.strip.image?.includes("rifleman-experimental/") && !instance.silhouetteShadow;
  if (!enabled) {
    if (instance.uprightMesh) instance.uprightMesh.visible = false;
    return;
  }
  const { sprite, container } = instance;
  const w = renderer.app.renderer.screen.width;
  const h = renderer.app.renderer.screen.height;
  const zoom = renderer.world.scale.x;
  const scale = container.scale.x;
  const view = { viewW: w, viewH: h, mapTilt: renderer._mapTiltDegrees };
  const anchor = {
    x: container.x * zoom + renderer.world.x - w / 2,
    y: container.y * zoom + renderer.world.y - h / 2,
  };
  const width = instance.strip.frameWidth;
  const height = instance.strip.frameHeight;
  const offsets = [[0, 0], [1, 0], [1, 1], [0, 1]].map(([x, y]) => ({
    x: ((x - sprite.anchor.x) * width * sprite.scale.x + sprite.x) * scale * zoom,
    y: ((y - sprite.anchor.y) * height * sprite.scale.y + sprite.y) * scale * zoom,
  }));
  const corners = uprightBodyCorners(view, anchor, offsets);
  if (!corners) return;
  if (!instance.uprightMesh) {
    instance.uprightMesh = new PIXI.PerspectiveMesh({ texture: sprite.texture, verticesX: 8, verticesY: 8 });
    container.addChild(instance.uprightMesh);
  }
  const mesh = instance.uprightMesh;
  mesh.texture = sprite.texture;
  mesh.tint = sprite.tint;
  mesh.visible = true;
  mesh.setCorners(...corners.flatMap((p) => [
    (p.x - anchor.x) / (zoom * scale), (p.y - anchor.y) / (zoom * scale),
  ]));
  sprite.visible = false;
}
