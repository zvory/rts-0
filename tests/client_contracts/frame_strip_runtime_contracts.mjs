import { assert } from "./assertions.mjs";
import { KIND, SETUP, STATE } from "../../client/src/protocol.js";
import { frameStripFrameIndex } from "../../client/src/renderer/rigs/frame_strip_runtime.js";
import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "../../client/src/renderer/rigs/machine_gunner_png_strip.js";

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
