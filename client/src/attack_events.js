function isAttackShotEvent(ev) {
  return !!(
    ev &&
    ev.e === "attack" &&
    typeof ev.from === "number" &&
    typeof ev.to === "number" &&
    ev.from !== ev.to
  );
}

export function attackEventsForFiredShots(events) {
  const shots = [];
  const seenAttackers = new Set();
  for (const ev of Array.isArray(events) ? events : []) {
    if (!isAttackShotEvent(ev) || seenAttackers.has(ev.from)) continue;
    seenAttackers.add(ev.from);
    shots.push(ev);
  }
  return shots;
}

export function muzzleFlashesForFiredShots(events, createdAt) {
  return attackEventsForFiredShots(events).map((ev) => ({
    from: ev.from,
    to: ev.to,
    targetPos: Array.isArray(ev.toPos) && ev.toPos.length === 2
      ? { x: ev.toPos[0], y: ev.toPos[1] }
      : null,
    createdAt,
  }));
}
