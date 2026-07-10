import fs from "node:fs";

import { assert } from "./assertions.mjs";

const appSource = fs.readFileSync(new URL("../../client/src/app.js", import.meta.url), "utf8");

assert(
  appSource.includes("S.OBSERVATION_READY") && appSource.includes("lastObservationRunId"),
  "all-AI completion metadata is retained through post-match replay",
);
assert(
  appSource.includes("renderObservationId") && appSource.includes("server lag logs"),
  "score screen exposes the replay/log observation handoff",
);
