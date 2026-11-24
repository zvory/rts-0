import { ABILITY, ABILITY_OBJECT_KIND } from "./protocol.js";

// Assets share the combat mix. Anchor is mastered 4 dB below the cast cues.
const GAIN = 0.7;

/** Uses only authoritative, recipient-visible objects and ability affordances. */
export class MatchAbilityAudio {
  constructor({ audio, categoryForOwner }) {
    this.audio = audio;
    this.categoryForOwner = categoryForOwner;
    this.anchors = new Map();
    this.shields = new Set();
    this.dashCharges = new Map();
  }

  update(objects = [], entities = [], active = true) {
    if (!this.audio) return;
    const anchors = new Set(), shields = new Set(), charges = new Map();
    for (const object of objects) {
      if (!Number.isFinite(object.x) || !Number.isFinite(object.y)) continue;
      const category = this.categoryForOwner(object.owner);
      if (object.ability === ABILITY.DISCIPLE_SHIELD && object.kind === ABILITY_OBJECT_KIND.SHIELD) {
        shields.add(object.id);
        if (active && !this.shields.has(object.id)) {
          this.audio.play("combat_magic_shield_01", {
            x: object.x, y: object.y, category, gain: GAIN, priority: 3,
            pitchVariance: 0, cooldownMs: 120,
          });
        }
      }
      if (object.ability !== ABILITY.EKAT_MAGIC_ANCHOR
        || object.kind !== ABILITY_OBJECT_KIND.MAGIC_ANCHOR) continue;
      anchors.add(object.id);
      if (!active) continue;
      const key = `combat:magic_anchor:${object.id}`;
      if (this.anchors.has(object.id) && this.audio.hasVoiceKey?.(key) !== false) {
        this.audio.setVoicePosition?.(key, object.x, object.y);
      } else if (this.audio.play("combat_magic_anchor_01", {
        key, x: object.x, y: object.y, category, gain: GAIN, priority: 1,
        loop: true, pitchVariance: 0, cooldownMs: 0, fadeInMs: 80,
      })) this.anchors.set(object.id, key);
    }
    for (const [id, key] of this.anchors) {
      if (active && anchors.has(id)) continue;
      this.audio.stopByKey(key);
      this.anchors.delete(id);
    }
    for (const entity of entities) {
      if (entity.shotReveal || entity.visionOnly || entity.hp <= 0
        || !Number.isFinite(entity.x) || !Number.isFinite(entity.y)) continue;
      const dash = entity.abilities?.find(a => a.ability === ABILITY.WIND_DASH);
      if (!Number.isInteger(dash?.remainingUses)) continue;
      charges.set(entity.id, dash.remainingUses);
      // Decreased authoritative charges confirm a successful cast, not an input attempt.
      // Missing owner-only metadata (including enemies) never infers a cast from movement.
      const previous = this.dashCharges.get(entity.id);
      if (active && previous !== undefined && dash.remainingUses < previous) {
        this.audio.play("combat_lightness_dash_01", {
          x: entity.x, y: entity.y, category: this.categoryForOwner(entity.owner),
          gain: GAIN, priority: 3, pitchVariance: 0, cooldownMs: 80,
        });
      }
    }
    this.shields = shields;
    this.dashCharges = charges;
  }

  stop({ reset = false } = {}) {
    for (const key of this.anchors.values()) this.audio?.stopByKey(key);
    this.anchors.clear();
    if (reset) {
      this.shields.clear();
      this.dashCharges.clear();
    }
  }

  destroy() { this.stop({ reset: true }); }
}
