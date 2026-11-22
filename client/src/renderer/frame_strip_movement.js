import { SNAPSHOT_MS } from "../config.js";
import { STATE } from "../protocol.js";

const FRAME_STRIP_MOVEMENT_HOLD_MS = SNAPSHOT_MS * 3;

export function frameStripMovementVisual(e, state, motion, now) {
  const previousMotion = motion?.get?.(e.id);
  const snapshotMotion = snapshotMovementSample(e, state);
  const renderMoving = renderedPositionChanged(e, motion);
  // Automatic pursuit can displace a unit whose public order state is still idle.
  const moveState = e?.state === STATE.MOVE || e?.state === STATE.ATTACK || e?.state === STATE.IDLE;
  const freshSnapshot = snapshotMotion != null &&
    authoritativeSampleChanged(snapshotMotion, previousMotion);
  const observedMovement = moveState && (
    renderMoving ||
    (freshSnapshot && snapshotMotion.moving) ||
    (e?.state !== STATE.IDLE && snapshotMotion == null && previousMotion == null)
  );
  let lastMovementAt = null;
  if (moveState) {
    lastMovementAt = observedMovement
      ? now
      : previousMotion?.state === e.state
        ? previousMotion.lastMovementAt ?? null
        : null;
  }
  if (motion) {
    motion.set(e.id, {
      x: finite(e.x, 0),
      y: finite(e.y, 0),
      state: e?.state ?? null,
      snapshotTick: snapshotMotion?.tick ?? null,
      snapshotX: snapshotMotion?.x ?? null,
      snapshotY: snapshotMotion?.y ?? null,
      lastMovementAt,
    });
  }
  const active = lastMovementAt != null &&
    now - lastMovementAt <= FRAME_STRIP_MOVEMENT_HOLD_MS;
  return {
    moving: Boolean(active),
    activity: active ? 1 : 0,
  };
}

function snapshotMovementSample(entity, state) {
  const current = state?._curById?.get?.(entity?.id);
  const previous = state?._prevById?.get?.(entity?.id);
  if (!current || !previous) return null;
  if (!Number.isFinite(current.x) || !Number.isFinite(current.y)) return null;
  if (!Number.isFinite(previous.x) || !Number.isFinite(previous.y)) return null;
  return {
    moving: distanceSq(current.x - previous.x, current.y - previous.y) > 0.0025,
    tick: Number.isFinite(state?.tick) ? state.tick : null,
    x: current.x,
    y: current.y,
  };
}

function authoritativeSampleChanged(sample, previousMotion) {
  if (!previousMotion) return true;
  if (sample.tick != null && previousMotion.snapshotTick != null) {
    return sample.tick !== previousMotion.snapshotTick;
  }
  return sample.x !== previousMotion.snapshotX || sample.y !== previousMotion.snapshotY;
}

function renderedPositionChanged(entity, motion) {
  const previous = motion?.get?.(entity?.id);
  if (!previous) return false;
  if (!Number.isFinite(entity?.x) || !Number.isFinite(entity?.y)) return false;
  return distanceSq(entity.x - previous.x, entity.y - previous.y) > 0.0025;
}

function distanceSq(dx, dy) {
  return dx * dx + dy * dy;
}

function finite(value, fallback) {
  return Number.isFinite(value) ? value : fallback;
}
