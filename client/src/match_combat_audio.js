import {
  attackFeedbackKind,
  attackKindHasCombatSound,
  machineGunnerHasAudibleTarget,
  machineGunSoundKey,
  panzerfaustFeedbackDedupKey,
  panzerfaustFeedbackSoundId,
} from "./combat_audio.js";
import { TICK_HZ } from "./config.js";
import { EVENT, KIND } from "./protocol.js";

const KAR98K_GAIN = 0.25;
const MG_BURST_GAIN = 0.7;
const MORTAR_LAUNCH_GAIN = 0.85;
const ARTILLERY_FIRE_GAIN = 1.2;
const ARTILLERY_LANDING_GAIN = 1;
const PANZERFAUST_LAUNCH_GAIN = 0.52;
const PANZERFAUST_IMPACT_GAIN = 0.42;
const WORLD_COMBAT_BED_ID = "combat_distant_bed_01";
const WORLD_COMBAT_BED_KEY = "combat:world_activity_bed";
const WORLD_COMBAT_BED_GAIN = 0.035;
const WORLD_COMBAT_BED_FADE_IN_MS = 750;
const WORLD_COMBAT_BED_FADE_OUT_MS = 2500;

// Measured from the decoded production asset's first impact transient. Keep the
// cue's explosion aligned to the authoritative ARTILLERY_IMPACT event.
const ARTILLERY_LANDING_LEAD_MS = 2808.322;

export function worldCombatBedAllowed(active, livePauseState, roomTimeState) {
  if (active !== true || livePauseState?.paused === true) return false;
  const speed = Number(roomTimeState?.speed);
  const duration = Number(roomTimeState?.durationTicks);
  const current = Number(roomTimeState?.currentTick);
  return roomTimeState?.paused !== true
    && (!Number.isFinite(speed) || speed > 0)
    && roomTimeState?.ended !== true
    && !(duration > 0 && current >= duration);
}

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

const POSITIONAL_EVENT_SOUNDS = Object.freeze({
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
  [EVENT.PANZERFAUST_LAUNCH]: {
    id: panzerfaustFeedbackSoundId(EVENT.PANZERFAUST_LAUNCH),
    priority: 3.25,
    gain: PANZERFAUST_LAUNCH_GAIN,
    cooldownMs: 140,
    pitchVariance: 0.035,
    panzerfaustDedup: true,
  },
  [EVENT.PANZERFAUST_IMPACT]: {
    id: panzerfaustFeedbackSoundId(EVENT.PANZERFAUST_IMPACT),
    priority: 3,
    gain: PANZERFAUST_IMPACT_GAIN,
    cooldownMs: 120,
    pitchVariance: 0.03,
    panzerfaustDedup: true,
  },
});

export class MatchCombatAudio {
  constructor({
    audio,
    state,
    controlPolicy = null,
    setTimer = globalThis.setTimeout.bind(globalThis),
    clearTimer = globalThis.clearTimeout.bind(globalThis),
  }) {
    this.audio = audio;
    this.state = state;
    this.controlPolicy = controlPolicy;
    this.setTimer = setTimer;
    this.clearTimer = clearTimer;
    this.missingCombatSoundKinds = new Set();
    this.activeMachineGunSoundKeys = new Map();
    this.pendingArtilleryLandingTimers = new Set();
    this.worldCombatBedPlaying = false;
  }

  updateWorldCombatBed(position) {
    if (!this.audio) return;
    const x = Number(position?.[0]);
    const y = Number(position?.[1]);
    const active = Number.isFinite(x) && Number.isFinite(y);
    if (active) {
      if (
        this.worldCombatBedPlaying
        && this.audio.hasVoiceKey?.(WORLD_COMBAT_BED_KEY) !== false
      ) {
        this.audio.setVoicePosition?.(WORLD_COMBAT_BED_KEY, x, y);
        return;
      }
      // A prior fade-out may still own the key when combat resumes.
      this.audio.stopByKey(WORLD_COMBAT_BED_KEY);
      this.worldCombatBedPlaying = this.audio.play(WORLD_COMBAT_BED_ID, {
        category: "combat_other",
        priority: -20,
        gain: WORLD_COMBAT_BED_GAIN,
        key: WORLD_COMBAT_BED_KEY,
        loop: true,
        x,
        y,
        directionalOnly: true,
        fadeInMs: WORLD_COMBAT_BED_FADE_IN_MS,
        pitchVariance: 0,
        cooldownMs: 0,
      });
      return;
    }
    if (!this.worldCombatBedPlaying) return;
    this.audio.stopByKey(WORLD_COMBAT_BED_KEY, { fadeOutMs: WORLD_COMBAT_BED_FADE_OUT_MS });
    this.worldCombatBedPlaying = false;
  }

