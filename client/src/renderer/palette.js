import { ARTILLERY_SETUP_TICKS, TICK_HZ } from "../config.js";
import { KIND } from "../protocol.js";

// Frames an entity id may go unseen before pooled objects are destroyed.
export const SWEEP_EVICT_FRAMES = 120;

// Deployed-weapon setup / teardown visuals are time-based on the client so the
// transition reads smoothly between snapshots.
export const DEPLOYED_WEAPON_ANIM_MS = 1000;
export const ARTILLERY_DEPLOYED_WEAPON_ANIM_MS = (ARTILLERY_SETUP_TICKS / TICK_HZ) * 1000;
export const WEAPON_RECOIL_PX = {
  [KIND.RIFLEMAN]: 8.0,
  [KIND.MACHINE_GUNNER]: 5.5,
  [KIND.ANTI_TANK_GUN]: 26.0,
  [KIND.MORTAR_TEAM]: 14.0,
  [KIND.ARTILLERY]: 30.0,
  [KIND.TANK]: 9.0,
  [KIND.SCOUT_CAR]: 6.5,
};

export const ZERO_OFFSET = Object.freeze({ x: 0, y: 0 });
