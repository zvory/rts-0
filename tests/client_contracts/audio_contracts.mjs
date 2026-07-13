// tests/client_contracts/audio_contracts.mjs
// Domain contract assertions imported by ../client_contracts.mjs.

import {
  assert,
  assertApprox,
  assertHasMethod,
} from "./assertions.mjs";
import {
  Audio,
  SOUND_MANIFEST,
  noticeSoundId,
} from "../../client/src/audio.js";
import {
  PANZERFAUST_IMPACT_SOUND_ID,
  PANZERFAUST_LAUNCH_SOUND_ID,
  attackFeedbackKind,
  attackKindHasCombatSound,
  defaultWeaponKindForAttackerKind,
  machineGunnerHasAudibleTarget,
  panzerfaustFeedbackDedupKey,
  panzerfaustFeedbackSoundId,
} from "../../client/src/combat_audio.js";
import { MatchCombatAudio } from "../../client/src/match_combat_audio.js";
import {
  EVENT,
  KIND,
  SETUP,
  STATE,
  WEAPON_KIND,
} from "../../client/src/protocol.js";

function fakeAudioParam(value = 1) {
  return {
    value,
    ramps: [],
    cancelScheduledValues() {},
    setValueAtTime(v) { this.value = v; },
    linearRampToValueAtTime(v, time) {
      this.value = v;
      this.ramps.push({ value: v, time });
    },
  };
}

class FakeAudioNode {
  connect() { return this; }
  disconnect() {}
}

class FakeBufferSource extends FakeAudioNode {
  constructor() {
    super();
    this.playbackRate = fakeAudioParam(1);
    this.buffer = null;
    this.onended = null;
    this.started = false;
    this.stopped = false;
  }
  start() {
    this.started = true;
  }
  stop() {
    this.stopped = true;
    if (this.onended) this.onended();
  }
}

function fakeGain() {
  const node = new FakeAudioNode();
  node.gain = fakeAudioParam(1);
  return node;
}

function fakeAudioContext() {
  return {
    state: "running",
    currentTime: 0,
    createBufferSource() { return new FakeBufferSource(); },
    createStereoPanner() {
      const node = new FakeAudioNode();
      node.pan = fakeAudioParam(0);
      return node;
    },
    createBiquadFilter() {
      const node = new FakeAudioNode();
      node.type = "";
      node.frequency = fakeAudioParam(0);
      return node;
    },
    createGain: fakeGain,
    close() {},
  };
}

assert(noticeSoundId("alert:under_attack") === "notice_under_attack", "under-attack notice has dedicated sound id");
assert(noticeSoundId("Not enough supply") === "notice_supply", "supply notice routes to supply voice line");
assert(noticeSoundId("Build more depots") === "notice_supply", "depot notice routes to supply voice line");
assert(noticeSoundId("Not enough steel") === "notice_steel", "steel notice routes to steel voice line");
assert(noticeSoundId("Not enough oil") === "notice_oil", "oil notice routes to oil voice line");
assert(noticeSoundId("Cannot build there") === "notice_cannot_build", "cannot-build notice routes to cannot-build voice line");
assert(noticeSoundId("Requirement not met") === null, "generic invalid notices stay silent");
assert(noticeSoundId("Unknown unit") === null, "unknown-unit notices stay silent");
assert(noticeSoundId("Not enough resources") === null, "generic resource notices stay silent");
assert(
  ["countdown_drei", "countdown_zwei", "countdown_eins"].every((id) =>
    SOUND_MANIFEST.some((entry) => entry.id === id && entry.url.startsWith("/assets/sound/ui/"))
  ),
  "countdown voice cues are exposed through the shared sound manifest",
);
assert(
  [PANZERFAUST_LAUNCH_SOUND_ID, PANZERFAUST_IMPACT_SOUND_ID].every((id) =>
    SOUND_MANIFEST.some((entry) => entry.id === id && entry.url.startsWith("/assets/sound/combat/"))
  ),
  "Panzerfaust combat cues are exposed through the shared sound manifest",
);

