import { KIND, WEAPON_KIND } from "./protocol.js";

const DEFAULT_RECOIL_DURATION_MS = 300;

const RECOIL_DURATION_MS_BY_KIND = Object.freeze({
  [KIND.RIFLEMAN]: 420,
  [KIND.PANZERFAUST]: 420,
  [KIND.MACHINE_GUNNER]: 160,
  [KIND.ANTI_TANK_GUN]: 820,
  [KIND.MORTAR_TEAM]: 520,
  [KIND.ARTILLERY]: 980,
  [KIND.SCOUT_CAR]: 160,
  [KIND.TANK]: 650,
});

const RECOIL_DURATION_MS_BY_WEAPON_KIND = Object.freeze({
  [WEAPON_KIND.RIFLEMAN_RIFLE]: RECOIL_DURATION_MS_BY_KIND[KIND.RIFLEMAN],
  [WEAPON_KIND.MACHINE_GUNNER_MG]: RECOIL_DURATION_MS_BY_KIND[KIND.MACHINE_GUNNER],
  [WEAPON_KIND.ANTI_TANK_GUN]: RECOIL_DURATION_MS_BY_KIND[KIND.ANTI_TANK_GUN],
  [WEAPON_KIND.MORTAR_TEAM_MORTAR]: RECOIL_DURATION_MS_BY_KIND[KIND.MORTAR_TEAM],
  [WEAPON_KIND.ARTILLERY_GUN]: RECOIL_DURATION_MS_BY_KIND[KIND.ARTILLERY],
  [WEAPON_KIND.SCOUT_CAR_MG]: RECOIL_DURATION_MS_BY_KIND[KIND.SCOUT_CAR],
  [WEAPON_KIND.PANZERFAUST_LOADED_SHOT]: 620,
  [WEAPON_KIND.TANK_CANNON]: RECOIL_DURATION_MS_BY_KIND[KIND.TANK],
});

export function weaponRecoilDurationMs(kind, weaponKind) {
  if (weaponKind === WEAPON_KIND.TANK_COAX) return 0;
  return RECOIL_DURATION_MS_BY_WEAPON_KIND[weaponKind]
    || RECOIL_DURATION_MS_BY_KIND[kind]
    || DEFAULT_RECOIL_DURATION_MS;
}

/**
 * Sample the renderer-authored recoil cycle at an elapsed time from the attack event.
 * Inactive samples represent a weapon with no recoil cycle or an expired cycle.
 */
export function sampleWeaponRecoilCycle(kind, elapsedMs, weaponKind) {
  const durationMs = weaponRecoilDurationMs(kind, weaponKind);
  if (durationMs <= 0 || elapsedMs > durationMs) {
    return { active: false, phase: 0, progress: 0 };
  }
  if (elapsedMs < 0) return { active: true, phase: 0, progress: 1 };
  const phase = clamp01(elapsedMs / durationMs);
  return { active: true, phase, progress: recoilCurve(phase) };
}

function recoilCurve(phase) {
  const progress = phase < 0 ? 0 : phase > 1 ? 1 : phase;
  if (progress < 0.18) {
    return 1 - progress * 0.12;
  }
  const settle = (progress - 0.18) / 0.82;
  return Math.cos(settle * Math.PI * 0.5) * 0.88;
}

function clamp01(value) {
  return Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0));
}