  playAttackSound(ev) {
    if (!this.audio) return;
    if (typeof ev.from === "number" && ev.from === ev.to) return;
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const to = typeof ev.to === "number" ? this.state.entityById(ev.to) : null;
    const pos = from || to;
    if (!pos || typeof pos.x !== "number" || typeof pos.y !== "number") return;

    const kind = from?.kind || KIND.RIFLEMAN;
    const feedbackKind = attackFeedbackKind(kind, ev.weaponKind);
    if (!attackKindHasCombatSound(kind, ev.weaponKind)) return;
    let spec = COMBAT_SOUNDS[feedbackKind];
    if (!spec) {
      spec = COMBAT_SOUNDS[KIND.RIFLEMAN];
      if (!this.missingCombatSoundKinds.has(feedbackKind)) {
        this.missingCombatSoundKinds.add(feedbackKind);
        console.warn(`audio: missing combat sound mapping for ${feedbackKind}, using rifle`);
      }
    }
    const id = this.audio.pickVariant(spec.ids);
    if (!id) return;
    const category = from && audioSelfOwner(this.state, from.owner, this.controlPolicy) ? "combat_self" : "combat_other";
    const key =
      from?.kind === KIND.MACHINE_GUNNER
        && feedbackKind === KIND.MACHINE_GUNNER
        && typeof ev.from === "number"
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
    if (ev?.e === EVENT.ARTILLERY_TARGET) this._scheduleArtilleryLanding(ev);
    const spec = POSITIONAL_EVENT_SOUNDS[ev.e];
    if (!spec) return;
    const pos = positionalEventSoundPosition(ev, this.state);
    if (!pos) return;
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const category = from && audioSelfOwner(this.state, from.owner, this.controlPolicy) ? "combat_self" : "combat_other";
    const opts = {
      x: pos.x,
      y: pos.y,
      category,
      priority: spec.priority,
      gain: spec.gain,
    };
    if (typeof spec.cooldownMs === "number") opts.cooldownMs = spec.cooldownMs;
    if (typeof spec.pitchVariance === "number") opts.pitchVariance = spec.pitchVariance;
    if (spec.panzerfaustDedup) {
      const dedupKey = panzerfaustFeedbackDedupKey(ev.e, pos.x, pos.y, category);
      if (dedupKey) opts.dedupKey = dedupKey;
    }
    this.audio.play(spec.id, opts);
  }

  _scheduleArtilleryLanding(ev) {
    if (!Number.isFinite(ev?.x) || !Number.isFinite(ev?.y)) return;
    const delayTicks = Number.isFinite(ev.delayTicks) ? Math.max(0, ev.delayTicks) : 0;
    const delayMs = Math.max(0, (delayTicks / TICK_HZ) * 1000 - ARTILLERY_LANDING_LEAD_MS);
    const from = typeof ev.from === "number" ? this.state.entityById(ev.from) : null;
    const category = from && audioSelfOwner(this.state, from.owner, this.controlPolicy) ? "combat_self" : "combat_other";
    let timer = null;
    timer = this.setTimer(() => {
      this.pendingArtilleryLandingTimers.delete(timer);
      this.audio?.play("combat_artillery_landing_01", {
        x: ev.x,
        y: ev.y,
        category,
        priority: 5,
        gain: ARTILLERY_LANDING_GAIN,
      });
    }, delayMs);
    this.pendingArtilleryLandingTimers.add(timer);
  }

  hasPointFireSound(eventKind) {
    return !!POSITIONAL_EVENT_SOUNDS[eventKind];
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

  destroy() {
    this.audio?.stopByKey(WORLD_COMBAT_BED_KEY);
    this.worldCombatBedPlaying = false;
    this.stopAllMachineGunSounds();
    for (const timer of this.pendingArtilleryLandingTimers) {
      this.clearTimer(timer);
    }
    this.pendingArtilleryLandingTimers.clear();
  }
}

function positionalEventSoundPosition(ev, state) {
  if (!ev) return null;
  if (ev.e === EVENT.MORTAR_LAUNCH && Number.isFinite(ev.fromX) && Number.isFinite(ev.fromY)) {
    return { x: ev.fromX, y: ev.fromY };
  }
  if (ev.e === EVENT.ARTILLERY_TARGET && typeof ev.from === "number") {
    const from = state.entityById(ev.from);
    if (from && Number.isFinite(from.x) && Number.isFinite(from.y)) return from;
  }
  if (
    ev.e === EVENT.PANZERFAUST_LAUNCH &&
    Number.isFinite(ev.fromX) &&
    Number.isFinite(ev.fromY)
  ) {
    return { x: ev.fromX, y: ev.fromY };
  }
  if (ev.e === EVENT.PANZERFAUST_IMPACT && Number.isFinite(ev.x) && Number.isFinite(ev.y)) {
    return { x: ev.x, y: ev.y };
  }
  return null;
}

function audioSelfOwner(state, owner, controlPolicy = null) {
  if (controlPolicy?.kind === "lab") {
    const feedbackOwner = typeof controlPolicy.feedbackOwner === "function"
      ? controlPolicy.feedbackOwner(state)
      : typeof controlPolicy.issueAsOwnerForSelection === "function"
        ? controlPolicy.issueAsOwnerForSelection(state.selectedEntities?.() || [])
        : null;
    const ownerId = Number(feedbackOwner);
    if (Number.isInteger(ownerId) && ownerId > 0) return Number(owner) === ownerId;
  }
  if (typeof state?.isOwnOwner === "function") return state.isOwnOwner(owner);
  return Number(owner) === state?.playerId;
}
