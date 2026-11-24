import { assert } from "./assertions.mjs";
import { MatchAbilityAudio } from "../../client/src/match_ability_audio.js";
import { ABILITY, ABILITY_OBJECT_KIND } from "../../client/src/protocol.js";

const plays = [], stops = [], voices = new Set();
let admitted = true;
const audio = {
  play(id, opts) { plays.push({ id, opts }); if (admitted) voices.add(opts.key); return admitted; },
  stopByKey(key) { stops.push(key); voices.delete(key); },
  hasVoiceKey: key => voices.has(key),
  setVoicePosition() {},
};
const sounds = new MatchAbilityAudio({ audio, categoryForOwner: owner => owner === 1 ? "combat_self" : "combat_other" });
const anchor = { id: 1, ability: ABILITY.EKAT_MAGIC_ANCHOR, kind: ABILITY_OBJECT_KIND.MAGIC_ANCHOR, owner: 2, x: 100, y: 200 };
const shield = { id: 2, ability: ABILITY.DISCIPLE_SHIELD, kind: ABILITY_OBJECT_KIND.SHIELD, owner: 1, x: 200, y: 300 };
const caster = remainingUses => ({ id: 3, owner: 1, hp: 100, x: 50, y: 60,
  abilities: [{ ability: ABILITY.WIND_DASH, remainingUses }] });
sounds.update([anchor, shield], [caster(2)]);
assert(plays.length === 2 && plays[0].opts.loop && plays[0].opts.category === "combat_other",
  "visible enemy anchor sustains on the spatial other bus");
assert(plays[1].id === "combat_magic_shield_01" && !plays[1].opts.loop,
  "shield is a single placement cue");
sounds.update([anchor, shield], [caster(1)]);
assert(plays.length === 3 && plays[2].id === "combat_lightness_dash_01",
  "successful authoritative dash charge use plays Silk Gust");
sounds.update([anchor, shield], [caster(2)]);
assert(plays.length === 3, "normal snapshots and recharge do not replay cast cues");
voices.clear();
sounds.update([anchor, shield], [caster(2)]);
assert(plays.length === 4, "evicted anchor resumes without replaying shield");
sounds.stop();
sounds.update([anchor, shield], [caster(1)], false);
assert(!voices.size && plays.length === 4, "paused snapshots stay silent and preserve current cast baselines");
sounds.update([anchor, shield], [caster(1)]);
assert(plays.length === 5 && plays.at(-1).id === "combat_magic_anchor_01",
  "resume restarts the sustain without replaying dash or shield");
sounds.update([], []);
assert(!voices.size && stops.includes("combat:magic_anchor:1"), "expiry or lost visibility immediately stops anchor");
sounds.update([{ ...anchor, x: NaN }], [{ ...caster(0), shotReveal: true }]);
sounds.update([], [{ ...caster(0), abilities: undefined }]);
assert(plays.length === 5, "invalid objects and reveal-only/missing charge metadata never invent casts");
admitted = false;
sounds.update([anchor], []);
admitted = true;
sounds.update([anchor], []);
assert(voices.size === 1, "a temporarily inaudible anchor can become audible later");
sounds.destroy();
assert(!voices.size && !sounds.shields.size && !sounds.dashCharges.size,
  "teardown clears voices and timeline baselines");
sounds.update([shield], [caster(0)]);
assert(plays.at(-1).id === "combat_magic_shield_01", "new timeline permits new shield cast cues");
sounds.destroy();
