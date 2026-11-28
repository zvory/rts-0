import { loadTeamPaintTexture } from "./team_paint.js";
import { KIND } from "../../protocol.js";
import { loadColorAdjustedTexture } from "./color_adjusted_texture.js";
import { loadWorkerSafeTexture } from "../raster_primitives.js";
import { frameStripRuntimeColorAdjustment } from "./frame_strip_color_profile.js";
import { liveRigKeyForEntity, LOADED_RIFLEMAN_RIG_KEY } from "./live_routing.js";
import { MACHINE_GUNNER_PNG_FRAME_STRIP } from "./machine_gunner_png_strip.js";
import { RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP } from "./rifleman_panzerfaust_png_strip.js";
import { RIFLEMAN_PNG_FRAME_STRIP } from "./rifleman_png_strip.js";
import { SCOUT_PLANE_PNG_FRAME_STRIP } from "./scout_plane_png_strip.js";
import { WARRIOR_PNG_FRAME_STRIP } from "./warrior_png_strip.js";

import {
  WIND_FRAME_STRIP_KEY, MAGIC_FRAME_STRIP_KEY, WAR_FRAME_STRIP_KEY,
  WIND_PNG_FRAME_STRIP, MAGIC_PNG_FRAME_STRIP, WAR_PNG_FRAME_STRIP,
  WIND_MASTER_FRAME_STRIP_KEY, MAGIC_MASTER_FRAME_STRIP_KEY, WAR_MASTER_FRAME_STRIP_KEY,
  WIND_MASTER_PNG_FRAME_STRIP, MAGIC_MASTER_PNG_FRAME_STRIP, WAR_MASTER_PNG_FRAME_STRIP,
} from "./disciple_png_strips.js";

const LIVE_FRAME_STRIPS = Object.freeze([
  [KIND.MACHINE_GUNNER, MACHINE_GUNNER_PNG_FRAME_STRIP],
  [LOADED_RIFLEMAN_RIG_KEY, RIFLEMAN_PANZERFAUST_PNG_FRAME_STRIP],
  [KIND.RIFLEMAN, RIFLEMAN_PNG_FRAME_STRIP],
  [KIND.SCOUT_PLANE, SCOUT_PLANE_PNG_FRAME_STRIP],
  [KIND.WARRIOR, WARRIOR_PNG_FRAME_STRIP],
  [WIND_FRAME_STRIP_KEY, WIND_PNG_FRAME_STRIP],
  [MAGIC_FRAME_STRIP_KEY, MAGIC_PNG_FRAME_STRIP],
  [WAR_FRAME_STRIP_KEY, WAR_PNG_FRAME_STRIP],
  [WIND_MASTER_FRAME_STRIP_KEY, WIND_MASTER_PNG_FRAME_STRIP],
  [MAGIC_MASTER_FRAME_STRIP_KEY, MAGIC_MASTER_PNG_FRAME_STRIP],
  [WAR_MASTER_FRAME_STRIP_KEY, WAR_MASTER_PNG_FRAME_STRIP],
]);

export function createLiveFrameStrips() {
  const strips = new Map();
  for (const [kind, strip] of LIVE_FRAME_STRIPS) {
    if (strip?.enabled) strips.set(kind, strip);
  }
  return strips;
}

// Each authoritative kind routes to its own production strip.
export function liveFrameStripKeyForEntity(entity) {
  if (entity?.daoMaster) {
    if (entity.kind === KIND.WIND_ADEPT) return WIND_MASTER_FRAME_STRIP_KEY;
    if (entity.kind === KIND.WAR_ADEPT) return WAR_MASTER_FRAME_STRIP_KEY;
    if (entity.kind === KIND.MAGIC_ADEPT) return MAGIC_MASTER_FRAME_STRIP_KEY;
  }
  return liveRigKeyForEntity(entity);
}

export function liveFrameStripFor(strips, kind) {
  return strips?.get?.(kind) ?? null;
}

export function loadFrameStripTexture(pixi, strip) {
  if (!pixi || !strip?.image) return Promise.resolve(null);
  const adjustment = frameStripRuntimeColorAdjustment(strip);
  if (strip.teamPaint) return loadTeamPaintTexture(pixi, strip, adjustment);
  const frameWidth = positiveDimension(strip.frameWidth, 1);
  const frameHeight = positiveDimension(strip.frameHeight, 1);
  const frameCount = Math.max(1, Math.trunc(positiveDimension(strip.frameCount, 1)));
  return loadColorAdjustedTexture(pixi, {
    image: strip.image,
    adjustment,
    widthFallbacks: [frameWidth * frameCount],
    heightFallbacks: [frameHeight],
    rawLoad: () => loadRawFrameStripTexture(pixi, strip),
    errorLabel: "frame strip image",
  });
}

function loadRawFrameStripTexture(pixi, strip) {
  return loadWorkerSafeTexture(pixi, strip.image);
}

function positiveDimension(value, fallback) {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}
