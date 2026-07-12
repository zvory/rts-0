export const WORLD_TO_SCENE_SCALE = 0.05;

function finite(value, label) {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new TypeError(`${label} must be finite`);
  return value;
}
export function worldPointToScene(point) {
  return Object.freeze({
    x: finite(point?.x, "world x") * WORLD_TO_SCENE_SCALE,
    y: finite(point?.heightPx ?? 0, "presentation height") * WORLD_TO_SCENE_SCALE,
    z: finite(point?.y, "world y") * WORLD_TO_SCENE_SCALE,
  });
}

export function sceneGroundToWorld(point) {
  return Object.freeze({
    x: finite(point?.x, "scene x") / WORLD_TO_SCENE_SCALE,
    y: finite(point?.z, "scene z") / WORLD_TO_SCENE_SCALE,
  });
}

export function worldHeightToScene(heightPx) {
  return finite(heightPx, "world height") * WORLD_TO_SCENE_SCALE;
}

export function worldScaleToScene(lengthPx) {
  return finite(lengthPx, "world scale") * WORLD_TO_SCENE_SCALE;
}

export function worldFacingToSceneYaw(facingRad) {
  return Math.PI / 2 - finite(facingRad, "world facing");
}

export function sceneYawToWorldFacing(yawRad) {
  return Math.PI / 2 - finite(yawRad, "scene yaw");
}

export function projectionSceneCamera(projection) {
  const perspective = projection?.perspective;
  const focus = projection?.camera?.focus;
  if (!perspective || !focus) throw new TypeError("Babylon requires a fixed-perspective ProjectionSnapshotV1.");
  const distance = finite(perspective.distanceWorldPx, "camera distance");
  const pitch = finite(perspective.pitchRad, "camera pitch");
  const target = worldPointToScene({ x: focus.x, y: focus.y, heightPx: 0 });
  const position = worldPointToScene({
    x: focus.x,
    y: focus.y - distance * Math.cos(pitch),
    heightPx: distance * Math.sin(pitch),
  });
  return Object.freeze({
    position,
    target,
    fovYRad: finite(perspective.fovYRad, "camera fov"),
    nearScene: worldScaleToScene(perspective.nearDepthWorldPx),
    farScene: worldScaleToScene(perspective.farDepthWorldPx),
  });
}

export function projectScenePoint(scenePoint, projection) {
  const world = sceneGroundToWorld(scenePoint);
  return projection.project({
    ...world,
    heightPx: finite(scenePoint?.y, "scene height") / WORLD_TO_SCENE_SCALE,
  });
}

export function sceneGroundHit(screenPoint, projection) {
  const world = projection.groundAtScreen(screenPoint);
  return world ? worldPointToScene({ ...world, heightPx: 0 }) : null;
}
