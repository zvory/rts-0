/**
 * Exclusive owner for a renderer prepared before Match takes ownership.
 *
 * A preparation may be transferred to one compatible match start or destroyed.
 * Every start must settle the slot, including starts that cannot reuse the
 * preparation, so an abandoned canvas cannot remain mounted beside the active
 * renderer.
 */
export class RendererPreparationSlot {
  constructor({
    onCountdownReady = () => {},
    onFailure = () => {},
    setTimer = globalThis.setTimeout?.bind(globalThis),
    clearTimer = globalThis.clearTimeout?.bind(globalThis),
  } = {}) {
    this._onCountdownReady = onCountdownReady;
    this._onFailure = onFailure;
    this._setTimer = setTimer;
    this._clearTimer = clearTimer;
    this._current = null;
  }

  get current() {
    return this._current;
  }

  warm(createPreparation) {
    if (this._current) return this._current;
    if (typeof createPreparation !== "function") {
      throw new TypeError("Renderer preparation requires a factory.");
    }
    const state = {
      acknowledged: false,
      cleanupTimer: undefined,
      countdownId: null,
      disposition: "owned",
      preparation: null,
      promise: null,
    };
    this._current = state;
    state.promise = Promise.resolve()
      .then(() => createPreparation())
      .then((preparation) => {
        if (!preparation || typeof preparation.destroy !== "function") {
          throw new TypeError("Renderer preparation factory returned an invalid owner.");
        }
        if (state.disposition === "discard") {
          preparation.destroy();
          return null;
        }
        state.preparation = preparation;
        if (state.disposition === "owned") this._acknowledge(state);
        return preparation;
      })
      .catch((error) => {
        if (this._current === state) this._current = null;
        this._clearCleanup(state);
        if (state.disposition !== "discard") this._onFailure(error);
        return null;
      });
    return state;
  }

  armCountdown(countdownId, durationMs) {
    const state = this._current;
    if (!state) return false;
    state.countdownId = countdownId;
    state.acknowledged = false;
    this._acknowledge(state);
    this._clearCleanup(state);
    if (typeof this._setTimer === "function") {
      state.cleanupTimer = this._setTimer(() => {
        if (this._current === state) this.discard();
      }, Math.max(1000, Number(durationMs) || 3000) + 5000);
    }
    return true;
  }

  async settleForStart({ reuse = false } = {}) {
    const state = this._takeCurrent(reuse ? "transfer" : "discard");
    if (!state) return null;
    const preparation = await state.promise;
    if (!reuse && preparation) preparation.destroy();
    return reuse ? preparation : null;
  }

  discard() {
    const state = this._takeCurrent("discard");
    if (!state) return;
    if (state.preparation) {
      state.preparation.destroy();
      state.preparation = null;
    }
  }

  releaseCountdown() {
    const state = this._current;
    if (!state) return;
    state.countdownId = null;
    state.acknowledged = false;
    this._clearCleanup(state);
  }

  _takeCurrent(disposition) {
    const state = this._current;
    if (!state) return null;
    this._current = null;
    state.disposition = disposition;
    this._clearCleanup(state);
    return state;
  }

  _acknowledge(state) {
    if (this._current !== state || !state.preparation
      || !Number.isInteger(state.countdownId) || state.acknowledged) return;
    state.acknowledged = true;
    this._onCountdownReady(state.countdownId);
  }

  _clearCleanup(state) {
    if (state.cleanupTimer === undefined) return;
    this._clearTimer?.(state.cleanupTimer);
    state.cleanupTimer = undefined;
  }
}

export function settleRendererPreparationForStart(
  slot,
  { replay = false, lab = false } = {},
) {
  if (!slot || typeof slot.settleForStart !== "function") {
    throw new TypeError("Match start requires a renderer preparation slot.");
  }
  return slot.settleForStart({ reuse: !replay && !lab });
}
