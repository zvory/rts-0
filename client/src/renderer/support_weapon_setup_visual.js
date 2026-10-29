import { KIND, SETUP, STATE } from "../protocol.js";
import { ARTILLERY_DEPLOYED_WEAPON_ANIM_MS, DEPLOYED_WEAPON_ANIM_MS } from "./palette.js";
import { clamp01, smoothstep01 } from "./shared.js";

export function supportWeaponSetupVisual(e, records, now) {
  if (e.kind === KIND.MORTAR_TEAM) {
    const deployed = e.state === STATE.MOVE ? 0 : 1;
    return {
      prongFactor: deployed,
      frameProgress: deployed,
      barrel: deployed === 1,
    };
  }
  const setupState = e.setupState || SETUP.PACKED;
  const prev = records.get(e.id);
  if (!prev || prev.state !== setupState) {
    records.set(e.id, { state: setupState, changedAt: now });
  }
  const rec = records.get(e.id);
  const elapsed = now - rec.changedAt;
  const durationMs = e.kind === KIND.ARTILLERY
    ? ARTILLERY_DEPLOYED_WEAPON_ANIM_MS
    : DEPLOYED_WEAPON_ANIM_MS;
  const progress = clamp01(elapsed / durationMs);
  const t = smoothstep01(progress);

  if (setupState === SETUP.SETTING_UP) {
    return { prongFactor: t, frameProgress: progress, barrel: false };
  }
  if (setupState === SETUP.TEARING_DOWN) {
    return { prongFactor: 1 - t, frameProgress: 1 - progress, barrel: false };
  }
  if (setupState === SETUP.DEPLOYED) {
    return { prongFactor: 1, frameProgress: 1, barrel: e.state !== STATE.MOVE };
  }
  return { prongFactor: 0, frameProgress: 0, barrel: false };
}
