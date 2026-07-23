import { assert } from "./assertions.mjs";
import { Camera } from "../../client/src/camera.js";
import { PresentationFrameAssembler } from "../../client/src/presentation/frame.js";
import { PRESENTATION_OUTCOME } from "../../client/src/presentation/submission.js";
import { PixiWorkerPresentationAdapter } from "../../client/src/renderer/pixi_worker_host.js";
import { RENDER_WORKER_RESPONSE } from "../../client/src/renderer/worker_messages.js";

const priorDocument = globalThis.document;
const priorRaf = globalThis.requestAnimationFrame;
const priorStats = globalThis.__rtsRenderWorkerStats;
const priorControl = globalThis.__rtsRenderWorkerControl;
globalThis.requestAnimationFrame = (callback) => { callback(10); return 1; };

async function queueAndLifecycleContracts() {
  const fixture = createFixture();
  const { adapter, worker, canvas, root, assembler } = fixture;
  const frame1 = assemble(assembler, 1);
  const frame2 = assemble(assembler, 2);
  const frame3 = assemble(assembler, 3);
  const first = adapter.render(frame1);
  const second = adapter.render(frame2);
  const third = adapter.render(frame3);

  assert((await second.settled).status === PRESENTATION_OUTCOME.SUPERSEDED,
    "one in-flight plus one latest pending frame supersedes the replaced pending id");
  assert(worker.frameMessages().map((message) => message.payload.frame.frameId).join(",") === "1",
    "the worker receives only the in-flight frame while the latest frame remains bounded on main");
  adapter.resize(800, 600);
  assert((await third.settled).status === PRESENTATION_OUTCOME.SUPERSEDED,
    "resize discards an ordinary pre-resize pending frame and its selection metadata");
  assert(!worker.messages.some((message) => message.type === "resize"),
    "resize waits behind the in-flight frame so it cannot clear a frame before its acknowledgment commits");

  worker.present(frame1);
  assert((await first.settled).status === PRESENTATION_OUTCOME.PRESENTED,
    "the exact in-flight frame id settles as presented");
  assert(worker.messages.at(-1).type === "resize", "deferred resize follows the in-flight acknowledgment");
  const resize = worker.messages.at(-1).payload;
  assert(resize.widthCssPx === 800 && resize.heightCssPx === 600 && resize.dpr === 1,
    "resize carries CSS dimensions, DPR, generation, and the last committed frame id");

  const frame4 = assemble(assembler, 4);
  const afterResize = adapter.render(frame4);
  assert(worker.messages.findLastIndex((message) => message.type === "resize")
      < worker.messages.findLastIndex((message) => message.type === "frame"),
    "the first post-resize frame is ordered after the resize barrier");
  await assertRejects(
    adapter.readPresentedPixels(frame1.frameId),
    "a framebuffer read is rejected while a newer frame is rendering",
  );
  worker.present(frame4);
  assert((await afterResize.settled).status === PRESENTATION_OUTCOME.PRESENTED,
    "the first post-resize frame presents against the resized canvas");
  assert(adapter.diagnostics().completed === 2 && adapter.diagnostics().superseded === 2,
    "worker diagnostics distinguish completed presentations from superseded submissions");
  assert(adapter.diagnostics().clonedBytes > 0,
    "worker diagnostics count transferable bytes before postMessage detaches their buffers");
  assert(adapter.diagnostics().displayAgeMs.samples === 2 && adapter.diagnostics().queueAgeMs.samples === 2,
    "worker diagnostics retain queue and compositor-observed display ages");

  adapter.enterFixedCapture();
  const frame5 = assemble(assembler, 5);
  const captureSubmission = adapter.render(frame5);
  assert(worker.frameMessages().at(-1).payload.capturePixels === true,
    "fixed capture marks the exact submitted frame for same-task framebuffer readback");
  worker.present(frame5, { rgba: new Uint8Array([1, 2, 3, 255]).buffer, width: 1, height: 1 });
  assert((await captureSubmission.settled).status === PRESENTATION_OUTCOME.PRESENTED,
    "fixed capture waits for its exact presented acknowledgment");
  const pixels = await adapter.readPresentedPixels(frame5.frameId);
  assert(pixels.width === 1 && pixels.height === 1 && pixels.rgba.join(",") === "1,2,3,255",
    "fixed capture returns pixels read in the worker presentation task for the matching frame");
  adapter.exitFixedCapture();

  adapter.destroy();
  adapter.destroy();
  assert(worker.terminated === 1 && canvas.removed === 1 && root.children.length === 0,
    "worker teardown is idempotent and removes listeners, worker, and transferred canvas once");
}

