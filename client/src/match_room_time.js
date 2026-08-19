import { syncRenderClockToRoomTime } from "./visual_clock.js";

export function applyRoomTimeState(match, state) {
  match.roomTimeControls?.applyRoomTimeState(state);
  syncRenderClockToRoomTime(match.renderClock, state);
  const speed = Number(state?.speed);
  const ended = state?.ended === true;
  if (state?.paused === true || (Number.isFinite(speed) && speed <= 0) || ended) {
    match.combatAudio?.updateWorldCombatBed(false);
  }
}
