import { assert } from "./assertions.mjs";
import {
  ApmTracker,
  LIVE_APM_WINDOW_SECONDS,
} from "../../client/src/apm_tracker.js";

const tickHz = 30;
const tracker = new ApmTracker({ tickHz });

tracker.recordAction(10);
assert(tracker.currentApm(10) === 6,
  "live APM extrapolates one command in the ten-second window to six actions per minute");

tracker.recordAction(20);
tracker.recordAction(20);
assert(tracker.currentApm(20) === 18,
  "live APM counts each submitted command once regardless of command payload shape");

assert(tracker.currentApm(tickHz * LIVE_APM_WINDOW_SECONDS + 10) === 12,
  "live APM expires commands outside the rolling window");

tracker.reset();
assert(tracker.currentApm(1000) === 0, "live APM resets between matches");
