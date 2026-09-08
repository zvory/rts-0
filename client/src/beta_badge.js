import { dom } from "./bootstrap.js";

const BETA_HOSTNAME = "rts-0-zvorygin-beta.fly.dev";

export function applyBetaBadge(app, hostname = window.location.hostname) {
  if (!dom.betaBadge) return;
  const standardLobbyUi = !dom.lobbyScreen.hidden && !app.labCatalogLaunch &&
    !app.labHandoffLaunch && !app.labLaunch && !app.replayLaunch &&
    !app.matchLaunch && !app.snapshotStreamLaunch && !app.stressTestLaunch &&
    !app.devWatch;
  dom.betaBadge.hidden = hostname !== BETA_HOSTNAME || !standardLobbyUi;
}