// Audio
// ---------------------------------------------------------------------------
{
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const priorLocalStorage = globalThis.localStorage;
  globalThis.window = {
    addEventListener() {},
    removeEventListener() {},
  };
  globalThis.document = {
    hidden: false,
    addEventListener() {},
    removeEventListener() {},
  };
  globalThis.localStorage = {
    getItem() { return null; },
    setItem() {},
  };

  const audio = new Audio();
  assertHasMethod(audio, "play", "Audio");
  assertHasMethod(audio, "playUI", "Audio");
  assertHasMethod(audio, "stopByKey", "Audio");
  assertHasMethod(audio, "preload", "Audio");
  assertHasMethod(audio, "setListener", "Audio");
  assertHasMethod(audio, "pickVariant", "Audio");
  audio.setListener({ x: 100, y: 100, referenceDistancePx: 400 });
  assertApprox(audio.listener.refDist, 400, 0.001, "Audio listener consumes semantic reference distance");

  const near = audio._computeSpatial(300, 100);
  assert(near !== null, "Audio spatial near emitter should play");
  assertApprox(near.gain, 1, 0.001, "Audio spatial gain is flat inside refDist");
  assertApprox(near.pan, 0.5, 0.001, "Audio spatial pan uses dx/refDist");

  const mid = audio._computeSpatial(900, 100);
  assert(mid !== null, "Audio spatial off-viewport emitter should play");
  assertApprox(mid.gain, 1 / 5, 0.001, "Audio spatial gain quadruples far-distance attenuation");

  const far = audio._computeSpatial(1300, 100);
  assert(far !== null, "Audio spatial max-distance edge should play");
  assertApprox(far.gain, 1 / 9, 0.001, "Audio spatial gain attenuates harder at maxDist");
  assertApprox(far.lpHz, 1200, 0.001, "Audio spatial lowpass reaches far cutoff");
  assert(audio._computeSpatial(1301, 100) === null, "Audio drops sounds beyond maxDist");

  const combatNear = audio._computeSpatial(260, 100, "combat_self");
  assert(combatNear !== null, "combat emitter at near boundary should play");
  assertApprox(combatNear.gain, 1, 0.001, "combat stays full gain through 0.4 reference distance");
  assertApprox(combatNear.lpHz, 20000, 0.001, "combat near region keeps the near lowpass cutoff");
  assertApprox(combatNear.distancePenalty, 0, 0.001, "combat near region has no distance priority penalty");

  const combatHalf = audio._computeSpatial(300, 100, "combat_other");
  assert(combatHalf !== null, "combat emitter at half reference distance should play");
  assertApprox(combatHalf.gain, 0.5, 0.001, "combat gain is 0.5 at half reference distance");

  const combatOneRef = audio._computeSpatial(500, 100, "combat_self");
  assert(combatOneRef !== null, "combat emitter at one reference distance should play");
  assertApprox(combatOneRef.gain, 1 / 7, 0.001, "combat gain is about 0.143 at one reference distance");
  assert(
    combatOneRef.lpHz > 1200,
    "combat lowpass remains above its far cutoff before the hard-drop boundary",
  );

  const combatEdge = audio._computeSpatial(580, 100, "combat_other");
  assert(combatEdge !== null, "combat emitter at hard-drop boundary should play");
  assertApprox(combatEdge.lpHz, 1200, 0.001, "combat lowpass reaches far cutoff at hard-drop boundary");
  assertApprox(combatEdge.distancePenalty, 30, 0.001, "combat priority penalty reaches 30 at boundary");
  assert(
    combatHalf.distancePenalty > combatNear.distancePenalty &&
      combatOneRef.distancePenalty > combatHalf.distancePenalty &&
      combatEdge.distancePenalty > combatOneRef.distancePenalty,
    "combat distance priority penalty rises monotonically outside the near region",
  );
  assert(
    audio._computeSpatial(581, 100, "combat_self") === null,
    "Audio drops combat beyond 1.2 reference distances",
  );

  const defaultAtCombatDrop = audio._computeSpatial(581, 100, "ambient");
  assert(defaultAtCombatDrop !== null, "default non-combat spatial profile keeps its original reach");
  assertApprox(
    defaultAtCombatDrop.gain,
    400 / (400 + 81 * 4),
    0.001,
    "default non-combat attenuation remains unchanged",
  );

  const priorPerformance = globalThis.performance;
  let now = 0;
  globalThis.performance = { now: () => now };

  let stopped = 0;
  let disconnected = 0;
  const keyedVoice = (key) => ({
    key,
    node: {
      onended: () => {},
      stop() { stopped += 1; },
    },
    trail: [{ disconnect() { disconnected += 1; } }],
  });
  audio.voices = [keyedVoice("mg:1"), keyedVoice("other"), keyedVoice("mg:1")];
  assert(audio.stopByKey("mg:1") === 2, "Audio.stopByKey reports stopped voices");
  assert(stopped === 2, "Audio.stopByKey stops matching voices");
  assert(disconnected === 2, "Audio.stopByKey disconnects matching voice nodes");
  assert(
    audio.voices.length === 1 && audio.voices[0].key === "other",
    "Audio.stopByKey keeps unrelated voices active",
  );
  audio.voices = [];

  audio.ctx = fakeAudioContext();
  audio.master = fakeGain();
  audio.gains = {
    ui: fakeGain(),
    alert: fakeGain(),
    combat_self: fakeGain(),
    combat_other: fakeGain(),
    unit_voice: fakeGain(),
    ambient: fakeGain(),
  };
  for (const [cat, gain] of Object.entries(audio.gains)) {
    gain.gain.value = audio.getCategoryVolume(cat);
  }

  audio.buffers.set("moving_combat", { duration: 0.1 });
  audio.setListener({ x: 100, y: 100, referenceDistancePx: 400 });
  assert(
    audio.play("moving_combat", {
      x: 300,
      y: 100,
      category: "combat_self",
      pitchVariance: 0,
    }),
    "active combat voice starts with the combat spatial profile",
  );
  const movingCombat = audio.voices.find((voice) => voice.id === "moving_combat");
  assert(movingCombat?.spatial?.category === "combat_self", "active voice remembers its combat category");
  audio.setListener({ x: 140, y: 100, referenceDistancePx: 400 });
  assertApprox(
    movingCombat.spatial.distGain.gain.ramps.at(-1).value,
    1,
    0.001,
    "listener refresh recomputes the combat near-region gain",
  );
  assertApprox(
    movingCombat.spatial.lp.frequency.ramps.at(-1).value,
    20000,
    0.001,
    "listener refresh recomputes the combat lowpass with the same profile",
  );
  assertApprox(movingCombat.distancePenalty, 0, 0.001, "listener refresh recomputes combat priority");
  assertApprox(
    movingCombat.spatial.distGain.gain.ramps.at(-1).time,
    0.03,
    0.001,
    "listener refresh preserves the smooth spatial ramp",
  );
  movingCombat.node.stop();

  for (let i = 0; i < 200; i++) audio.buffers.set(`pool_${i}`, { duration: 0.1 });
  for (let i = 0; i < 120; i++) {
    audio.play(`pool_${i}`, { category: "ambient" });
    assert(audio.voices.length <= 48, "ambient voice pool stays capped");
    now += 1;
  }
  for (let i = 120; i < 200; i++) {
    audio.play(`pool_${i}`, { category: "alert" });
    assert(audio.voices.length <= 48, "alert voice pool stays capped");
    now += 1;
  }
  assert(audio.voices.length <= 48, "Audio voice pool stays capped");
  assert(audio.voices.every((v) => v.category === "alert"), "Audio priority eviction keeps highest-priority voices");

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("notice_under_attack", { duration: 0.5 });
  now = 10_000;
  assert(
    audio.play("notice_under_attack", {
      category: "alert",
      alertId: "under_attack",
      alertX: 100,
      alertY: 100,
      cooldownMs: 0,
      duck: true,
    }),
    "first under-attack alert plays",
  );
  now += 1000;
  assert(
    audio.play("notice_under_attack", {
      category: "alert",
      alertId: "under_attack",
      alertX: 2000,
      alertY: 100,
      cooldownMs: 0,
      duck: true,
    }),
    "presenter-admitted under-attack voices bypass generic spoken cooldown",
  );

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("notice_supply", { duration: 2.3 });
  now = 30_000;
  assert(audio.play("notice_supply", { category: "alert" }), "first spoken alert plays");
  now += 1500;
  assert(!audio.play("notice_supply", { category: "alert" }), "spoken alert cooldown honors buffer duration");
  now += 801;
  assert(audio.play("notice_supply", { category: "alert" }), "spoken alert plays after buffer-duration cooldown");

  audio.voices.slice().forEach((v) => v.node.stop());
  audio.buffers.set("duck_notice_a", { duration: 0.1 });
  audio.buffers.set("duck_notice_b", { duration: 0.1 });
  audio.buffers.set("duck_alert", { duration: 0.1 });
  now = 40_000;
  const ambientBefore = audio.gains.ambient.gain.value;
  const combatBefore = audio.gains.combat_self.gain.value;
  assert(audio.play("duck_notice_a", { category: "ui", duck: true }), "explicit ducking notice plays");
  assertApprox(
    audio.gains.ambient.gain.value,
    ambientBefore * Math.pow(10, -12 / 20),
    0.0001,
    "ducking voice lowers ambient by 12 dB",
  );
  assertApprox(
    audio.gains.combat_self.gain.value,
    combatBefore * Math.pow(10, -10 / 20),
    0.0001,
    "ducking voice lowers combat by 10 dB",
  );
  assertApprox(
    audio.gains.combat_self.gain.ramps.at(-1).time,
    0.08,
    0.0001,
    "combat duck attacks over 80 milliseconds",
  );
  audio.setCategoryVolume("ambient", 0.6);
  audio.setCategoryVolume("combat_self", 0.7);
  assertApprox(
    audio.gains.ambient.gain.value,
    0.6 * Math.pow(10, -12 / 20),
    0.0001,
    "ambient slider changes preserve an active duck",
  );
  assertApprox(
    audio.gains.combat_self.gain.value,
    0.7 * Math.pow(10, -10 / 20),
    0.0001,
    "combat slider changes preserve an active duck",
  );
  assert(audio.play("duck_notice_b", { category: "ui", duck: true }), "overlapping ducking notice plays");
  assert(audio.alertDuckDepth === 2, "overlapping ducking voices increment duck depth");
  const firstDuck = audio.voices.find((voice) => voice.id === "duck_notice_a");
  const secondDuck = audio.voices.find((voice) => voice.id === "duck_notice_b");
  firstDuck.node.stop();
  assert(audio.alertDuckDepth === 1, "first completed voice does not restore buses early");
  audio.ctx.currentTime = 4;
  secondDuck.node.stop();
  assert(audio.alertDuckDepth === 0, "last completed voice releases duck depth");
  assertApprox(audio.gains.ambient.gain.value, audio.getCategoryVolume("ambient"), 0.0001, "ambient bus restores");
  assertApprox(audio.gains.combat_self.gain.value, audio.getCategoryVolume("combat_self"), 0.0001, "combat bus restores");
  assertApprox(
    audio.gains.combat_self.gain.ramps.at(-1).time,
    6,
    0.0001,
    "combat bus restores over two seconds",
  );
  assert(audio.play("duck_alert", { category: "alert" }), "alert category still ducks by default");
  assert(audio.alertDuckDepth === 1, "default alert duck participates in depth tracking");
  audio.voices.slice().forEach((v) => v.node.stop());

  audio.destroy();
  globalThis.window = priorWindow;
  globalThis.document = priorDocument;
  globalThis.localStorage = priorLocalStorage;
  globalThis.performance = priorPerformance;
}

