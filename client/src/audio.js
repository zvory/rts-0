// Audio — client-side sound engine. See docs/sound/PHASE_1.md and PHASE_2.md.
//
// Owns one AudioContext (lazily created on first user gesture, per browser policy),
// a category gain bus, a buffer cache keyed by sound id, and a capped voice pool.
// Wired in main.js via DI; no module other than main.js constructs this class.
//
// Invariants:
//   - `play()` is a no-op until a user gesture has unlocked the AudioContext.
//   - One AudioBuffer per id; never decode per playback.
//   - Voice pool is capped (VOICE_CAP); eviction is priority-based, not FIFO.
//   - Pitch variance uses a seeded RNG so audio is reproducible for replays.
//   - Spatial play (opts.x/y present) goes through StereoPanner + lowpass per-voice.
//     `setListener()` must be called each frame from main.js to keep distance math
//     in sync with the camera.

const CATEGORIES = Object.freeze([
  "ui",
  "alert",
  "combat_self",
  "combat_other",
  "unit_voice",
  "ambient",
]);
const CATEGORY_SET = new Set(CATEGORIES);

/** Per-category default volumes when no localStorage value is present. */
const DEFAULT_MASTER = 0.7;
const DEFAULT_AMBIENT = 0.4;
const DEFAULT_OTHER = 1.0;

/** Maximum concurrent voices. Beyond this, eviction kicks in. */
const VOICE_CAP = 48;
/** Drop repeats of the same `id` within this window (ms). */
const DEDUP_MS = 60;
/** Pitch jitter (±fraction) applied via playbackRate when caller does not override. */
const PITCH_VARIANCE = 0.06;
/** Master gain ramp time on tab show/hide (s). */
const VISIBILITY_RAMP_S = 0.1;
/** Aging bonus subtracted from priority per second of voice life (favors evicting old). */
const AGE_BONUS_PER_S = 0.1;

/** Multiple of `refDist` beyond which a spatial sound is dropped entirely. */
const MAX_DIST_MULT = 3;
/** Lowpass cutoff at the listener (Hz). */
const LP_NEAR_HZ = 20000;
/** Lowpass cutoff at `maxDist` (Hz). Muffled-far cue. */
const LP_FAR_HZ = 1200;
/** Fallback refDist (world px) used until main.js sets one. */
const DEFAULT_REF_DIST = 1920;

export { CATEGORIES };

