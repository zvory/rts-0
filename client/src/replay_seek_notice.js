import { TICK_HZ } from "./config.js";

export function formatReplaySeekNotice({ fromTick, targetTick } = {}) {
  const from = Number(fromTick);
  const target = Number(targetTick);
  if (!Number.isFinite(from) || !Number.isFinite(target)) return "";

  const deltaTicks = Math.abs(target - from);
  if (deltaTicks === 0) return "Seeking to the current replay position…";

  const rawSeconds = deltaTicks / TICK_HZ;
  const seconds = rawSeconds < 1
    ? Math.max(0.03, Math.round(rawSeconds * 100) / 100)
    : rawSeconds < 10
      ? Math.round(rawSeconds * 10) / 10
      : Math.round(rawSeconds);
  const unit = seconds === 1 ? "second" : "seconds";
  const direction = target > from ? "forward" : "backward";
  return `Seeking ${direction} ${seconds} ${unit}…`;
}
