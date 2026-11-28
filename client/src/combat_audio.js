import { EVENT, KIND, WEAPON_KIND, daoForKind, isDiscipleKind } from "./protocol.js";

export const PANZERFAUST_LAUNCH_SOUND_ID = "combat_panzerfaust_launch_01";
export const PANZERFAUST_IMPACT_SOUND_ID = "combat_panzerfaust_impact_01";

const PANZERFAUST_SOUND_BUCKET_PX = 192;

// Presentation material, not the armored balance flag: mortars and scout cars
// still need the metallic sword hit even though they are unarmored.
const METALLIC_SWORD_TARGETS = new Set([
  KIND.TANK, KIND.SCOUT_CAR, KIND.COMMAND_CAR, KIND.SCOUT_PLANE,
  KIND.MORTAR_TEAM, KIND.ANTI_TANK_GUN, KIND.ARTILLERY, KIND.ROCKET_LAUNCHER,
]);

export function discipleAttackSoundId(attacker, target, weaponKind) {
  const dao = daoForKind(attacker?.kind);
  if (weaponKind === WEAPON_KIND.DISCIPLE_ORB || (!weaponKind && dao === "magic")) {
    return "combat_disciple_orb_01";
  }
  if (dao === "wind") return "combat_disciple_spear_01";
  return METALLIC_SWORD_TARGETS.has(target?.kind)
    ? "combat_disciple_sword_strike_01"
    : "combat_disciple_sword_stab_01";
}

const DEFAULT_WEAPON_KIND_BY_ATTACKER_KIND = Object.freeze({
  [KIND.WORKER]: WEAPON_KIND.WORKER_TOOLS,
  [KIND.GOLEM]: WEAPON_KIND.GOLEM_FISTS,
  [KIND.RIFLEMAN]: WEAPON_KIND.RIFLEMAN_RIFLE,
  [KIND.PANZERFAUST]: WEAPON_KIND.RIFLEMAN_RIFLE,
  [KIND.WARRIOR]: WEAPON_KIND.WARRIOR_SWORD,
  [KIND.BOW_NOVICE]: WEAPON_KIND.DISCIPLE_ARROW,
  [KIND.WIND_ADEPT]: WEAPON_KIND.WARRIOR_SWORD,
  [KIND.WAR_ADEPT]: WEAPON_KIND.WARRIOR_SWORD,
  [KIND.MAGIC_ADEPT]: WEAPON_KIND.DISCIPLE_ORB,
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
  [WEAPON_KIND.WARRIOR_SWORD]: KIND.WARRIOR,
  [WEAPON_KIND.DISCIPLE_ORB]: KIND.WARRIOR,
  [WEAPON_KIND.DISCIPLE_ARROW]: KIND.BOW_NOVICE,
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
  if (kind === KIND.BOW_NOVICE || weaponKind === WEAPON_KIND.DISCIPLE_ARROW) return KIND.BOW_NOVICE;
  const defaultWeaponKind = defaultWeaponKindForAttackerKind(kind);
  if (!weaponKind || weaponKind === defaultWeaponKind) {
    return isDiscipleKind(kind) ? KIND.WARRIOR : kind;
  }
  return ATTACK_FEEDBACK_KIND_BY_WEAPON[weaponKind] || kind;
}

export function attackKindHasCombatSound(kind, weaponKind) {
  if (kind === KIND.BOW_NOVICE || weaponKind === WEAPON_KIND.DISCIPLE_ARROW
    || weaponKind === WEAPON_KIND.PANZERFAUST_LOADED_SHOT) return false;
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
