import { SWEEP_EVICT_FRAMES } from "./palette.js";

// Layer names in back-to-front draw order. Index in this array == child index in `world`.
export const LAYERS = [
  "terrain",
  "resources",
  "buildingShadows",
  "buildings",
  "unitShadows",
  "units",
  "smokes",
  "selectionRings",
  "hpBars",
  "fog",
  "shotRevealShadows",
  "shotReveals",
  "feedback",
  "placement",
];

export function _sweep() {
  // Tally which ids were touched in any pool this frame, then bump/reset the
  // shared per-id unseen counter so an id alive in one layer isn't evicted
  // from another (e.g. a building's footprint + its icon).
  const seenAny = new Set();
  for (const key of Object.keys(this._seen)) {
    for (const id of this._seen[key]) seenAny.add(id);
  }

  const evict = new Set();
  const ids = new Set([...this._unseen.keys()]);
  for (const key of Object.keys(this._pools)) {
    for (const id of this._pools[key].keys()) ids.add(id);
  }
  if (this._iconPool) for (const id of this._iconPool.keys()) ids.add(id);
  if (this._queueLabelPool) for (const id of this._queueLabelPool.keys()) ids.add(id);
  if (this._rigComparisonPool) for (const id of this._rigComparisonPool.keys()) ids.add(id);
  for (const id of ids) {
    if (seenAny.has(id)) {
      this._unseen.delete(id);
    } else {
      const n = (this._unseen.get(id) || 0) + 1;
      if (n >= SWEEP_EVICT_FRAMES) evict.add(id);
      else this._unseen.set(id, n);
    }
  }

  for (const key of Object.keys(this._pools)) {
    const pool = this._pools[key];
    const seen = this._seen[key];
    for (const [id, g] of pool) {
      if (seen.has(id)) continue;
      if (evict.has(id)) {
        this.layers[key].removeChild(g);
        g.destroy();
        pool.delete(id);
      } else {
        g.visible = false;
      }
    }
  }
  if (this._iconPool) {
    const seen = this._seen.buildings;
    for (const [id, t] of this._iconPool) {
      if (seen.has(id)) continue;
      if (evict.has(id)) {
        this.layers.buildings.removeChild(t);
        t.destroy();
        this._iconPool.delete(id);
      } else {
        t.visible = false;
      }
    }
  }
  if (this._queueLabelPool) {
    const seen = this._seen.buildings;
    for (const [id, t] of this._queueLabelPool) {
      if (seen.has(id)) continue;
      if (evict.has(id)) {
        this.layers.buildings.removeChild(t);
        t.destroy();
        this._queueLabelPool.delete(id);
      } else {
        t.visible = false;
      }
    }
  }
  if (this._rigComparisonPool) {
    const seen = this._seen.rigComparisons || new Set();
    for (const [id, instance] of this._rigComparisonPool) {
      if (seen.has(id)) continue;
      if (evict.has(id)) {
        this.layers.rigComparisons?.removeChild?.(instance.container);
        instance.destroy();
        this._rigComparisonPool.delete(id);
      } else {
        instance.container.visible = false;
      }
    }
  }
  for (const id of evict) this._unseen.delete(id);
}
