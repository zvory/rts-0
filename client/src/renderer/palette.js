import { KIND } from "../protocol.js";

// Frames an entity id may go unseen before pooled objects are destroyed.
export const SWEEP_EVICT_FRAMES = 120;

// Deployed-weapon setup / teardown visuals are time-based on the client so the
// transition reads smoothly between snapshots.
export const DEPLOYED_WEAPON_ANIM_MS = 1000;
export const WEAPON_RECOIL_PX = {
  [KIND.RIFLEMAN]: 8.0,
  [KIND.MACHINE_GUNNER]: 5.5,
  [KIND.AT_TEAM]: 26.0,
  [KIND.TANK]: 9.0,
  [KIND.SCOUT_CAR]: 6.5,
};

export const ZERO_OFFSET = Object.freeze({ x: 0, y: 0 });
