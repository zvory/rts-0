const PART_SELECTION_KEYS = new WeakMap();

export function normalizedPartSet(parts) {
  if (parts == null) return null;
  if (parts instanceof Set) return parts;
  if (typeof parts === "string") return new Set([parts]);
  if (typeof parts?.[Symbol.iterator] !== "function") return null;
  return new Set(parts);
}

export function partSelectionKey(parts) {
  const cacheable = isImmutablePartSelection(parts);
  if (cacheable) {
    const cached = PART_SELECTION_KEYS.get(parts);
    if (cached !== undefined) return cached;
  }
  const selected = normalizedPartSet(parts);
  const key = selected ? [...selected].sort().join("\u0000") : null;
  if (cacheable) PART_SELECTION_KEYS.set(parts, key);
  return key;
}

export function isImmutablePartSelection(parts) {
  return Array.isArray(parts) && Object.isFrozen(parts);
}
