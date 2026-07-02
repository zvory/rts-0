import { KIND, WEAPON_KIND } from "../protocol.js";
import { muzzleFlashRadius } from "./shared.js";

const TANK_COAX_FLASH_RADIUS = 6;
const TANK_COAX_TRACER_COLOR = 0xfff0a6;
const TANK_COAX_TRACER_CORE_COLOR = 0xffffff;
const TANK_COAX_TRACER_TAIL_COLOR = 0xffcc47;

export function muzzleFeedbackStyle(feedbackKind, weaponKind) {
  if (weaponKind === WEAPON_KIND.TANK_COAX) {
    return {
      flashRadius: TANK_COAX_FLASH_RADIUS,
      tracerWidth: 1.8,
      tracerColor: TANK_COAX_TRACER_COLOR,
      tracerAlpha: 0.98,
      tracerCoreWidth: 0.75,
      tracerCoreColor: TANK_COAX_TRACER_CORE_COLOR,
      tracerCoreAlpha: 0.72,
      tailWidth: 0.9,
      tailColor: TANK_COAX_TRACER_TAIL_COLOR,
      tailAlpha: 0.38,
    };
  }
  return {
    flashRadius: muzzleFlashRadius(feedbackKind),
    tracerWidth: feedbackKind === KIND.ANTI_TANK_GUN ? 2.5 : 1.5,
    tracerColor: 0xffe066,
    tracerAlpha: 0.92,
    tracerCoreWidth: 0,
    tracerCoreColor: 0xffffff,
    tracerCoreAlpha: 0,
    tailWidth: feedbackKind === KIND.ANTI_TANK_GUN ? 1.4 : 1.0,
    tailColor: 0xffd84a,
    tailAlpha: 0.46,
  };
}
