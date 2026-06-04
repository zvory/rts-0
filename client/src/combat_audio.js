import { KIND } from "./protocol.js";

export function machineGunSoundKey(id) {
  return `combat:machine_gunner:${id}`;
}

export function machineGunnerHasAudibleTarget(e) {
  return !!(
    e &&
    e.kind === KIND.MACHINE_GUNNER &&
    typeof e.targetId === "number"
  );
}
