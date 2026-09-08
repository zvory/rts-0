const BETA_HOSTNAME = "rts-0-zvorygin-beta.fly.dev";

export function shouldShowBetaBadge(app, hostname, lobbyVisible) {
  const standardLobbyUi = lobbyVisible && !app.labCatalogLaunch &&
    !app.labHandoffLaunch && !app.labLaunch && !app.replayLaunch &&
    !app.matchLaunch && !app.snapshotStreamLaunch && !app.stressTestLaunch &&
    !app.devWatch;
  return hostname === BETA_HOSTNAME && standardLobbyUi;
}

export function applyBetaBadge(app, dom, hostname = window.location.hostname) {
  if (!dom.betaBadge) return;
  dom.betaBadge.hidden = !shouldShowBetaBadge(app, hostname, !dom.lobbyScreen.hidden);
}