// ---------------------------------------------------------------------------
// Combat audio
// ---------------------------------------------------------------------------
{
  assert(
    machineGunnerHasAudibleTarget({
      kind: KIND.MACHINE_GUNNER,
      state: STATE.MOVE,
      setupState: SETUP.TEARING_DOWN,
      targetId: 7,
    }),
    "MG combat loop stays active while the machine gunner still has a target",
  );
  assert(
    !machineGunnerHasAudibleTarget({
      kind: KIND.MACHINE_GUNNER,
      state: STATE.ATTACK,
      setupState: SETUP.DEPLOYED,
    }),
    "MG combat loop stops once the machine gunner has no target",
  );
  assert(
    !machineGunnerHasAudibleTarget({
      kind: KIND.RIFLEMAN,
      targetId: 7,
    }),
    "non-MG targets do not hold the MG combat loop",
  );
  assert(
    !attackKindHasCombatSound(KIND.WORKER),
    "worker attacks are silent instead of falling back to rifle shots",
  );
  assert(attackKindHasCombatSound(KIND.RIFLEMAN), "rifleman attacks still play combat sounds");
  assert(
    defaultWeaponKindForAttackerKind(KIND.TANK) === WEAPON_KIND.TANK_CANNON,
    "tank default attack resolves to the tank cannon weapon id",
  );
  assert(
    attackFeedbackKind(KIND.TANK, WEAPON_KIND.TANK_CANNON) === KIND.TANK,
    "default tank cannon weapon hint preserves tank feedback",
  );
  assert(
    attackFeedbackKind(KIND.TANK, WEAPON_KIND.TANK_COAX) === KIND.MACHINE_GUNNER,
    "tank coax weapon hint maps to machine-gun feedback",
  );
  assert(
    attackKindHasCombatSound(KIND.TANK, WEAPON_KIND.TANK_COAX),
    "tank coax attack events play machine-gun combat sound",
  );
  assert(
    defaultWeaponKindForAttackerKind(KIND.PANZERFAUST) === WEAPON_KIND.PANZERFAUST_LOADED_SHOT,
    "Panzerfaust attack feedback resolves to its loaded-shot weapon id",
  );
  assert(
    attackFeedbackKind(KIND.PANZERFAUST, WEAPON_KIND.PANZERFAUST_LOADED_SHOT) === KIND.PANZERFAUST,
    "Panzerfaust loaded-shot weapon hint preserves Panzerfaust feedback",
  );
  assert(
    !attackKindHasCombatSound(KIND.PANZERFAUST, WEAPON_KIND.PANZERFAUST_LOADED_SHOT),
    "generic Panzerfaust attack events stay silent instead of reusing rifle or tank sounds",
  );
  assert(
    panzerfaustFeedbackSoundId(EVENT.PANZERFAUST_LAUNCH) === PANZERFAUST_LAUNCH_SOUND_ID,
    "Panzerfaust launch events map to a dedicated launch cue",
  );
  assert(
    panzerfaustFeedbackSoundId(EVENT.PANZERFAUST_IMPACT) === PANZERFAUST_IMPACT_SOUND_ID,
    "Panzerfaust impact events map to a dedicated impact cue",
  );
  assert(
    panzerfaustFeedbackSoundId(EVENT.PANZERFAUST_CONVERSION) === null,
    "Legacy Panzerfaust conversion stays silent",
  );
  assert(
    ![
      "combat_tank_01",
      "combat_rifle_02",
      "combat_rifle_03",
      "combat_mg_burst_02",
      "combat_mg_burst_03",
      "combat_artillery_fire_05",
    ].includes(panzerfaustFeedbackSoundId(EVENT.PANZERFAUST_LAUNCH)),
    "Panzerfaust launch does not reuse tank, rifle, machine-gun, or artillery cues",
  );
  assert(
    panzerfaustFeedbackDedupKey(EVENT.PANZERFAUST_LAUNCH, 320, 384, "combat_self") ===
      "panzerfaust_launch:combat_self:1:2",
    "Panzerfaust launch dedup buckets nearby grouped shots",
  );
  assert(
    attackFeedbackKind(KIND.RIFLEMAN, "future_unknown_weapon") === KIND.RIFLEMAN,
    "unknown attack weapon hints preserve attacker-kind feedback",
  );
}

