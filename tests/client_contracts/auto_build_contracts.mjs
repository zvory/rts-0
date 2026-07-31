import { autoBuild } from "../../client/src/state_auto_build.js";
import { assertDeepEqual } from "./assertions.mjs";

assertDeepEqual(
  autoBuild(null),
  { paused: false, reserveSteel: 0, reserveOil: 0, ack: 0 },
  "Auto-Build client fallback starts running with zero Steel and Oil floors",
);
