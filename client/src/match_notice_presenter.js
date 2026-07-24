import { noticeSoundId } from "./audio.js";
import {
  UNDER_ATTACK_ID,
  VIEWPORT_ALERT_MARGIN_PX,
  noticeAlertId,
  noticeDisplayText,
} from "./alerts.js";
import { NOTICE_SEVERITY } from "./protocol.js";

export const UNDER_ATTACK_BUCKET_PX = 960;
export const UNDER_ATTACK_COOLDOWN_MS = 10000;
export const STEEL_SHORTAGE_NOTICE = "Not enough steel";
export const STEEL_SHORTAGE_COOLDOWN_MS = 60000;

/**
 * Match-scoped presentation policy for existing server Notice events.
 * Owns transient incident admission so no notice state survives a rematch.
 */
export class MatchNoticePresenter {
  constructor({
    toast,
    minimap,
    audio,
    isReplay,
    isSpectator,
    pointInViewport,
    now = () => performance.now(),
  }) {
    this.toast = toast;
    this.minimap = minimap;
    this.audio = audio;
    this.isReplay = isReplay;
    this.isSpectator = isSpectator;
    this.pointInViewport = pointInViewport;
    this.now = now;
    this.lastUnderAttackByBucket = new Map();
    this.lastSteelShortageAt = null;
  }

  present(ev) {
    if (!ev?.msg) return false;

    const alertId = noticeAlertId(ev.msg);
    const severity = ev.severity || (alertId ? NOTICE_SEVERITY.ALERT : NOTICE_SEVERITY.INFO);
    const hasPos = Number.isFinite(ev.x) && Number.isFinite(ev.y);
    const isAlert = severity === NOTICE_SEVERITY.ALERT || !!alertId;

    if (alertId === UNDER_ATTACK_ID && !this._admitUnderAttack(ev, hasPos)) return false;
    if (ev.msg === STEEL_SHORTAGE_NOTICE && !this._admitSteelShortage()) return false;

    this.toast?.(noticeDisplayText(ev.msg));
    if (isAlert) {
      if (hasPos) this.minimap?.ping(ev.x, ev.y, severity, alertId === UNDER_ATTACK_ID);
      else this.minimap?.pulseBorder();
    }

    if (this.isReplay?.() || this.isSpectator?.() || !this.audio) return true;
    if (
      alertId === UNDER_ATTACK_ID &&
      hasPos &&
      this.pointInViewport?.(ev.x, ev.y, VIEWPORT_ALERT_MARGIN_PX)
    ) {
      return true;
    }

    const soundId = noticeSoundId(ev.msg);
    if (!soundId) return true;

    const opts = {
      category: isAlert ? "alert" : "ui",
      priority: isAlert ? 3 : 1,
      alertId,
      duck: true,
    };
    if (hasPos) {
      opts.alertX = ev.x;
      opts.alertY = ev.y;
    }
    if (alertId === UNDER_ATTACK_ID) opts.cooldownMs = 0;
    this.audio.play(soundId, opts);
    return true;
  }

  _admitUnderAttack(ev, hasPos) {
    const bucket = hasPos
      ? `${Math.floor(ev.x / UNDER_ATTACK_BUCKET_PX)}:${Math.floor(ev.y / UNDER_ATTACK_BUCKET_PX)}`
      : "unpositioned";
    const now = this.now();
    const last = this.lastUnderAttackByBucket.get(bucket);
    if (last != null && now - last < UNDER_ATTACK_COOLDOWN_MS) return false;
    this.lastUnderAttackByBucket.set(bucket, now);
    return true;
  }

  _admitSteelShortage() {
    const now = this.now();
    if (
      this.lastSteelShortageAt != null &&
      now - this.lastSteelShortageAt < STEEL_SHORTAGE_COOLDOWN_MS
    ) {
      return false;
    }
    this.lastSteelShortageAt = now;
    return true;
  }
}
