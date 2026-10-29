// Revisioned, source-detached pixel snapshots. Only changed cache canvases cross the worker wire.
export class MinimapGpuAssets {
  constructor() {
    this.sources = new WeakMap();
    this.records = new Map();
    this.nextId = 1;
  }

  track(canvas) {
    if (this.sources.has(canvas)) return canvas;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    const source = { id: this.nextId++, revision: 0, context, snapshot: null };
    this.sources.set(canvas, source);
    for (const name of ["clearRect", "fillRect", "strokeRect", "drawImage", "fill", "stroke", "putImageData"]) {
      const original = context[name].bind(context);
      context[name] = (...args) => { source.revision++; return original(...args); };
    }
    return canvas;
  }

  image(image, { atlas = false, smooth = true } = {}) {
    let source = this.sources.get(image);
    if (!source) {
      if (image.getContext) {
        this.track(image);
        source = this.sources.get(image);
      } else {
        const canvas = document.createElement("canvas");
        canvas.width = image.width; canvas.height = image.height;
        canvas.getContext("2d").drawImage(image, 0, 0);
        this.track(canvas);
        source = this.sources.get(canvas);
        this.sources.set(image, source);
        source.canvas = canvas;
      }
    }
    const canvas = source.canvas || image;
    if (!source.snapshot || source.snapshot.revision !== source.revision
      || source.snapshot.fullWidth !== canvas.width || source.snapshot.fullHeight !== canvas.height) {
      const pixels = source.context.getImageData(0, 0, canvas.width, canvas.height);
      const trimmed = atlas ? trimImage(pixels) : { width: pixels.width, height: pixels.height,
        offsetX: 0, offsetY: 0, pixels: new Uint8Array(pixels.data.buffer) };
      source.snapshot = Object.freeze({ id: source.id, revision: source.revision, atlas, smooth,
        fullWidth: canvas.width, fullHeight: canvas.height, ...trimmed });
      this.records.set(source.id, source.snapshot);
    }
    this.records.set(source.id, source.snapshot);
    return source.id;
  }

  destroy() { this.records.clear(); this.sources = new WeakMap(); }
}

function trimImage(image) {
  const { width, height, data } = image;
  let left = width, top = height, right = -1, bottom = -1;
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    if (!data[(y * width + x) * 4 + 3]) continue;
    left = Math.min(left, x); right = Math.max(right, x);
    top = Math.min(top, y); bottom = Math.max(bottom, y);
  }
  if (right < left) return { width: 1, height: 1, offsetX: 0, offsetY: 0, pixels: new Uint8Array(4) };
  left = Math.max(0, left - 2); top = Math.max(0, top - 2);
  right = Math.min(width - 1, right + 2); bottom = Math.min(height - 1, bottom + 2);
  const croppedWidth = right - left + 1, croppedHeight = bottom - top + 1;
  const pixels = new Uint8Array(croppedWidth * croppedHeight * 4);
  for (let y = 0; y < croppedHeight; y++) {
    pixels.set(data.subarray(((top + y) * width + left) * 4,
      ((top + y) * width + left + croppedWidth) * 4), y * croppedWidth * 4);
  }
  return { width: croppedWidth, height: croppedHeight, offsetX: left, offsetY: top, pixels };
}
