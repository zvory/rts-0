/**
 * Resolve the Pixi transform for one atlas sprite without touching display state.
 * Runtime rendering and assembled HUD icons share this so authored atlas offsets
 * and pivots cannot drift between the two presentations.
 */
export function resolvePngSpriteTransform(state, frame, part, output = {}) {
  const pixelsPerUnitX = frame.pixelsPerUnitX || frame.pixelsPerUnit || 1;
  const pixelsPerUnitY = frame.pixelsPerUnitY || frame.pixelsPerUnit || 1;
  const stateRotation = state.transform.rotation;
  const cos = Math.cos(stateRotation);
  const sin = Math.sin(stateRotation);
  const localX = state.localOffset?.x ?? 0;
  const localY = state.localOffset?.y ?? 0;
  const rotatedLocalX = localX === 0 && localY === 0 ? 0 : localX * cos - localY * sin;
  const rotatedLocalY = localX === 0 && localY === 0 ? 0 : localX * sin + localY * cos;
  const defaultPivotX = frame.originX + state.pivot.x * pixelsPerUnitX;
  const defaultPivotY = frame.originY + state.pivot.y * pixelsPerUnitY;
  const pivotX = part?.rotationPivotX ?? frame.rotationPivotX ?? defaultPivotX;
  const pivotY = part?.rotationPivotY ?? frame.rotationPivotY ?? defaultPivotY;
  const spriteOffsetX = part?.positionOffsetX ?? frame.positionOffsetX ?? 0;
  const spriteOffsetY = part?.positionOffsetY ?? frame.positionOffsetY ?? 0;
  const rotatedSpriteX = spriteOffsetX === 0 && spriteOffsetY === 0
    ? 0
    : spriteOffsetX * cos - spriteOffsetY * sin;
  const rotatedSpriteY = spriteOffsetX === 0 && spriteOffsetY === 0
    ? 0
    : spriteOffsetX * sin + spriteOffsetY * cos;
  const pivotReferenceRotation = stateRotation + (
    part?.rotationPivotReferenceOffset ?? frame.rotationPivotReferenceOffset ?? 0
  );
  const pivotReferenceCos = Math.cos(pivotReferenceRotation);
  const pivotReferenceSin = Math.sin(pivotReferenceRotation);
  const pivotLocalX = (pivotX - defaultPivotX) / pixelsPerUnitX;
  const pivotLocalY = (pivotY - defaultPivotY) / pixelsPerUnitY;
  const pivotOffsetX = pivotLocalX === 0 && pivotLocalY === 0
    ? 0
    : pivotLocalX * pivotReferenceCos - pivotLocalY * pivotReferenceSin;
  const pivotOffsetY = pivotLocalX === 0 && pivotLocalY === 0
    ? 0
    : pivotLocalX * pivotReferenceSin + pivotLocalY * pivotReferenceCos;

  output.x = state.transform.x + rotatedLocalX + rotatedSpriteX + pivotOffsetX;
  output.y = state.transform.y + rotatedLocalY + rotatedSpriteY + pivotOffsetY;
  output.pivotX = pivotX;
  output.pivotY = pivotY;
  output.scaleX = (
    state.transform.scaleX * (state.geometryScale?.x ?? 1)
  ) / pixelsPerUnitX;
  output.scaleY = (
    state.transform.scaleY * (state.geometryScale?.y ?? 1)
  ) / pixelsPerUnitY;
  output.rotation = stateRotation + (part?.rotationOffset ?? frame.rotationOffset ?? 0);
  return output;
}
