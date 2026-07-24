import { NOTICE } from "./protocol.js";

export const TOAST_MS = 2600;
export const UNDER_ATTACK_ID = "under_attack";
export const ALERT_PREFIX = "alert:";
export const VIEWPORT_ALERT_MARGIN_PX = 64;

export function noticeAlertId(msg) {
  const m = String(msg || "").trim().toLowerCase();
  if (!m.startsWith(ALERT_PREFIX)) return "";
  return m.slice(ALERT_PREFIX.length).trim();
}



export function noticeDisplayText(msg) {
  const raw = String(msg || "");
  if (raw === NOTICE.ARTILLERY_STEEL_SHORTAGE) return "Not enough steel";
  const id = noticeAlertId(raw);
  if (id === UNDER_ATTACK_ID) return "Under attack";
  if (id) return id.replaceAll("_", " ");
  return raw;
}

