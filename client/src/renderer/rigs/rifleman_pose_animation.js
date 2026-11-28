// Experimental Rifleman: body movement uses its own bearing; combat uses the weapon bearing.
export function riflemanPoseFrame(instance, entity, context) {
  const now = Number(context.now) || 0;
  const previous = instance._riflemanPosition;
  const dx = previous ? entity.x - previous.x : 0, dy = previous ? entity.y - previous.y : 0;
  const displaced = previous && Math.hypot(dx, dy) > 0.01;
  if (displaced) instance._riflemanLastMovementAt = now;
  const fallbackMoving = instance._riflemanLastMovementAt != null
    && now - instance._riflemanLastMovementAt <= 100;
  const firing = context.recoilProgress > 0 && context.recoilWeaponKind === "rifleman_rifle";
  const moving = !firing && (typeof context.frameStripMoving === "boolean"
    ? context.frameStripMoving : fallbackMoving);
  instance._riflemanPosition = { x: entity.x, y: entity.y };
  if (displaced) instance._riflemanMoveFacing = Math.atan2(dy, dx);
  const bodyFacing = Number.isFinite(entity.facing) ? entity.facing
    : instance._riflemanMoveFacing ?? Math.PI / 2;
  const facing = moving ? bodyFacing
    : Number.isFinite(entity.weaponFacing) ? entity.weaponFacing : bodyFacing;
  const count = instance.strip?.directional?.directionCount ?? 16;
  const step = Math.PI * 2 / count;
  const direction = ((Math.round((facing - Math.PI / 2) / step) % count) + count) % count;
  instance.poseDirection = direction;
  let pose = 0;
  if (moving) {
    instance._riflemanFirstShot = null;
    instance._riflemanShot = null;
    pose = 2 + Math.floor(now / 90) % 6;
  } else {
    if (context.recoilProgress > 0 && context.recoilWeaponKind === "rifleman_rifle") {
      const shot = now - (Number(context.recoilPhase) || 0) * 420;
      if (instance._riflemanShot == null || shot > instance._riflemanShot + 150) {
        instance._riflemanShot = shot;
        instance._riflemanFirstShot ??= shot;
      }
    }
    if (instance._riflemanFirstShot != null) {
      const raiseAge = now - instance._riflemanFirstShot;
      if (raiseAge < 350) pose = raiseAge < 45 ? 8 : raiseAge < 90 ? 1 : raiseAge < 160 ? 9 : raiseAge < 260 ? 10 : 11;
      else {
        const age = now - instance._riflemanShot - 350;
        pose = age < 0 ? 11 : age < 70 ? 12 : age < 260 ? 13 : age < 380 ? 14 : 11;
        const engaging = entity.targetId != null;
        if (!engaging && now - instance._riflemanShot > 1500) {
          pose = now - instance._riflemanShot < 1660 ? 15 : 0;
          if (pose === 0) { instance._riflemanFirstShot = null; instance._riflemanShot = null; }
        }
      }
    }
  }
  return direction * 16 + pose;
}
