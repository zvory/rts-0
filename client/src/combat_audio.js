import { EVENT, KIND, WEAPON_KIND } from "./protocol.js";

export const PANZERFAUST_LAUNCH_SOUND_ID = "combat_panzerfaust_launch_01";
export const PANZERFAUST_IMPACT_SOUND_ID = "combat_panzerfaust_impact_01";

const PANZERFAUST_SOUND_BUCKET_PX = 192;

const DEFAULT_WEAPON_KIND_BY_ATTACKER_KIND = Object.freeze({
  [KIND.WORKER]: WEAPON_KIND.WORKER_TOOLS,
  [KIND.GOLEM]: WEAPON_KIND.GOLEM_FISTS,
  [KIND.RIFLEMAN]: WEAPON_KIND.RIFLEMAN_RIFLE,
  [KIND.PANZERFAUST]: WEAPON_KIND.RIFLEMAN_RIFLE,
  [KIND.MACHINE_GUNNER]: WEAPON_KIND.MACHINE_GUNNER_MG,
  [KIND.SCOUT_CAR]: WEAPON_KIND.SCOUT_CAR_MG,
  [KIND.ANTI_TANK_GUN]: WEAPON_KIND.ANTI_TANK_GUN,
  [KIND.MORTAR_TEAM]: WEAPON_KIND.MORTAR_TEAM_MORTAR,
  [KIND.ARTILLERY]: WEAPON_KIND.ARTILLERY_GUN,
  [KIND.TANK]: WEAPON_KIND.TANK_CANNON,
});

const ATTACK_FEEDBACK_KIND_BY_WEAPON = Object.freeze({
  [WEAPON_KIND.WORKER_TOOLS]: KIND.WORKER,
  [WEAPON_KIND.GOLEM_FISTS]: KIND.GOLEM,
  [WEAPON_KIND.RIFLEMAN_RIFLE]: KIND.RIFLEMAN,
  [WEAPON_KIND.MACHINE_GUNNER_MG]: KIND.MACHINE_GUNNER,
  [WEAPON_KIND.PANZERFAUST_LOADED_SHOT]: KIND.RIFLEMAN,
  [WEAPON_KIND.SCOUT_CAR_MG]: KIND.SCOUT_CAR,
  [WEAPON_KIND.ANTI_TANK_GUN]: KIND.ANTI_TANK_GUN,
  [WEAPON_KIND.MORTAR_TEAM_MORTAR]: KIND.MORTAR_TEAM,
  [WEAPON_KIND.ARTILLERY_GUN]: KIND.ARTILLERY,
  [WEAPON_KIND.TANK_CANNON]: KIND.TANK,
  [WEAPON_KIND.TANK_COAX]: KIND.MACHINE_GUNNER,
});

const PANZERFAUST_FEEDBACK_SOUND_IDS = Object.freeze({
  [EVENT.PANZERFAUST_LAUNCH]: PANZERFAUST_LAUNCH_SOUND_ID,
  [EVENT.PANZERFAUST_IMPACT]: PANZERFAUST_IMPACT_SOUND_ID,
});

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

export function defaultWeaponKindForAttackerKind(kind) {
  return DEFAULT_WEAPON_KIND_BY_ATTACKER_KIND[kind];
}

export function attackFeedbackKind(kind, weaponKind) {
  const defaultWeaponKind = defaultWeaponKindForAttackerKind(kind);
  if (!weaponKind || weaponKind === defaultWeaponKind) return kind;
  return ATTACK_FEEDBACK_KIND_BY_WEAPON[weaponKind] || kind;
}

export function attackKindHasCombatSound(kind, weaponKind) {
  if (weaponKind === WEAPON_KIND.PANZERFAUST_LOADED_SHOT) return false;
  const feedbackKind = attackFeedbackKind(kind, weaponKind);
  return feedbackKind !== KIND.WORKER;
}

export function panzerfaustFeedbackSoundId(eventKind) {
  return PANZERFAUST_FEEDBACK_SOUND_IDS[eventKind] || null;
}

export function panzerfaustFeedbackDedupKey(eventKind, x, y, category = "combat_other") {
  if (!panzerfaustFeedbackSoundId(eventKind) || !Number.isFinite(x) || !Number.isFinite(y)) {
    return null;
  }
  const prefix = eventKind === EVENT.PANZERFAUST_LAUNCH
    ? "panzerfaust_launch"
    : "panzerfaust_impact";
  const bx = Math.floor(x / PANZERFAUST_SOUND_BUCKET_PX);
  const by = Math.floor(y / PANZERFAUST_SOUND_BUCKET_PX);
  return `${prefix}:${category}:${bx}:${by}`;
}