async function generationAndFatalContracts() {
  const fixture = createFixture();
  const { adapter, worker, root, assembler } = fixture;
  const oldFrame = assemble(assembler, 1);
  const old = adapter.render(oldFrame);
  assembler.reset({ map: fixture.map, generation: 2 });
  const freshFrame = assemble(assembler, 2);
  const fresh = adapter.render(freshFrame);
  assert((await old.settled).status === PRESENTATION_OUTCOME.SUPERSEDED,
    "generation reset discards all old in-flight presentation metadata");
  worker.present(oldFrame);
  assert(adapter.diagnostics().staleResponses === 1 && adapter.diagnostics().failed === 0,
    "a late acknowledgment from an old generation is ignored without poisoning the new match");
  worker.present(freshFrame);
  assert((await fresh.settled).status === PRESENTATION_OUTCOME.PRESENTED,
    "the monotonically current generation still presents after a stale acknowledgment");

  const fatalFrame = assemble(assembler, 3);
  const fatal = adapter.render(fatalFrame);
  const priorConsoleError = console.error;
  try {
    console.error = () => {};
    worker.emit(response(RENDER_WORKER_RESPONSE.PRESENTED, 2, {
      frameId: fatalFrame.frameId + 99,
      workerUpdateMs: 1,
      workerPresentMs: 1,
      queueAgeMs: 1,
      displayAgeMs: 2,
    }));
  } finally {
    console.error = priorConsoleError;
  }
  assert((await fatal.settled).status === PRESENTATION_OUTCOME.FAILED,
    "an out-of-order current-generation acknowledgment fails the renderer boundedly");
  assert(adapter.diagnostics().failed === 1 && worker.terminated === 1,
    "fatal protocol failure tears down the worker and records one failed renderer lifecycle");
  assert(root.children[0]?.className === "renderer-fatal-error" && root.children[0]?.role === "alert",
    "fatal worker failure leaves a visible bounded match error and does not construct a fallback");
  adapter.destroy();
  adapter.destroy();
  assert(worker.terminated === 1, "destroy after fatal remains idempotent");

  const staleFailureFixture = createFixture();
  const staleFailureOldFrame = assemble(staleFailureFixture.assembler, 1);
  const staleFailureOld = staleFailureFixture.adapter.render(staleFailureOldFrame);
  staleFailureFixture.assembler.reset({ map: staleFailureFixture.map, generation: 2 });
  const staleFailureFreshFrame = assemble(staleFailureFixture.assembler, 2);
  const staleFailureFresh = staleFailureFixture.adapter.render(staleFailureFreshFrame);
  assert((await staleFailureOld.settled).status === PRESENTATION_OUTCOME.SUPERSEDED,
    "generation reset supersedes the old job before its worker result arrives");
  const savedConsoleError = console.error;
  try {
    console.error = () => {};
    staleFailureFixture.worker.emit(response(RENDER_WORKER_RESPONSE.FAILED, 1, {
      frameId: staleFailureOldFrame.frameId,
      code: "webglContextLost",
      message: "old generation render failed",
      stack: "Error: old generation render failed\n    at pixi_render_worker.js:42:7",
      source: "pixi_render_worker.js",
      line: 42,
      column: 7,
    }));
  } finally {
    console.error = savedConsoleError;
  }
  assert((await staleFailureFresh.settled).status === PRESENTATION_OUTCOME.FAILED,
    "a worker-closing failure from an old generation settles current-generation work");
  assert(staleFailureFixture.adapter.terminalFailure()?.message === "old generation render failed"
      && staleFailureFixture.worker.terminated === 1,
    "stale-generation worker failure remains lifecycle-fatal instead of leaving a dead worker active");
  await Promise.resolve();
  assert(
    staleFailureFixture.adapter.diagnostics().lastErrorCode === "webglContextLost" &&
      staleFailureFixture.adapter.diagnostics().contextLost === 1 &&
      staleFailureFixture.adapter.diagnostics().lastErrorStack.includes("pixi_render_worker.js:42:7") &&
      staleFailureFixture.adapter.diagnostics().lastErrorSource === "pixi_render_worker.js" &&
      staleFailureFixture.incidents.length === 1,
    "worker failures preserve bounded cause/source diagnostics and immediately notify the match reporter",
  );
  staleFailureFixture.adapter.destroy();

  const postFailureFixture = createFixture();
  postFailureFixture.assembler.reset({ map: postFailureFixture.map, generation: 2 });
  const postFailureFrame = assemble(postFailureFixture.assembler, 2);
  postFailureFixture.worker.postMessage = () => { throw new Error("worker post failed"); };
  const priorPostFailureConsoleError = console.error;
  let postFailure;
  try {
    console.error = () => {};
    postFailure = postFailureFixture.adapter.render(postFailureFrame);
  } finally {
    console.error = priorPostFailureConsoleError;
  }
  assert((await postFailure.settled).status === PRESENTATION_OUTCOME.FAILED,
    "a synchronous worker command failure returns a settled presentation instead of escaping render");
  assert(postFailureFixture.adapter.terminalFailure()?.message === "worker post failed"
      && postFailureFixture.worker.terminated === 1,
    "synchronous worker command failure tears down the unusable renderer lifecycle");
  postFailureFixture.adapter.destroy();
}

