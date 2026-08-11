import { BirthdayBanner } from "./birthday_banner.js";
import { KIND } from "./protocol.js";
import { mountLiveUnitIcon } from "./renderer/rigs/unit_icon_sources.js";
import { sampleWeaponRecoilCycle } from "./weapon_recoil_cycle.js";

export function createBirthdayBanner(dom) {
  return new BirthdayBanner(dom.birthdayBanner, {
    tankIconElements: [dom.birthdayTankLeftIcon, dom.birthdayTankRightIcon],
    mountTankIcon: (element, index) => mountLiveUnitIcon(element, KIND.TANK, {
      teamColor: "#c7d07a",
      delayMs: 250 + index * 1200,
      sampleCycle: (elapsedMs) => sampleWeaponRecoilCycle(KIND.TANK, elapsedMs),
    }),
  });
}