function clamp01(v) {
  v = Number(v);
  if (!isFinite(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function lsRead(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    if (v == null) return fallback;
    const n = parseFloat(v);
    return isFinite(n) ? clamp01(n) : fallback;
  } catch {
    return fallback;
  }
}

function lsWrite(key, v) {
  try {
    localStorage.setItem(key, String(v));
  } catch {
    /* private mode etc. — settings reset across reloads, no other harm. */
  }
}

// mulberry32: tiny deterministic PRNG. We never use Math.random in audio so the
// per-frame sequence of `play()` calls remains reproducible (phase 5).
function mulberry32(seed) {
  let s = seed >>> 0;
  return function () {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class Audio {
  constructor() {
    /** @type {AudioContext|null} */
    this.ctx = null;
    /** @type {GainNode|null} */
    this.master = null;
    /** @type {Record<string, GainNode>} */
    this.gains = {};
    /** @type {Map<string, AudioBuffer>} */
    this.buffers = new Map();
    /** @type {Map<string, Promise<AudioBuffer|null>>} */
    this.pending = new Map();
    /** Active voices, oldest first. */
    this.voices = [];
    /** Last-played timestamps (ms) for dedup. */
    this.lastPlay = new Map();
    /** Deterministic RNG for pitch variance. */
    this.rng = mulberry32(0xb7e15163);
    /** Manifest entries queued before the context was unlocked. */
    this._queuedManifests = [];
    /** Listener pose in world pixels + reference distance (1 screen-width at current zoom). */
    this.listener = { x: 0, y: 0, refDist: DEFAULT_REF_DIST };

    this.volume = { master: lsRead("audio.master", DEFAULT_MASTER) };
    for (const c of CATEGORIES) {
      const fallback = c === "ambient" ? DEFAULT_AMBIENT : DEFAULT_OTHER;
      this.volume[c] = lsRead(`audio.cat.${c}`, fallback);
    }

    this._gesture = this._gesture.bind(this);
    this._visibility = this._visibility.bind(this);
    this._gestureEvents = ["pointerdown", "keydown", "touchstart"];
    for (const ev of this._gestureEvents) {
      window.addEventListener(ev, this._gesture, true);
    }
    document.addEventListener("visibilitychange", this._visibility);
  }

  /**
   * Preload a manifest of `{ id, url, category }`. Safe to call before the
   * audio context exists — entries are decoded once the first gesture unlocks it.
   * Re-preloading the same id is a no-op.
   * @returns {Promise<void>}
   */
  preload(manifest) {
    if (!Array.isArray(manifest) || manifest.length === 0) return Promise.resolve();
    if (!this.ctx) {
      this._queuedManifests.push(manifest);
      return Promise.resolve();
    }
    return this._decodeManifest(manifest);
  }

  /**
   * Schedule a one-shot.
   * @param {string} id manifest id
   * @param {object} [opts]
   * @param {number} [opts.x] world pixels - when present, sound is spatialized
   * @param {number} [opts.y] world pixels - when present, sound is spatialized
   * @param {number} [opts.priority] higher wins eviction (default 1)
   * @param {string} [opts.category] one of CATEGORIES (default "ui")
   * @param {number} [opts.pitchVariance] override default jitter (0 to disable)
   * @returns {boolean} true if scheduled, false if dropped
   */
  play(id, opts) {
    if (!this.ctx) return false;
    const buf = this.buffers.get(id);
    if (!buf) return false;
    opts = opts || {};

    const now = performance.now();
    const last = this.lastPlay.get(id);
    if (last != null && now - last < DEDUP_MS) return false;

    const category = opts.category && CATEGORY_SET.has(opts.category) ? opts.category : "ui";
    const priority = typeof opts.priority === "number" ? opts.priority : 1;

    // Spatial gate: if a position is supplied, compute attenuation/pan/lpHz from
    // the current listener. Beyond MAX_DIST_MULT * refDist the sound is dropped
    // before we even allocate a voice — the biggest perf win in a 200-unit fight.
    let spatial = null;
    if (typeof opts.x === "number" && typeof opts.y === "number") {
      spatial = this._computeSpatial(opts.x, opts.y);
      if (!spatial) return false;
    }

    if (this.voices.length >= VOICE_CAP) {
      if (!this._evictLowest(priority, now)) return false;
    }

    const src = this.ctx.createBufferSource();
    src.buffer = buf;
    const variance = opts.pitchVariance == null ? PITCH_VARIANCE : Math.max(0, opts.pitchVariance);
    if (variance > 0) {
      src.playbackRate.value = 1 + (this.rng() * 2 - 1) * variance;
    }
    const bus = this.gains[category] || this.master;

    const trail = [];
    if (spatial) {
      const panner = this.ctx.createStereoPanner();
      panner.pan.value = spatial.pan;
      const lp = this.ctx.createBiquadFilter();
      lp.type = "lowpass";
      lp.frequency.value = spatial.lpHz;
      const distGain = this.ctx.createGain();
      distGain.gain.value = spatial.gain;
      src.connect(panner);
      panner.connect(lp);
      lp.connect(distGain);
      distGain.connect(bus);
      trail.push(panner, lp, distGain);
    } else {
      src.connect(bus);
    }
    try {
      src.start();
    } catch {
      return false;
    }
    const voice = { node: src, priority, startedAt: now, category, id, trail };
    this.voices.push(voice);
    src.onended = () => {
      const i = this.voices.indexOf(voice);
      if (i >= 0) this.voices.splice(i, 1);
      for (const n of trail) {
        try { n.disconnect(); } catch { /* already disconnected */ }
      }
    };
    this.lastPlay.set(id, now);
    return true;
  }

  /** Convenience: forces the ui category (non-spatial). */
  playUI(id, opts) {
    return this.play(id, { ...(opts || {}), category: "ui" });
  }

  /**
   * Pick a variant id from a non-empty list using the seeded RNG. Returns null
   * if the list is empty. See PHASE_2.md §"Combat SFX wiring" — avoids the
   * "machine gun" feel of identical samples stacking.
   * @param {string[]} ids
   * @returns {string|null}
   */
  pickVariant(ids) {
    if (!Array.isArray(ids) || ids.length === 0) return null;
    if (ids.length === 1) return ids[0];
    const i = Math.floor(this.rng() * ids.length);
    return ids[i < ids.length ? i : ids.length - 1];
  }

  /**
   * Update the listener pose. `zoom` is screen px per world px; `viewW` lets us
   * derive "1 screen-width worth of world pixels" for the flat full-volume zone.
   * @param {number} x world pixels
   * @param {number} y world pixels
   * @param {number} zoom screen px per world px
   * @param {number} [viewW] viewport width in screen px
   */
  setListener(x, y, zoom, viewW) {
    if (!isFinite(x) || !isFinite(y)) return;
    this.listener.x = x;
    this.listener.y = y;
    if (isFinite(zoom) && zoom > 0) {
      const w = isFinite(viewW) && viewW > 0 ? viewW : DEFAULT_REF_DIST;
      this.listener.refDist = Math.max(1, w / zoom);
    }
  }

  setMasterVolume(v) {
    v = clamp01(v);
    this.volume.master = v;
    lsWrite("audio.master", v);
    if (this.master && this.ctx && !document.hidden) {
      const now = this.ctx.currentTime;
      this.master.gain.cancelScheduledValues(now);
      this.master.gain.setValueAtTime(v, now);
    }
  }

  setCategoryVolume(cat, v) {
    if (!CATEGORY_SET.has(cat)) return;
    v = clamp01(v);
    this.volume[cat] = v;
    lsWrite(`audio.cat.${cat}`, v);
    const g = this.gains[cat];
    if (g && this.ctx) {
      const now = this.ctx.currentTime;
      g.gain.cancelScheduledValues(now);
      g.gain.setValueAtTime(v, now);
    }
  }

  getMasterVolume() {
    return this.volume.master;
  }

  getCategoryVolume(cat) {
    return this.volume[cat];
  }

  destroy() {
    document.removeEventListener("visibilitychange", this._visibility);
    for (const ev of this._gestureEvents) {
      window.removeEventListener(ev, this._gesture, true);
    }
    for (const v of this.voices) {
      try {
        v.node.stop();
      } catch {
        /* already stopped */
      }
    }
    this.voices.length = 0;
    this.buffers.clear();
    this.pending.clear();
    this.lastPlay.clear();
    if (this.ctx) {
      try {
        this.ctx.close();
      } catch {
        /* ignore */
      }
      this.ctx = null;
    }
    this.master = null;
    this.gains = {};
  }

  // --- Internals ------------------------------------------------------------

  _gesture() {
    if (this.ctx) return;
    const Ctor = window.AudioContext || window.webkitAudioContext;
    if (!Ctor) return;
    let ctx;
    try {
      ctx = new Ctor();
    } catch {
      return;
    }
    this.ctx = ctx;
    this.master = ctx.createGain();
    this.master.gain.value = this.volume.master;
    this.master.connect(ctx.destination);
    for (const c of CATEGORIES) {
      const g = ctx.createGain();
      g.gain.value = this.volume[c];
      g.connect(this.master);
      this.gains[c] = g;
    }
    for (const ev of this._gestureEvents) {
      window.removeEventListener(ev, this._gesture, true);
    }
    const queued = this._queuedManifests;
    this._queuedManifests = [];
    for (const m of queued) {
      void this._decodeManifest(m);
    }
  }

  _visibility() {
    if (!this.master || !this.ctx) return;
    const now = this.ctx.currentTime;
    const g = this.master.gain;
    g.cancelScheduledValues(now);
    g.setValueAtTime(g.value, now);
    const target = document.hidden ? 0 : this.volume.master;
    g.linearRampToValueAtTime(target, now + VISIBILITY_RAMP_S);
  }

  /**
   * Compute the Phase 2 spatial parameters for an emitter, or null if it is
   * beyond max distance and should be dropped before voice allocation.
   * @param {number} x emitter world x
   * @param {number} y emitter world y
   * @returns {{gain:number, pan:number, lpHz:number}|null}
   */
  _computeSpatial(x, y) {
    const refDist = Math.max(1, this.listener.refDist || DEFAULT_REF_DIST);
    const dx = x - this.listener.x;
    const dy = y - this.listener.y;
    const d = Math.sqrt(dx * dx + dy * dy);
    const maxDist = MAX_DIST_MULT * refDist;
    if (d > maxDist) return null;
    const gain = clamp01(refDist / Math.max(d, refDist));
    const pan = Math.max(-1, Math.min(1, dx / refDist));
    const farT = clamp01(d / maxDist);
    const lpHz = LP_NEAR_HZ + (LP_FAR_HZ - LP_NEAR_HZ) * farT;
    return { gain, pan, lpHz };
  }

  async _decodeManifest(manifest) {
    const jobs = [];
    for (const entry of manifest) {
      if (!entry || !entry.id || !entry.url) continue;
      if (this.buffers.has(entry.id) || this.pending.has(entry.id)) continue;
      const p = this._fetchAndDecode(entry.url).then(
        (buf) => {
          this.buffers.set(entry.id, buf);
          this.pending.delete(entry.id);
          return buf;
        },
        (err) => {
          this.pending.delete(entry.id);
          console.warn(`audio: decode failed for ${entry.id}`, err);
          return null;
        },
      );
      this.pending.set(entry.id, p);
      jobs.push(p);
    }
    await Promise.all(jobs);
  }

  async _fetchAndDecode(url) {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
    const data = await res.arrayBuffer();
    // Safari historically supported only the callback form of decodeAudioData.
    // The promise form has been the standard since Safari 14.1; both code paths
    // are kept so we degrade gracefully on older builds.
    return await new Promise((resolve, reject) => {
      const maybePromise = this.ctx.decodeAudioData(data, resolve, reject);
      if (maybePromise && typeof maybePromise.then === "function") {
        maybePromise.then(resolve, reject);
      }
    });
  }

  /**
   * Try to evict the worst-scoring voice (priority - age_bonus * age). If even
   * the worst voice outranks the incoming call, the new sound is dropped.
   * @returns {boolean} true if a slot was freed
   */
  _evictLowest(incomingPriority, now) {
    if (this.voices.length === 0) return true;
    let worstIdx = -1;
    let worstScore = Infinity;
    for (let i = 0; i < this.voices.length; i++) {
      const v = this.voices[i];
      const ageS = (now - v.startedAt) / 1000;
      const score = v.priority - ageS * AGE_BONUS_PER_S;
      if (score < worstScore) {
        worstScore = score;
        worstIdx = i;
      }
    }
    if (worstIdx < 0) return false;
    if (worstScore >= incomingPriority) return false;
    try {
      this.voices[worstIdx].node.stop();
    } catch {
      /* already ended */
    }
    this.voices.splice(worstIdx, 1);
    return true;
  }
}

/**
 * Phase-1 sound manifest. URLs are served by the Rust process from `client/assets`.
 * IDs are stable seams referenced by main.js, input.js, and (later phases) renderer.js.
 */
export const SOUND_MANIFEST = Object.freeze([
  { id: "notice_generic",      url: "/assets/sound/alert/alert_under_attack_01.mp3",  category: "alert" },
  { id: "notice_supply",       url: "/assets/sound/alert/alert_supply_low_01.mp3",    category: "alert" },
  { id: "notice_steel",        url: "/assets/sound/alert/alert_steel_low_01.mp3",     category: "alert" },
  { id: "notice_oil",          url: "/assets/sound/alert/alert_oil_low_01.mp3",       category: "alert" },
  { id: "notice_cannot_build", url: "/assets/sound/alert/alert_cannot_build_01.mp3",  category: "alert" },
  { id: "notice_out_of_range", url: "/assets/sound/alert/alert_out_of_range_01.mp3",  category: "alert" },
  { id: "build_confirm",       url: "/assets/sound/buildings/buildings_construction_start_01.mp3", category: "ui" },
  { id: "combat_tank_01",      url: "/assets/sound/combat/combat_tank_cannon_01.mp3", category: "combat_other" },
  { id: "combat_tank_06",      url: "/assets/sound/combat/combat_tank_cannon_06.mp3", category: "combat_other" },
  { id: "combat_rifle_02",     url: "/assets/sound/combat/combat_kar98k_02.mp3", category: "combat_other" },
  { id: "combat_rifle_03",     url: "/assets/sound/combat/combat_kar98k_03.mp3", category: "combat_other" },
  { id: "combat_mg_burst_02",  url: "/assets/sound/combat/combat_mg42_burst_02.mp3", category: "combat_other" },
  { id: "combat_mg_burst_03",  url: "/assets/sound/combat/combat_mg42_burst_03.mp3", category: "combat_other" },
  { id: "victory",             url: "/assets/sound/ui/ui_victory_01.mp3",             category: "ui" },
  { id: "defeat",              url: "/assets/sound/ui/ui_defeat_01.mp3",              category: "ui" },
]);

/**
 * Pick a notice sound id from a server `Notice` message text. Simple keyword
 * routing — sufficient until phase 4 adds richer alert categorization.
 */
export function noticeSoundId(msg) {
  const m = (msg || "").toLowerCase();
  if (m.includes("supply") || m.includes("depot")) return "notice_supply";
  if (m.includes("steel")) return "notice_steel";
  if (m.includes("oil")) return "notice_oil";
  if (m.includes("cannot build") || m.includes("can't build") || m.includes("blocked")) {
    return "notice_cannot_build";
  }
  if (m.includes("out of range") || m.includes("too far")) return "notice_out_of_range";
  return "notice_generic";
}
