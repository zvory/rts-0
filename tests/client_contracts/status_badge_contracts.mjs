import { StatusBadge } from "../../client/src/status_badge.js";
import { assert } from "./assertions.mjs";

const root = { innerHTML: "" };
const badge = new StatusBadge(root);

assert(root.innerHTML === "", "status badge stays empty until match metrics arrive");

badge.setMatchMetrics({
  latencyMs: 28,
  fps: 60,
  fpsOneMinute: 58,
  serverTickMs: 4,
  serverLagMs: 1,
  jitterMs: 3,
  issues: {
    latency: { active: false, count: 0 },
    slowTick: { active: false, count: 0 },
    headOfLine: { active: false, count: 0 },
    jitter: { active: false, count: 0 },
  },
});

assert(root.innerHTML.includes("rtt 28ms"), "status badge retains live latency");
assert(root.innerHTML.includes("fps 60"), "status badge retains live FPS");
assert(root.innerHTML.includes("1m fps 58"), "status badge retains rolling FPS");
assert(root.innerHTML.includes("tick 4ms"), "status badge retains server tick time");
assert(root.innerHTML.includes("lag 1ms"), "status badge retains server lag");
assert(root.innerHTML.includes("jit 3ms"), "status badge retains jitter");
assert(root.innerHTML.includes("issues ok"), "status badge retains issue status");
assert(!root.innerHTML.includes("status-badge-build"), "status badge omits the build SHA");
assert(!root.innerHTML.includes("status-badge-copy"), "status badge omits the copy control");

badge.clearMatchMetrics();
assert(root.innerHTML === "", "clearing match metrics empties the status badge");