async function editorFatalContracts() {
  const fixture = createFixture({ surface: "mapEditor" });
  const { adapter } = fixture;
  const record = (frameId) => ({
    version: 1,
    generation: 1,
    frameId,
    camera: { x: 0, y: 0, zoom: 1 },
    terrainUpdate: null,
    overlay: null,
  });
  const pending = adapter.presentEditor(record(1));
  const priorConsoleError = console.error;
  try {
    console.error = () => {};
    adapter._failFatal(new Error("planned editor worker failure"));
  } finally {
    console.error = priorConsoleError;
  }
  assert((await pending).status === PRESENTATION_OUTCOME.FAILED,
    "an in-flight editor frame reports fatal worker failure instead of teardown");
  const afterFatal = await adapter.presentEditor(record(2));
  assert(afterFatal.status === PRESENTATION_OUTCOME.FAILED
      && afterFatal.error?.message === "planned editor worker failure",
    "later editor submissions preserve the fatal error and do not masquerade as destroyed");
  assert(adapter.terminalFailure()?.message === "planned editor worker failure",
    "the match owner can distinguish a terminal renderer failure from a bounded frame failure");
  adapter.destroy();
}

async function measurementBoundaryContracts() {
  const savedPerformance = globalThis.performance;
  let now = 5;
  globalThis.performance = { timeOrigin: 1000, now: () => now };
  try {
    const fixture = createFixture();
    const { adapter, worker, assembler } = fixture;
    const frame1 = assemble(assembler, 1);
    const frame2 = assemble(assembler, 2);
    const first = adapter.render(frame1);
    now = 15;
    const second = adapter.render(frame2);
    now = 65;
    worker.present(frame1);
    await first.settled;
    const dispatchedSecond = worker.frameMessages().at(-1);
    assert(dispatchedSecond.payload.submittedAtMs === 1015,
      "a pending frame keeps its host-acceptance timestamp through later packet construction and dispatch");
    now = 75;
    worker.present(frame2);
    await second.settled;
    assert(adapter.diagnostics().displayAgeMs.p95 >= 60,
      "display age includes host-pending and main-thread packet work instead of starting after cloning");
    adapter.destroy();
  } finally {
    globalThis.performance = savedPerformance;
  }
}

