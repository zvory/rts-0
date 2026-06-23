/**
 * Pure client-side command arming state.
 *
 * The composer does not know about DOM, hit-testing, networking, or whether the
 * server will accept a command. It only answers command lifetime questions for
 * the input layer: what is armed, whether a click should queue, and whether the
 * armed command survives after a click or key release.
 */
export class CommandComposer {
  constructor() {
    this._armed = null;
    this._lastTap = null;
  }

  /** @returns {null|string|object} */
  get target() {
    return cloneTarget(this._armed?.target ?? null);
  }

  /** @returns {boolean} */
  get shiftPreserved() {
    return !!this._armed?.shiftPreserved;
  }

  /**
   * Arm a command target.
   * @param {string|object} target
   * @param {{source?: "tap"|"hold", key?: string|null, shiftKey?: boolean, now?: number}} [options]
   * @returns {{quickCast:boolean,target:string|object,queued:boolean}}
   */
  arm(target, options = {}) {
    const now = finiteTime(options.now);
    const source = options.source === "hold" ? "hold" : "tap";
    const key = typeof options.key === "string" ? options.key : null;
    const sameTap =
      source === "tap" &&
      this._lastTap &&
      targetsEqual(this._lastTap.target, target) &&
      now - this._lastTap.at <= QUICK_CAST_MS;

    this._armed = {
      target: cloneTarget(target),
      source,
      key,
      held: source === "hold",
      shiftPreserved: !!options.shiftKey,
      preserveTapOnRelease: false,
      issuedWhileHeld: false,
    };
    this._lastTap = source === "tap" ? { target: cloneTarget(target), at: now } : null;
    return { quickCast: !!sameTap, target: cloneTarget(target), queued: !!options.shiftKey };
  }

  /**
   * Mark a physical key as held for the currently armed target.
   * @param {string|object} target
   * @param {string} key
   * @param {{shiftKey?: boolean, preserveTapOnRelease?: boolean}} [options]
   */
  hold(target, key, options = {}) {
    if (!this._armed || !targetsEqual(this._armed.target, target)) {
      this._armed = {
        target: cloneTarget(target),
        source: "hold",
        key,
        held: true,
        shiftPreserved: !!options.shiftKey,
        preserveTapOnRelease: false,
        issuedWhileHeld: false,
      };
      return;
    }
    this._armed.source = "hold";
    this._armed.key = key;
    this._armed.held = true;
    this._armed.shiftPreserved = this._armed.shiftPreserved || !!options.shiftKey;
    this._armed.preserveTapOnRelease =
      this._armed.preserveTapOnRelease || !!options.preserveTapOnRelease;
  }

  /**
   * Report that the armed command issued from a click.
   * @param {{shiftKey?: boolean}} [options]
   * @returns {{target:null|string|object,queued:boolean,keepArmed:boolean}}
   */
  issue(options = {}) {
    if (!this._armed) return { target: null, queued: !!options.shiftKey, keepArmed: false };
    const queued = !!options.shiftKey;
    if (queued) this._armed.shiftPreserved = true;
    const held = this._armed.held;
    const keepArmed = held || this._armed.shiftPreserved;
    const target = cloneTarget(this._armed.target);
    if (held) this._armed.issuedWhileHeld = true;
    if (!keepArmed) this.cancel();
    return { target, queued, keepArmed };
  }

  /**
   * Release a physical command key.
   * @param {string} key
   * @param {{shiftKey?: boolean}} [options]
   * @returns {boolean} true when a target remains armed
   */
  releaseKey(key, options = {}) {
    if (!this._armed || this._armed.key !== key) return !!this._armed;
    this._armed.held = false;
    if (options.shiftKey || this._armed.shiftPreserved) {
      this._armed.shiftPreserved = true;
      return true;
    }
    if (this._armed.preserveTapOnRelease && !this._armed.issuedWhileHeld) {
      return true;
    }
    this.cancel();
    return false;
  }

  /**
   * Release Shift. If Shift was the only reason a tapped command stayed armed,
   * clear it.
   */
  releaseShift() {
    if (!this._armed) return;
    this._armed.shiftPreserved = false;
    if (!this._armed.held) this.cancel();
  }

  cancel() {
    this._armed = null;
  }
}

export const QUICK_CAST_MS = 300;

function finiteTime(now) {
  return typeof now === "number" && Number.isFinite(now) ? now : performanceNow();
}

function performanceNow() {
  return typeof performance !== "undefined" && typeof performance.now === "function"
    ? performance.now()
    : Date.now();
}

function cloneTarget(target) {
  if (!target || typeof target !== "object") return target ?? null;
  return { ...target };
}

function targetsEqual(a, b) {
  if (a === b) return true;
  if (!a || !b || typeof a !== "object" || typeof b !== "object") return false;
  return a.kind === b.kind && a.ability === b.ability;
}
