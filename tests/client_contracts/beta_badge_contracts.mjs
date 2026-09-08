import { shouldShowBetaBadge } from "../../client/src/beta_badge.js";
import { assert } from "./assertions.mjs";

const betaHostname = "rts-0-zvorygin-beta.fly.dev";
const standardLobby = {
  labCatalogLaunch: null,
  labHandoffLaunch: null,
  labLaunch: null,
  replayLaunch: null,
  matchLaunch: null,
  snapshotStreamLaunch: null,
  stressTestLaunch: null,
  devWatch: null,
};

assert(
  shouldShowBetaBadge(standardLobby, betaHostname, true),
  "beta badge is visible in the beta deployment's standard lobby",
);
assert(
  !shouldShowBetaBadge(standardLobby, "example.com", true),
  "beta badge is hidden outside the beta deployment",
);
assert(
  !shouldShowBetaBadge(standardLobby, betaHostname, false),
  "beta badge is hidden outside the lobby screen",
);

for (const launchProperty of [
  "labCatalogLaunch",
  "labHandoffLaunch",
  "labLaunch",
  "replayLaunch",
  "matchLaunch",
  "snapshotStreamLaunch",
  "stressTestLaunch",
  "devWatch",
]) {
  assert(
    !shouldShowBetaBadge({ ...standardLobby, [launchProperty]: {} }, betaHostname, true),
    `beta badge is hidden for ${launchProperty}`,
  );
}