{
  const plays = [];
  const entities = new Map([
    [21, { id: 21, owner: 1, kind: KIND.TANK, x: 220, y: 240 }],
    [22, { id: 22, owner: 2, kind: KIND.WORKER, x: 280, y: 240 }],
  ]);
  const combatAudio = new MatchCombatAudio({
    state: {
      playerId: 1,
      entityById: (id) => entities.get(id) || null,
    },
    audio: {
      pickVariant: (ids) => ids[0],
      play(id, opts) {
        plays.push({ id, opts });
        return true;
      },
      stopByKey() {},
    },
  });

  combatAudio.playAttackSound({
    e: EVENT.ATTACK,
    from: 21,
    to: 22,
    weaponKind: WEAPON_KIND.TANK_COAX,
  });

  assert(plays[0].id === "combat_mg_burst_02", "tank coax uses the machine-gun burst cue");
  assert(plays[0].opts.category === "combat_self", "own tank coax cue uses the self combat bus");
  assert(!plays[0].opts.key, "tank coax bursts do not start the sustained MG loop key");
  assert(
    combatAudio.activeMachineGunSoundKeys.size === 0,
    "tank coax audio does not register as a persistent machine-gunner loop",
  );
}

{
  const plays = [];
  const entities = new Map([
    [31, { id: 31, owner: 1, kind: KIND.PANZERFAUST, x: 300, y: 340 }],
  ]);
  const combatAudio = new MatchCombatAudio({
    state: {
      playerId: 1,
      entityById: (id) => entities.get(id) || null,
    },
    audio: {
      pickVariant: (ids) => ids[0],
      play(id, opts) {
        plays.push({ id, opts });
        return true;
      },
      stopByKey() {},
    },
  });

  combatAudio.playPointFireSound({
    e: EVENT.PANZERFAUST_LAUNCH,
    from: 31,
    fromX: 300,
    fromY: 340,
    toX: 352,
    toY: 340,
  });
  assert(plays[0].id === PANZERFAUST_LAUNCH_SOUND_ID, "match combat audio routes Panzerfaust launches");
  assert(plays[0].opts.x === 300 && plays[0].opts.y === 340, "Panzerfaust launch audio uses the projected launch point");
  assert(plays[0].opts.category === "combat_self", "own visible Panzerfaust launches use the self combat bus");
  assert(plays[0].opts.cooldownMs >= 120, "Panzerfaust launch audio applies an anti-spam cooldown");
  assert(
    plays[0].opts.dedupKey.startsWith("panzerfaust_launch:combat_self:"),
    "Panzerfaust launch audio uses a coarse spatial dedup bucket",
  );

  combatAudio.playPointFireSound({ e: EVENT.PANZERFAUST_IMPACT, x: 352, y: 340 });
  assert(plays.at(-1).id === PANZERFAUST_IMPACT_SOUND_ID, "match combat audio routes Panzerfaust impacts");
  assert(plays.at(-1).opts.x === 352 && plays.at(-1).opts.y === 340, "Panzerfaust impact audio uses the projected impact point");
  assert(plays.at(-1).opts.category === "combat_other", "Panzerfaust impacts without a visible source avoid claiming self ownership");
  assert(plays.at(-1).opts.gain < plays[0].opts.gain, "Panzerfaust impact cue is quieter than the launch cue");

  const playCount = plays.length;
  combatAudio.playPointFireSound({ e: EVENT.PANZERFAUST_CONVERSION, id: 31 });
  assert(plays.length === playCount, "Legacy Panzerfaust conversion does not play a combat cue");
}

// ---------------------------------------------------------------------------
