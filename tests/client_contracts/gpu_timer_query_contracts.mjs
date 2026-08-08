import { assert } from "./assertions.mjs";
import {
  AsyncGpuTimerQueries,
  runRafIndependentGpuSamples,
} from "../../client/src/renderer/gpu_timer_queries.js";

class FakeGl {
  constructor() {
    this.QUERY_RESULT_AVAILABLE = 1;
    this.QUERY_RESULT = 2;
    this.extension = { TIME_ELAPSED_EXT: 3, GPU_DISJOINT_EXT: 4 };
    this.nextId = 0;
    this.elapsedNs = 2_500_000;
    this.available = true;
    this.disjoint = false;
    this.deleted = [];
    this.beginCount = 0;
    this.endCount = 0;
  }
  getExtension(name) { return name === "EXT_disjoint_timer_query_webgl2" ? this.extension : null; }
  createQuery() { this.nextId += 1; return { id: this.nextId }; }
  beginQuery() { this.beginCount += 1; }
  endQuery() { this.endCount += 1; }
  getParameter() { return this.disjoint; }
  getQueryParameter(_query, parameter) {
    return parameter === this.QUERY_RESULT_AVAILABLE ? this.available : this.elapsedNs;
  }
  deleteQuery(query) { this.deleted.push(query.id); }
}

{
  const gl = new FakeGl();
  const timer = new AsyncGpuTimerQueries(gl, { maxPending: 2, maxSamples: 2 });
  let draws = 0;
  timer.measure("shadow.units", () => { draws += 1; });
  assert(timer.summary().pending === 1 && timer.summary().groups.length === 0,
    "GPU timing never synchronously reads a newly issued query");
  assert(timer.poll() === 1 && timer.summary().groups[0]?.avgMs === 2.5,
    "GPU timing collects elapsed nanoseconds only after availability");
  timer.measure("shadow.units", () => { draws += 1; });
  timer.measure("shadow.units", () => { draws += 1; });
  timer.poll();
  assert(timer.summary().groups[0]?.samples === 2,
    "GPU timing keeps its diagnostic sample history bounded");
  assert(draws === 3 && gl.beginCount === 3 && gl.endCount === 3,
    "GPU timing wraps every accepted diagnostic draw without changing execution");
  timer.destroy();
  assert(gl.deleted.length === 3 && timer.summary().supported === false,
    "GPU timing deletes completed and pending query resources on teardown");
}

{
  const gl = new FakeGl();
  const timer = new AsyncGpuTimerQueries(gl);
  gl.available = false;
  timer.measure("shadow.terrain", () => {});
  assert(timer.poll() === 0 && timer.summary().pending === 1,
    "unavailable GPU results remain pending without a blocking read");
  gl.disjoint = true;
  timer.poll();
  assert(timer.summary().pending === 0 && timer.summary().disjoint === 1,
    "disjoint GPU intervals are discarded rather than reported as timings");
}

{
  const gl = new FakeGl();
  const timer = new AsyncGpuTimerQueries(gl);
  const calls = [];
  const summary = await runRafIndependentGpuSamples({
    timer,
    label: "shadow.microbenchmark",
    count: 3,
    warmup: 2,
    sample: (index, warmup) => calls.push({ index, warmup }),
    yieldTask: async () => {},
  });
  assert(calls.length === 5 && calls.filter((call) => call.warmup).length === 2,
    "GPU microbenchmark performs untimed warmup before bounded measured samples");
  assert(summary.groups[0]?.samples === 3,
    "GPU microbenchmark sample count is independent of requestAnimationFrame cadence");
}