function createFixture({ surface = "match" } = {}) {
  const map = { width: 2, height: 2, tileSize: 32, terrain: [0, 1, 2, 3], resources: [] };
  const assembler = new PresentationFrameAssembler({ map });
  const worker = new FakeWorker();
  const canvas = {
    style: {},
    width: 640,
    height: 480,
    removed: 0,
    remove() { this.removed += 1; root.children = root.children.filter((child) => child !== this); },
  };
  const root = {
    children: [canvas],
    appendChild(child) { this.children.push(child); },
    querySelector(selector) { return this.children.find((child) => selector === ".renderer-fatal-error" && child.className === "renderer-fatal-error") || null; },
  };
  globalThis.document = {
    createElement() {
      return {
        className: "",
        role: "",
        textContent: "",
        setAttribute(name, value) { this[name] = value; },
      };
    },
  };
  const incidents = [];
  const adapter = new PixiWorkerPresentationAdapter(root, canvas, worker, {
    state: () => ({ resources: {}, _curById: new Map(), _prevById: new Map() }),
    staticMap: () => assembler.staticMap,
    renderIncident: (incident) => incidents.push(incident),
  }, { surface });
  return { adapter, worker, canvas, root, assembler, map, incidents };
}

function assemble(assembler, tick) {
  const camera = new Camera(640, 480);
  camera.setBounds(64, 64, 640, 480);
  return assembler.assemble({
    frameContext: { alpha: 1, interpolatedEntities: [] },
    projection: camera.projectionSnapshot(),
    fog: {
      visibleGrid: new Uint8Array([1, 1, 1, 1]),
      exploredGrid: new Uint8Array([1, 1, 1, 1]),
      visibleRevision: tick,
      exploredRevision: tick,
    },
    sourceTick: tick,
    visualTimeMs: tick * 16,
  });
}

class FakeWorker {
  constructor() {
    this.listeners = new Map();
    this.messages = [];
    this.terminated = 0;
  }
  addEventListener(type, listener) { this.listeners.set(type, listener); }
  removeEventListener(type, listener) { if (this.listeners.get(type) === listener) this.listeners.delete(type); }
  postMessage(message) { this.messages.push(message); }
  terminate() { this.terminated += 1; }
  emit(message) { this.listeners.get("message")?.({ data: message }); }
  frameMessages() { return this.messages.filter((message) => message.type === "frame"); }
  present(frame, extra = {}) {
    this.emit(response(RENDER_WORKER_RESPONSE.PRESENTED, frame.generation, {
      frameId: frame.frameId,
      workerUpdateMs: 2,
      workerPresentMs: 1,
      queueAgeMs: 1,
      displayAgeMs: 3,
      ...extra,
    }));
  }
}

function response(type, generation, payload) {
  return { version: 1, type, generation, payload };
}

function restoreGlobal(name, value) {
  if (value === undefined) delete globalThis[name];
  else globalThis[name] = value;
}

async function assertRejects(promise, message) {
  try {
    await promise;
  } catch {
    return;
  }
  throw new Error(message);
}

try {
  await queueAndLifecycleContracts();
  await generationAndFatalContracts();
  await editorFatalContracts();
  await measurementBoundaryContracts();
} finally {
  restoreGlobal("document", priorDocument);
  restoreGlobal("requestAnimationFrame", priorRaf);
  restoreGlobal("__rtsRenderWorkerStats", priorStats);
  restoreGlobal("__rtsRenderWorkerControl", priorControl);
}
