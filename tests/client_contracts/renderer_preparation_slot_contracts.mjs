import {
  assert,
  assertDeepEqual,
} from "./assertions.mjs";
import {
  RendererPreparationSlot,
  settleRendererPreparationForStart,
} from "../../client/src/renderer/preparation_slot.js";

{
  const readyIds = [];
  const timers = [];
  let createCount = 0;
  let destroyed = 0;
  const preparation = { destroy() { destroyed += 1; } };
  const slot = new RendererPreparationSlot({
    onCountdownReady: (id) => readyIds.push(id),
    setTimer: (callback, delay) => {
      timers.push({ callback, delay });
      return timers.length;
    },
    clearTimer() {},
  });
  const first = slot.warm(async () => {
    createCount += 1;
    return preparation;
  }, { compatibilityKey: "pixi" });
  const second = slot.warm(() => {
    createCount += 1;
    return preparation;
  }, { compatibilityKey: "pixi" });
  assert(first === second, "renderer preparation slot admits only one warm owner");
  slot.armCountdown(23, 3000);
  await first.promise;
  assertDeepEqual(readyIds, [23], "prepared renderer acknowledges its countdown once");
  assert(createCount === 1, "duplicate warm requests reuse the exclusive preparation");
  assert(timers[0]?.delay === 8000, "preparation cleanup remains bounded beyond countdown");
  const transferred = await slot.settleForStart({
    reuse: true,
    compatibilityKey: "pixi",
  });
  assert(transferred === preparation && destroyed === 0,
    "compatible match start atomically takes renderer preparation ownership");
  assert(slot.current === null, "transferred preparation leaves the pre-match slot empty");
}

{
  let finishFirstPreparation;
  let secondCreateCount = 0;
  let firstDestroyed = 0;
  const slot = new RendererPreparationSlot();
  slot.warm(() => new Promise((resolve) => {
    finishFirstPreparation = resolve;
  }));
  await Promise.resolve();
  slot.discard();
  const second = slot.warm(async () => {
    secondCreateCount += 1;
    return { destroy() {} };
  });
  await Promise.resolve();
  assert(secondCreateCount === 0,
    "replacement warmup waits for the discarded preparation to finish teardown");
  finishFirstPreparation({ destroy() { firstDestroyed += 1; } });
  await second.promise;
  assert(firstDestroyed === 1 && secondCreateCount === 1,
    "replacement warmup starts only after its predecessor is destroyed");
  slot.discard();
}

{
  let destroyed = 0;
  const failures = [];
  const slot = new RendererPreparationSlot({
    onCountdownReady() {
      throw new Error("planned readiness failure");
    },
    onFailure: (error) => failures.push(error.message),
  });
  const state = slot.warm(async () => ({
    destroy() { destroyed += 1; },
  }));
  slot.armCountdown(24, 3000);
  await state.promise;
  assert(destroyed === 1 && slot.current === null,
    "readiness callback failure cannot orphan the prepared renderer");
  assertDeepEqual(failures, ["planned readiness failure"],
    "readiness callback failure is reported after renderer teardown");
}

{
  let destroyed = 0;
  const slot = new RendererPreparationSlot();
  const state = slot.warm(async () => ({
    destroy() { destroyed += 1; },
  }), { compatibilityKey: "alternate" });
  await state.promise;
  const transferred = await slot.settleForStart({
    reuse: true,
    compatibilityKey: "pixi",
  });
  assert(transferred === null && destroyed === 1,
    "a match start destroys a prepared renderer from an incompatible backend");
}

{
  let finishPreparation;
  let destroyed = 0;
  let settled = false;
  const slot = new RendererPreparationSlot();
  slot.warm(() => new Promise((resolve) => {
    finishPreparation = resolve;
  }));
  await Promise.resolve();
  const settling = slot.settleForStart({ reuse: false }).then(() => {
    settled = true;
  });
  await Promise.resolve();
  assert(!settled, "incompatible start waits for in-flight preparation teardown");
  finishPreparation({ destroy() { destroyed += 1; } });
  await settling;
  assert(destroyed === 1 && slot.current === null,
    "incompatible start destroys preparation before another renderer may be created");
}

{
  let destroyed = 0;
  const slot = new RendererPreparationSlot();
  const state = slot.warm(async () => ({ destroy() { destroyed += 1; } }));
  await state.promise;
  slot.discard();
  slot.discard();
  assert(destroyed === 1, "renderer preparation discard is idempotent");
}

{
  const reuseDecisions = [];
  const slot = {
    settleForStart({ reuse, compatibilityKey }) {
      reuseDecisions.push({ reuse, compatibilityKey });
      return Promise.resolve(null);
    },
  };
  await settleRendererPreparationForStart(slot, { compatibilityKey: "pixi" });
  await settleRendererPreparationForStart(slot, {
    replay: true,
    compatibilityKey: "pixi",
  });
  await settleRendererPreparationForStart(slot, {
    lab: true,
    compatibilityKey: "alternate",
  });
  assertDeepEqual(
    reuseDecisions,
    [
      { reuse: true, compatibilityKey: "pixi" },
      { reuse: true, compatibilityKey: "pixi" },
      { reuse: false, compatibilityKey: "alternate" },
    ],
    "every start settles the slot; compatible live and replay matches may adopt its renderer",
  );
}
