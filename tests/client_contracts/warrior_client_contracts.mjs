import { assert } from "./assertions.mjs";
import { VisualEffectBuffers } from "../../client/src/state_visual_effects.js";
import { EVENT, WEAPON_KIND } from "../../client/src/protocol.js";

const effects = new VisualEffectBuffers();
effects.applySnapshotEvents([{
  e: EVENT.ATTACK,
  from: 10,
  to: 11,
  weaponKind: WEAPON_KIND.WARRIOR_SWORD,
}], 1000, () => null);

assert(effects.muzzleFlashes.length === 0, "Warrior sword attacks create no tracer or muzzle flash");
assert(
  effects.weaponRecoilKind(10) === WEAPON_KIND.WARRIOR_SWORD,
  "Warrior sword attacks still drive the authored swipe frame",
);
