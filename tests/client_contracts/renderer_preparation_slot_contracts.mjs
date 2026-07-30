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
  });
  const second = slot.warm(() => {
    createCount += 1;
    return preparation;
  });
  assert(first === second, "renderer preparation slot admits only one warm owner");
  slot.armCountdown(23, 3000);
  await first.promise;
  assertDeepEqual(readyIds, [23], "prepared renderer acknowledges its countdown once");
  assert(createCount === 1, "duplicate warm requests reuse the exclusive preparation");
  assert(timers[0]?.delay === 8000, "preparation cleanup remains bounded beyond countdown");
  const transferred = await slot.settleForStart({ reuse: true });
  assert(transferred === preparation && destroyed === 0,
    "compatible match start atomically takes renderer preparation ownership");
  assert(slot.current === null, "transferred preparation leaves the pre-match slot empty");
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
    settleForStart({ reuse }) {
      reuseDecisions.push(reuse);
      return Promise.resolve(null);
    },
  };
  await settleRendererPreparationForStart(slot);
  await settleRendererPreparationForStart(slot, { replay: true });
  await settleRendererPreparationForStart(slot, { lab: true });
  assertDeepEqual(
    reuseDecisions,
    [true, false, false],
    "every start settles the slot; only ordinary matches may adopt its renderer",
  );
}
