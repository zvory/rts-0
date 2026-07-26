import { HUD } from "./hud.js";

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
    match.rendererBackendBundle.unitIconSvgForKind,
  );
}
