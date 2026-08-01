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
    this._disposalBarrier = Promise.resolve();
  }

  get current() {
    return this._current;
  }

  warm(createPreparation, { compatibilityKey = null } = {}) {
    if (this._current) return this._current;
    if (typeof createPreparation !== "function") {
      throw new TypeError("Renderer preparation requires a factory.");
    }
    const state = {
      acknowledged: false,
      cleanupTimer: undefined,
      compatibilityKey,
      countdownId: null,
      disposition: "owned",
      preparation: null,
      promise: null,
    };
    this._current = state;
    state.promise = this._disposalBarrier
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
        let failure = error;
        if (state.preparation) {
          const preparation = state.preparation;
          state.preparation = null;
          try {
            preparation.destroy();
          } catch (destroyError) {
            failure = destroyError;
          }
        }
        if (state.disposition !== "discard") this._onFailure(failure);
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

  async settleForStart({ reuse = false, compatibilityKey = null } = {}) {
    const state = this._current;
    const shouldReuse = !!state && reuse && state.compatibilityKey === compatibilityKey;
    const takenState = this._takeCurrent(shouldReuse ? "transfer" : "discard");
    if (!takenState) {
      await this._disposalBarrier;
      return null;
    }
    if (!shouldReuse) return this._dispose(takenState);
    await this._disposalBarrier;
    return takenState.promise;
  }

  discard() {
    const state = this._takeCurrent("discard");
    if (!state) return;
    if (state.preparation) {
      state.preparation.destroy();
      state.preparation = null;
    }
    void this._dispose(state);
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

  _dispose(state) {
    const disposal = this._disposalBarrier.then(async () => {
      const preparation = await state.promise;
      if (preparation && state.preparation === preparation) {
        state.preparation = null;
        preparation.destroy();
      }
      return null;
    });
    this._disposalBarrier = disposal.catch(() => null);
    return disposal;
  }

  _clearCleanup(state) {
    if (state.cleanupTimer === undefined) return;
    this._clearTimer?.(state.cleanupTimer);
    state.cleanupTimer = undefined;
  }
}

export function settleRendererPreparationForStart(
  slot,
  { lab = false, compatibilityKey = null } = {},
) {
  if (!slot || typeof slot.settleForStart !== "function") {
    throw new TypeError("Match start requires a renderer preparation slot.");
  }
  return slot.settleForStart({
    reuse: !lab,
    compatibilityKey,
  });
}
