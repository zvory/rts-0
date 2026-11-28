import { STATS } from "../config.js";
import { KIND, WEAPON_KIND } from "../protocol.js";
import { gfxNoFill, gfxCircle, gfxStrokeLine, gfxFill, gfxStroke } from "./native_graphics.js";
import { clamp01, rendererVisualNow } from "./shared.js";
import { attackFeedbackKindForWeapon, attackFeedbackOriginForWeapon } from "./attack_feedback_origin.js";
import { muzzleFeedbackStyle } from "./weapon_feedback_style.js";
import { isDiscipleProjectile } from "./disciple_projectiles.js";
import { experimentalRiflemanMuzzle } from "./rigs/rifleman_experimental_png_strip.js";
import { rigContainerScale } from "./rigs/animation.js";

export function _drawMuzzleFlashes(state, colorByOwner) {
  const g = this._feedbackGfx;
  if (!state || typeof state.liveMuzzleFlashes !== "function") return;
  const now = rendererVisualNow(this);
  const shotTimes = this._experimentalShotTimes ??= new Map();
  for (const [key, timing] of shotTimes) {
    if (now > timing.expiresAt) shotTimes.delete(key);
  }
  const flashes = state.liveMuzzleFlashes(now);
  this._discipleProjectiles?.update(flashes, now, g, colorByOwner);
  if (!flashes.length) return;

  for (const f of flashes) {
    if (isDiscipleProjectile(f.weaponKind)) continue;
    const attacker = state.entityById(f.from);
    if (!attacker) continue;
    const target = state.entityById(f.to);
    const targetPos = f.shotOrigin ? f.targetPos : (target || f.targetPos);

    const experimental = attacker.kind === KIND.RIFLEMAN && this._experimentalRiflemanFrameStrip?.();
    // The setting can be on while the atlas is still loading (or failed to load).
    // Keep normal-art feedback on its original clock during that fallback.
    let createdAt = f.createdAt;
    if (f.experimentalDelayMs) {
      const key = `${f.from}:${f.createdAt}`;
      let timing = shotTimes.get(key);
      if (!timing) {
        timing = { createdAt: f.createdAt - (experimental ? 0 : f.experimentalDelayMs),
          expiresAt: f.createdAt + 240 };
        shotTimes.set(key, timing);
      }
      // Loading the atlas midway through a shot must not play that shot twice.
      createdAt = timing.createdAt;
    }
    const age = now - createdAt;
    if (age < 0 || age > 240) continue;
    const t = clamp01(age / 240);
    const fade = 1 - t;

    const feedbackKind = attackFeedbackKindForWeapon(attacker.kind, f.weaponKind);
    const style = muzzleFeedbackStyle(feedbackKind, f.weaponKind);
    const baseR = style.flashRadius;
    if (baseR <= 0) continue;

    const stat = STATS[feedbackKind] || STATS[attacker.kind] || {};
    const origin = experimental
      ? experimentalRiflemanMuzzle(attacker, rigContainerScale(this._unitRenderContexts?.get?.(attacker.id))) : attackFeedbackOriginForWeapon({
      definitionsByKind: this._liveRigDefinitionsByKind,
      attacker,
      weaponKind: f.weaponKind,
      targetPos,
      state,
      now,
      map: this._map,
      stat,
    });
    const mx = origin.x;
    const my = origin.y;

    if (targetPos && f.shotOrigin) {
      const sx = experimental ? mx : f.shotOrigin[0], sy = experimental ? my : f.shotOrigin[1];
      const dx = targetPos.x - sx, dy = targetPos.y - sy;
      const distance = Math.hypot(dx, dy);
      const head = Math.min(distance, age * 2.7);
      const tail = Math.max(12, head - 7.5);
      if (distance > 0 && head > tail && age * 2.7 < distance + 7.5) {
        gfxStrokeLine(g, sx + dx * tail / distance, sy + dy * tail / distance,
          sx + dx * head / distance, sy + dy * head / distance, 1.1, 0xffd84a, 0.98);
      }
    } else if (targetPos) {
      gfxStrokeLine(g, mx, my, targetPos.x, targetPos.y,
        style.tracerWidth, style.tracerColor, style.tracerAlpha * fade);
      if (style.tracerCoreWidth > 0) {
        gfxStrokeLine(g, mx, my, targetPos.x, targetPos.y,
          style.tracerCoreWidth, style.tracerCoreColor, style.tracerCoreAlpha * fade);
      }

    }

    const rigOwnsTankCannonFlash = attacker.kind === KIND.TANK && f.weaponKind !== WEAPON_KIND.TANK_COAX;
    const suppressCircularFlash = feedbackKind === KIND.RIFLEMAN && !experimental;
    if (!rigOwnsTankCannonFlash && !suppressCircularFlash) {
      // Flash: bright core that scales up slightly then fades.
      const r = baseR * (0.7 + 0.45 * t);
      gfxStroke(g, 0);
      gfxFill(g, 0xfff2a8, 0.85 * fade);
      gfxCircle(g, mx, my, r);
      gfxNoFill(g);
      gfxFill(g, 0xffd84a, 0.55 * fade);
      gfxCircle(g, mx, my, r * 0.55);
      gfxNoFill(g);
    }
  }
}
