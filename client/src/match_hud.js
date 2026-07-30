import { HUD } from "./hud.js";
import { TabMenu } from "./tab_menu.js";

/** Compose the DOM HUD from Match-owned services and renderer presentation assets. */
export function createMatchHud(match, rootEl) {
  return new HUD(
    rootEl,
    match.state,
    match.commandInteraction,
    match.audio,
    match.hotkeyProfiles,
    match.clientIntent,
    match.controlPolicy,
    match.camera,
    match.apmTracker,
    match.rendererBackendBundle.unitIconMarkupForKind,
  );
}

/** Compose the transient, browser-local hold-Tab menu for a controllable live player. */
export function createMatchTabMenu(match, rootEl, button) {
  if (!matchTabMenuAvailable(match)) return null;
  return new TabMenu({
    root: rootEl,
    button,
    settings: match.settings,
    hotkeyProfiles: match.hotkeyProfiles,
  });
}

export function matchTabMenuAvailable(match) {
  return !!match &&
    match.running !== false &&
    match.capabilities?.commands?.gameplay === true &&
    !match.replayViewer &&
    !match.state?.spectator &&
    !match.labMetadata &&
    !match.devWatch &&
    match.net?.offline !== true;
}
