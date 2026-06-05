// Pixels the cursor must travel before a press becomes a box-drag (vs a click).
export const DRAG_THRESHOLD_PX = 4;
// Forgiving extra padding around entity hit areas, in world px.
export const HIT_PAD_PX = 3;
// Large distance bonus so an own entity always beats an overlapping foreign one.
export const OWN_HIT_BONUS = 1e6;
// Fallbacks when an entity kind has no STATS entry (defensive; shouldn't happen).
export const DEFAULT_HIT_RADIUS = 10;
export const DEFAULT_TILE_SIZE = 32;
// Wheel zoom multiplier per notch.
export const ZOOM_STEP = 0.12;
