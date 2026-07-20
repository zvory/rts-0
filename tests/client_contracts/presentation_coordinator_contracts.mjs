import { assert } from "./assertions.mjs";
import { PresentationCoordinator } from "../../client/src/presentation/coordinator.js";
import {
  PRESENTATION_OUTCOME,
  createPresentationSubmission,
  immediatePresentationSubmission,
  outcomeRecord,
} from "../../client/src/presentation/submission.js";

const frame = (generation, frameId, groundDecalRevision = 0) => ({
  version: 1,
  generation,
  frameId,
  groundDecalRevision,
});
const scene = (generation, frameId) => ({ version: 1, generation, frameId, projection: {}, proxies: [] });

{
  const published = [];
  const retained = [];
  const counters = [];
  const coordinator = new PresentationCoordinator({
    publishSelectionScene: (value) => published.push(value),
    acknowledgeGroundDecals: (revision) => retained.push(revision),
    recordCounter: (label) => counters.push(label),
  });
  const currentFrame = frame(1, 1, 4);
  const completion = coordinator.submit({
    frame: currentFrame,
    selectionScene: scene(1, 1),
    submission: immediatePresentationSubmission({
      generation: 1,
      frameId: 1,
      retainedRevision: 4,
      status: PRESENTATION_OUTCOME.PRESENTED,
    }),
  });
  assert(published.length === 0 && retained.length === 0, "immediate adapters cannot re-enter Match during submit");
  const outcome = await completion;
  assert(outcome.status === PRESENTATION_OUTCOME.PRESENTED, "immediate success settles as presented");
  assert(published[0]?.frameId === 1, "presented publishes only its matching selection scene");
  assert(retained.join(",") === "4", "retained independently acknowledges the exact durable revision");
  assert(coordinator.displayedFrameCount === 1, "only presented advances the public displayed-frame count");
  assert(counters.includes("presentation.frames.submitted") && counters.includes("presentation.frames.presented"),
    "submission and presentation counters remain separate");
}

{
  const retained = [];
  const failures = [];
  const retainedDeferred = deferred();
  const terminalDeferred = deferred();
  const coordinator = new PresentationCoordinator({
    acknowledgeGroundDecals: (revision) => retained.push(revision),
    recordFailure: (error) => failures.push(error.message),
  });
  const completion = coordinator.submit({
    frame: frame(1, 1, 8),
    selectionScene: scene(1, 1),
    submission: createPresentationSubmission({
      generation: 1,
      frameId: 1,
      retained: retainedDeferred.promise,
      settled: terminalDeferred.promise,
    }),
  });
  retainedDeferred.resolve(outcomeRecord(PRESENTATION_OUTCOME.RETAINED, frame(1, 1), {
    groundDecalRevision: 8,
  }));
  await Promise.resolve();
  assert(retained.join(",") === "8", "retain-then-fail releases the model queue before terminal failure");
  terminalDeferred.resolve(outcomeRecord(PRESENTATION_OUTCOME.FAILED, frame(1, 1), {
    error: { name: "Error", message: "planned failure" },
  }));
  assert((await completion).status === PRESENTATION_OUTCOME.FAILED, "failure remains bounded to its submitted frame");
  assert(failures.join(",") === "planned failure", "failed includes a useful bounded error");
}

{
  const published = [];
  const protocols = [];
  const coordinator = new PresentationCoordinator({
    publishSelectionScene: (value) => published.push(value),
    recordProtocolError: (message) => protocols.push(message),
  });
  await coordinator.submit({
    frame: frame(1, 1),
    selectionScene: scene(1, 1),
    submission: immediatePresentationSubmission({ generation: 1, frameId: 1, status: PRESENTATION_OUTCOME.PRESENTED }),
  });
  await coordinator.submit({
    frame: frame(1, 2),
    selectionScene: scene(1, 2),
    submission: immediatePresentationSubmission({ generation: 1, frameId: 2, status: PRESENTATION_OUTCOME.SUPERSEDED }),
  });
  assert(published.length === 1 && published[0].frameId === 1, "superseded preserves the last displayed selection scene");
  coordinator.acceptTerminal(outcomeRecord(PRESENTATION_OUTCOME.PRESENTED, frame(1, 1)));
  coordinator.acceptTerminal(outcomeRecord(PRESENTATION_OUTCOME.PRESENTED, frame(1, 99)));
  assert(protocols.length === 2, "duplicate and unknown acknowledgments become bounded protocol errors");
  assert(coordinator.displayedFrameCount === 1, "stale or unknown acknowledgments cannot advance displayed counters");
}

{
  const oldRetained = deferred();
  const oldTerminal = deferred();
  const published = [];
  const coordinator = new PresentationCoordinator({ publishSelectionScene: (value) => published.push(value) });
  const oldCompletion = coordinator.submit({
    frame: frame(1, 1, 3),
    selectionScene: scene(1, 1),
    submission: createPresentationSubmission({
      generation: 1,
      frameId: 1,
      retained: oldRetained.promise,
      settled: oldTerminal.promise,
    }),
  });
  const nextCompletion = coordinator.submit({
    frame: frame(2, 1),
    selectionScene: scene(2, 1),
    submission: immediatePresentationSubmission({ generation: 2, frameId: 1, status: PRESENTATION_OUTCOME.PRESENTED }),
  });
  assert((await oldCompletion).status === PRESENTATION_OUTCOME.SUPERSEDED, "generation reset settles older pending frames as superseded");
  assert((await nextCompletion).status === PRESENTATION_OUTCOME.PRESENTED, "new generation can present frame id one");
  oldRetained.resolve(outcomeRecord(PRESENTATION_OUTCOME.RETAINED, frame(1, 1), { groundDecalRevision: 3 }));
  oldTerminal.resolve(outcomeRecord(PRESENTATION_OUTCOME.PRESENTED, frame(1, 1)));
  await Promise.resolve();
  assert(published.length === 1 && published[0].generation === 2, "late old-generation completion cannot replace the visible scene");
}

{
  const retainedDeferred = deferred();
  const terminalDeferred = deferred();
  const published = [];
  const retained = [];
  const coordinator = new PresentationCoordinator({
    publishSelectionScene: (value) => published.push(value),
    acknowledgeGroundDecals: (revision) => retained.push(revision),
  });
  const completion = coordinator.submit({
    frame: frame(1, 1, 6),
    selectionScene: scene(1, 1),
    submission: createPresentationSubmission({
      generation: 1,
      frameId: 1,
      retained: retainedDeferred.promise,
      settled: terminalDeferred.promise,
    }),
  });
  coordinator.destroy();
  assert((await completion).status === PRESENTATION_OUTCOME.DESTROYED, "destroy settles pending presentation work");
  retainedDeferred.resolve(outcomeRecord(PRESENTATION_OUTCOME.RETAINED, frame(1, 1), { groundDecalRevision: 6 }));
  terminalDeferred.resolve(outcomeRecord(PRESENTATION_OUTCOME.PRESENTED, frame(1, 1)));
  await Promise.resolve();
  assert(published.length === 0 && retained.length === 0, "destroy prevents every late selection and decal side effect");
}

function deferred() {
  let resolve;
  const promise = new Promise((accept) => { resolve = accept; });
  return { promise, resolve };
}

console.log("✅ presentation_coordinator_contracts.mjs: asynchronous lifecycle contracts passed");
