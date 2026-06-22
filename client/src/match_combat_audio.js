import {
  attackKindHasCombatSound,
  machineGunnerHasAudibleTarget,
  machineGunSoundKey,
} from "./combat_audio.js";
import { EVENT, KIND } from "./protocol.js";

const KAR98K_GAIN = 0.25;
const MG_BURST_GAIN = 0.7;
const MORTAR_LAUNCH_GAIN = 0.85;
const ARTILLERY_FIRE_GAIN = 1.2;

const COMBAT_SOUNDS = Object.freeze({
  [KIND.TANK]: {
    ids: ["combat_tank_01"],
    priority: 4,
    gain: 2,
  },
  [KIND.SCOUT_CAR]: {
    ids: ["combat_mg_burst_02", "combat_mg_burst_03"],
    priority: 2.5,
    gain: MG_BURST_GAIN,
  },
  [KIND.RIFLEMAN]: {
    ids: ["combat_rifle_02", "combat_rifle_03"],
    priority: 2,
    gain: KAR98K_GAIN,
  },
  [KIND.ANTI_TANK_GUN]: {
    ids: ["combat_tank_01"],
    priority: 4,
    gain: 2,
  },
  [KIND.MACHINE_GUNNER]: {
    ids: ["combat_mg_burst_02", "combat_mg_burst_03"],
    priority: 2.5,
    gain: MG_BURST_GAIN,
  },
});

const POINT_FIRE_SOUNDS = Object.freeze({
  [EVENT.MORTAR_LAUNCH]: {
    id: "combat_mortar_launch_04",
    priority: 3.5,
    gain: MORTAR_LAUNCH_GAIN,
  },
  [EVENT.ARTILLERY_TARGET]: {
    id: "combat_artillery_fire_05",
    priority: 4.5,
    gain: ARTILLERY_FIRE_GAIN,
  },
});

export class MatchCombatAudio {
  constructor({ audio, state }) {
    this.audio = audio;
    this.state = state;
    this.missingCombatSoundKinds = new Set();
    this.activeMachineGunSoundKeys = new Map();
  }

  playAttackSound(ev) {
    if (!this.audio) return;
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const to = typeof ev.to === "number" ? this.state.entityById(ev.to) : null;
    const pos = from || to;
    if (!pos || typeof pos.x !== "number" || typeof pos.y !== "number") return;

    const kind = from?.kind || KIND.RIFLEMAN;
    if (!attackKindHasCombatSound(kind)) return;
    let spec = COMBAT_SOUNDS[kind];
    if (!spec) {
      spec = COMBAT_SOUNDS[KIND.RIFLEMAN];
      if (!this.missingCombatSoundKinds.has(kind)) {
        this.missingCombatSoundKinds.add(kind);
        console.warn(`audio: missing combat sound mapping for ${kind}, using rifle`);
      }
    }
    const id = this.audio.pickVariant(spec.ids);
    if (!id) return;
    const category = from && from.owner === this.state.playerId ? "combat_self" : "combat_other";
    const key =
      kind === KIND.MACHINE_GUNNER && typeof ev.from === "number"
        ? machineGunSoundKey(ev.from)
        : undefined;
    const played = this.audio.play(id, {
      x: pos.x,
      y: pos.y,
      category,
      priority: spec.priority,
      gain: spec.gain,
      key,
    });
    if (played && key) this.activeMachineGunSoundKeys.set(ev.from, key);
  }

  playPointFireSound(ev) {
    if (!this.audio) return;
    const spec = POINT_FIRE_SOUNDS[ev.e];
    if (!spec) return;
    let pos = null;
    if (ev.e === EVENT.MORTAR_LAUNCH && Number.isFinite(ev.fromX) && Number.isFinite(ev.fromY)) {
      pos = { x: ev.fromX, y: ev.fromY };
    } else if (ev.e === EVENT.ARTILLERY_TARGET && typeof ev.from === "number") {
      const from = this.state.entityById(ev.from);
      if (from && Number.isFinite(from.x) && Number.isFinite(from.y)) pos = from;
    }
    if (!pos) return;
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const category = from && from.owner === this.state.playerId ? "combat_self" : "combat_other";
    this.audio.play(spec.id, {
      x: pos.x,
      y: pos.y,
      category,
      priority: spec.priority,
      gain: spec.gain,
    });
  }

  stopInactiveMachineGunSounds() {
    if (!this.audio || this.activeMachineGunSoundKeys.size === 0) return;
    for (const [id, key] of this.activeMachineGunSoundKeys) {
      if (machineGunnerHasAudibleTarget(this.state.entityById(id))) continue;
      this.audio.stopByKey(key);
      this.activeMachineGunSoundKeys.delete(id);
    }
  }

  stopAllMachineGunSounds() {
    if (!this.audio) {
      this.activeMachineGunSoundKeys.clear();
      return;
    }
    for (const key of this.activeMachineGunSoundKeys.values()) {
      this.audio.stopByKey(key);
    }
    this.activeMachineGunSoundKeys.clear();
  }
}
