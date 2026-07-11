const GRID_SNAPSHOT_VERSION = 1;

export class GridSnapshotCache {
  constructor() {
    this._snapshot = null;
  }

  snapshot({ revision, width, height, source }) {
    const shape = normalizeShape(width, height);
    const normalizedRevision = normalizeRevision(revision);
    if (
      this._snapshot &&
      this._snapshot.revision === normalizedRevision &&
      this._snapshot.width === shape.width &&
      this._snapshot.height === shape.height
    ) {
      return this._snapshot;
    }
    this._snapshot = createGridSnapshot({
      revision: normalizedRevision,
      width: shape.width,
      height: shape.height,
      source,
    });
    return this._snapshot;
  }

  clear() {
    this._snapshot = null;
  }
}

export function createGridSnapshot({ revision, width, height, source }) {
  const shape = normalizeShape(width, height);
  const normalizedRevision = normalizeRevision(revision);
  const count = shape.width * shape.height;
  if (!source || typeof source.length !== "number" || source.length < count) {
    throw new RangeError(`Grid source must contain at least ${count} values.`);
  }
  const values = new Uint8Array(count);
  for (let index = 0; index < count; index += 1) {
    const value = Number(source[index]);
    if (!Number.isFinite(value)) throw new TypeError(`Grid value ${index} must be finite.`);
    values[index] = Math.max(0, Math.min(255, Math.trunc(value)));
  }

  return Object.freeze({
    version: GRID_SNAPSHOT_VERSION,
    revision: normalizedRevision,
    width: shape.width,
    height: shape.height,
    get(index) {
      return Number.isInteger(index) && index >= 0 && index < count ? values[index] : undefined;
    },
    copyInto(targetTypedArray, targetOffset = 0) {
      if (!isTypedArray(targetTypedArray)) {
        throw new TypeError("GridSnapshot.copyInto requires a typed array target.");
      }
      if (!Number.isInteger(targetOffset) || targetOffset < 0) {
        throw new RangeError("GridSnapshot target offset must be a non-negative integer.");
      }
      if (targetOffset + count > targetTypedArray.length) {
        throw new RangeError("GridSnapshot target does not have enough capacity.");
      }
      targetTypedArray.set(values, targetOffset);
      return count;
    },
  });
}

function normalizeShape(width, height) {
  const normalizedWidth = Number(width);
  const normalizedHeight = Number(height);
  if (!Number.isInteger(normalizedWidth) || normalizedWidth < 0) {
    throw new RangeError("Grid width must be a non-negative integer.");
  }
  if (!Number.isInteger(normalizedHeight) || normalizedHeight < 0) {
    throw new RangeError("Grid height must be a non-negative integer.");
  }
  return { width: normalizedWidth, height: normalizedHeight };
}

function normalizeRevision(revision) {
  const normalized = Number(revision);
  if (!Number.isInteger(normalized) || normalized < 0) {
    throw new RangeError("Grid revision must be a non-negative integer.");
  }
  return normalized;
}

function isTypedArray(value) {
  return ArrayBuffer.isView(value) && !(value instanceof DataView) && typeof value.set === "function";
}
