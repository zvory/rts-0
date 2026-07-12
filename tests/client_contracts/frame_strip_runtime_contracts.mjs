import { assert } from "./assertions.mjs";
import { KIND, SETUP, STATE } from "../../client/src/protocol.js";
import {
  frameStripFrameIndex,
  frameStripVisualFacing,
  frameStripWorldScale,
} from "../../client/src/renderer/rigs/frame_strip_runtime.js";
import { _frameStripMovementVisual } from "../../client/src/renderer/units.js";
import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/machine_gunner_png_strip.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/rifleman_png_strip.js";

const deployedMachineGunner = {
  id: 7,
  kind: KIND.MACHINE_GUNNER,
  state: STATE.ATTACK,
  setupState: SETUP.DEPLOYED,
};

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 0,
    recoilPhase: 0,
  }) === 11,
  "deployed Machine Gunner holds the setup-complete frame when not firing",
);

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 1,
    recoilPhase: 0,
  }) === 12,
  "Machine Gunner firing animation starts from the deployed rest frame",
);

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 0.8,
    recoilPhase: 0.34,
  }) === 13,
  "Machine Gunner firing animation advances to the recoil peak frame",
);

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, deployedMachineGunner, {
    recoilProgress: 0.4,
    recoilPhase: 0.67,
  }) === 14,
  "Machine Gunner firing animation returns to the deployed rest frame",
);

assert(
  frameStripFrameIndex(
    MACHINE_GUNNER_PNG_FRAME_STRIP,
    { ...deployedMachineGunner, setupState: SETUP.SETTING_UP },
    {
      setupVisual: { frameProgress: 1 },
      recoilProgress: 1,
      recoilPhase: 0.34,
    },
  ) === 11,
  "Machine Gunner setup frames take precedence over firing frames",
);

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE },
    {
      now: 0,
      recoilProgress: 1,
      recoilPhase: 0.05,
    },
  ) === 5,
  "moving Rifleman uses its first firing frame during active weapon recoil",
);

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE },
    {
      now: 0,
      recoilProgress: 1,
      recoilPhase: 0.18,
    },
  ) === 6,
  "moving Rifleman advances to its recovery firing frame during active weapon recoil",
);

const postRecoilFrame = frameStripFrameIndex(
  RIFLEMAN_PNG_FRAME_STRIP,
  { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE },
  {
    now: 0,
    recoilProgress: 1,
    recoilPhase: 0.3,
  },
);
assert(postRecoilFrame !== 5 && postRecoilFrame !== 6,
  "moving Rifleman returns to movement frames after the brief firing hold");

const stationaryMoveContext = { now: 1000, frameStripMoving: false };

assert(
  frameStripFrameIndex(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE, weaponFacing: 1.2 },
    stationaryMoveContext,
  ) === RIFLEMAN_PNG_FRAME_STRIP.idleFrame,
  "Rifleman in move state holds idle frame when its position is not changing",
);

assert(
  frameStripVisualFacing(
    RIFLEMAN_PNG_FRAME_STRIP,
    { id: 11, kind: KIND.RIFLEMAN, state: STATE.MOVE, facing: 0.4, weaponFacing: 1.2 },
    stationaryMoveContext,
  ) === 1.2,
  "stationary Rifleman in move state uses standing weapon-facing art",
);

const stationaryMachineGunner = {
  id: 12,
  kind: KIND.MACHINE_GUNNER,
  state: STATE.MOVE,
  setupState: SETUP.PACKED,
  facing: 0.3,
  weaponFacing: 1.1,
};

assert(
  frameStripFrameIndex(MACHINE_GUNNER_PNG_FRAME_STRIP, stationaryMachineGunner, stationaryMoveContext) ===
    MACHINE_GUNNER_PNG_FRAME_STRIP.idleFrame,
  "Machine Gunner in move state holds idle frame when its position is not changing",
);

assert(
  frameStripVisualFacing(MACHINE_GUNNER_PNG_FRAME_STRIP, stationaryMachineGunner, stationaryMoveContext) ===
    stationaryMachineGunner.facing,
  "stationary packed Machine Gunner in move state keeps body-facing idle art",
);

assert(
  frameStripWorldScale(MACHINE_GUNNER_PNG_FRAME_STRIP, stationaryMachineGunner, stationaryMoveContext) ===
    MACHINE_GUNNER_PNG_FRAME_STRIP.worldScale,
  "stationary Machine Gunner in move state does not use carried-movement scale",
);

const stationaryMoveEntity = {
  id: 20,
  kind: KIND.RIFLEMAN,
  state: STATE.MOVE,
  x: 40,
  y: 50,
};

assert(
  frameStripMovementFor(stationaryMoveEntity, stationaryMoveEntity, stationaryMoveEntity).moving === false,
  "renderer marks move-state frame-strip units stationary when current and previous snapshots did not move",
);

assert(
  frameStripMovementFor(stationaryMoveEntity, { ...stationaryMoveEntity, x: 38 }, stationaryMoveEntity).moving === true,
  "renderer marks frame-strip units moving when authoritative snapshot positions changed",
);

assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    stationaryMoveEntity,
    stationaryMoveEntity,
    new Map([[stationaryMoveEntity.id, { x: 36, y: 50 }]]),
  ).moving === true,
  "renderer keeps frame-strip movement active for predicted render-position changes",
);

const heldMovementSnapshot = {
  previous: { ...stationaryMoveEntity, x: 38 },
  current: stationaryMoveEntity,
  motion: new Map(),
  tick: 120,
};
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    0,
  ).moving === true,
  "renderer admits a fresh authoritative movement sample",
);
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    1000 / 60,
  ).moving === true,
  "renderer keeps movement frames latched across the next 60 FPS render of a held 30 Hz snapshot",
);
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    1000 / 30,
  ).moving === true,
  "renderer keeps movement frames latched until the next authoritative snapshot is due",
);
assert(
  frameStripMovementFor(
    stationaryMoveEntity,
    heldMovementSnapshot.previous,
    heldMovementSnapshot.current,
    heldMovementSnapshot.motion,
    heldMovementSnapshot.tick,
    120,
  ).moving === false,
  "renderer settles a paused or stalled held movement snapshot back to idle after the movement hold",
);

function frameStripMovementFor(entity, previous, current, motion = new Map(), tick = null, now = 0) {
  return _frameStripMovementVisual.call(
    { _frameStripMotion: motion, visualNow: () => now },
    entity,
    {
      _prevById: new Map([[entity.id, previous]]),
      _curById: new Map([[entity.id, current]]),
      tick,
    },
  );
}
