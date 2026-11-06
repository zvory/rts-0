import { ABILITY, KIND } from "../protocol.js";
import { MORTAR_FIRE_COOLDOWN_TICKS, STATS } from "../config.js";

// Match the command card's circular face, single accent hand, and clockwise
// progress. The server discloses this cooldown only to the mortar's team (or
// explicitly privileged observer perspectives); never infer it from launches.
export function drawMortarCooldownClock(entity) {
  if (entity.kind !== KIND.MORTAR_TEAM) return;
  const left = entity.abilities?.find((entry) => entry.ability === ABILITY.MORTAR_FIRE)?.cooldownLeft;
  if (!Number.isFinite(left) || left <= 0) return;
  const progress = 1 - Math.min(1, left / MORTAR_FIRE_COOLDOWN_TICKS);
  const angle = progress * Math.PI * 2;
  const g = this._slot("cooldownClocks", entity.id);
  g.position.set(entity.x, entity.y - (STATS[entity.kind]?.size || 9) - 23);
  g.alpha = 0.65;
  g.circle(0, 0, 9).fill({ color: 0x11110f, alpha: 0.92 })
    .stroke({ width: 1.5, color: 0x5b5341 });
  g.moveTo(0, 0).lineTo(Math.sin(angle) * 6, -Math.cos(angle) * 6)
    .stroke({ width: 1.5, color: 0xc7d07a });
}
