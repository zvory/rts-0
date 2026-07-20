class FakeGraphics {
  constructor() {
    this.position = { set: (x = 0, y = 0) => { this.x = x; this.y = y; } };
    this.scale = { set: (x = 1, y = x) => { this.scaleX = x; this.scaleY = y; } };
  }
  clear() { return this; }
  stroke() { return this; }
  fill() { return this; }
  cut() { this.calls?.push?.(["cut"]); return this; }
  beginHole() {}
  endHole() {}
  endFill() {}
  poly() { return this; }
  circle() { return this; }
  ellipse() { return this; }
  rect() { return this; }
  roundRect() { return this; }
  moveTo() { return this; }
  lineTo() { return this; }
  closePath() { return this; }
  arc() {}
}

export class RecordingGraphics extends FakeGraphics {
  constructor() {
    super();
    this.calls = [];
  }
  stroke(style) {
    this.calls.push(["lineStyle", style?.width, style?.color, style?.alpha]);
    return this;
  }
  moveTo(x, y) {
    this.calls.push(["moveTo", x, y]);
    return this;
  }
  lineTo(x, y) {
    this.calls.push(["lineTo", x, y]);
    return this;
  }
  closePath() {
    this.calls.push(["closePath"]);
    return this;
  }
  fill(style) {
    this.calls.push(["beginFill", style?.color, style?.alpha]);
    return this;
  }
  beginHole() {
    this.calls.push(["beginHole"]);
  }
  endHole() {
    this.calls.push(["endHole"]);
  }
  clear() {
    this.calls.push(["clear"]);
    return this;
  }
  circle(x, y, radius) {
    this.calls.push(["drawCircle", x, y, radius]);
    return this;
  }
  ellipse(x, y, rx, ry) {
    this.calls.push(["drawEllipse", x, y, rx, ry]);
    return this;
  }
  arc(x, y, radius, start, end, anticlockwise) {
    this.calls.push(["arc", x, y, radius, start, end, anticlockwise]);
  }
  rect(x, y, width, height) {
    this.calls.push(["drawRect", x, y, width, height]);
    return this;
  }
  poly(points) {
    this.calls.push(["drawPolygon", points]);
    return this;
  }
  roundRect(x, y, width, height, radius) {
    this.calls.push(["drawRoundedRect", x, y, width, height, radius]);
    return this;
  }
}

export function installFakePixi() {
  const priorPixi = globalThis.PIXI;
  const priorWindow = globalThis.window;
  const priorOffscreenCanvas = globalThis.OffscreenCanvas;

  class FakeContainer {
    constructor() {
      this.children = [];
      this.position = { set: (x = 0, y = 0) => { this.x = x; this.y = y; } };
      this.scale = { set: (x = 1, y = x) => { this.scaleX = x; this.scaleY = y; } };
      this.visible = true;
    }
    addChild(...children) {
      for (const child of children) {
        this.children.push(child);
        child.parent = this;
      }
      return children[0];
    }
    removeChild(child) {
      this.children = this.children.filter((item) => item !== child);
      child.parent = null;
    }
    removeChildren() {
      const removed = this.children;
      for (const child of removed) child.parent = null;
      this.children = [];
      return removed;
    }
    destroy() {}
  }

  class PixiGraphics extends RecordingGraphics {
    constructor() {
      super();
      this.visible = true;
      this.alpha = 1;
    }
    destroy() {
      this.destroyed = true;
    }
  }

  class FakeText {
    constructor(options = {}) {
      this.text = options.text || "";
      this.style = options.style || {};
      this.visible = true;
      this.alpha = 1;
      this.position = { set: (x = 0, y = 0) => { this.x = x; this.y = y; } };
      this.scale = { set: (x = 1, y = x) => { this.scaleX = x; this.scaleY = y; } };
      this.anchor = { set: (x = 0, y = x) => { this.anchorX = x; this.anchorY = y; } };
    }
    destroy() {
      this.destroyed = true;
    }
  }

  class FakeApplication {
    constructor(options = {}) {
      this.options = options;
      this.renderCalls = 0;
      this.ticker = {
        started: false,
        startCalls: 0,
        stopCalls: 0,
        start: () => {
          this.ticker.started = true;
          this.ticker.startCalls += 1;
        },
        stop: () => {
          this.ticker.started = false;
          this.ticker.stopCalls += 1;
        },
      };
      this.stage = new FakeContainer();
      this.canvas = { style: {}, parentNode: null };
      this.renderer = {
        roundPixels: false,
        resize: (w, h) => {
          this.width = w;
          this.height = h;
        },
      };
    }
    async init(options = {}) {
      this.options = options;
    }
    render() {
      this.renderCalls += 1;
    }
    destroy() {
      this.destroyed = true;
    }
  }

  class FakeTexture {
    constructor(resource, options = {}) {
      this.resource = resource;
      this.options = options;
      this.updateCount = 0;
      this.destroyed = false;
      this.source = {
        update: () => {
          this.updateCount += 1;
        },
        destroy: () => {
          this.textureSourceDestroyed = true;
        },
      };
    }
    update() {
      this.updateCount += 1;
    }
    destroy(options) {
      this.destroyed = true;
      this.destroyOptions = options;
    }
    static from(resource, options = {}) {
      return new FakeTexture(resource, options);
    }
  }

  class FakeRectangle {
    constructor(x = 0, y = 0, width = 0, height = 0) {
      this.x = x;
      this.y = y;
      this.width = width;
      this.height = height;
    }
  }

  class FakeSprite {
    constructor(texture) {
      this.texture = texture;
      this.visible = true;
      this.scale = { set: (x = 1, y = x) => { this.scaleX = x; this.scaleY = y; } };
      this.position = { set: (x = 0, y = 0) => { this.x = x; this.y = y; } };
      this.anchor = { set: (x = 0, y = x) => { this.anchorX = x; this.anchorY = y; } };
    }
    destroy(options) {
      this.destroyed = true;
      this.destroyOptions = options;
      if (options === true || options?.texture) {
        this.texture?.destroy?.(options);
      }
    }
  }

  globalThis.window = {
    ...(priorWindow || {}),
    devicePixelRatio: 1,
    innerWidth: 800,
    innerHeight: 600,
  };
  globalThis.PIXI = {
    Application: FakeApplication,
    Container: FakeContainer,
    Graphics: PixiGraphics,
    Text: FakeText,
    Texture: FakeTexture,
    Assets: { load: async (resource) => FakeTexture.from(resource) },
    Rectangle: FakeRectangle,
    Sprite: FakeSprite,
    SCALE_MODES: { NEAREST: "nearest" },
    TextureStyle: { defaultOptions: {} },
  };
  globalThis.OffscreenCanvas = class FakeOffscreenCanvas {
    constructor(width, height) {
      this.width = width;
      this.height = height;
      this.context = fakeCanvasContext(this);
    }
    getContext() { return this.context; }
  };

  return () => {
    if (priorPixi === undefined) delete globalThis.PIXI;
    else globalThis.PIXI = priorPixi;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
    if (priorOffscreenCanvas === undefined) delete globalThis.OffscreenCanvas;
    else globalThis.OffscreenCanvas = priorOffscreenCanvas;
  };
}

function fakeCanvasContext(canvas) {
  return {
    canvas,
    imageSmoothingEnabled: true,
    fillStyle: "",
    strokeStyle: "",
    globalAlpha: 1,
    globalCompositeOperation: "source-over",
    clearRect() {},
    fillRect() {},
    strokeRect() {},
    drawImage() {},
    putImageData() {},
    getImageData: (_x, _y, width = canvas.width, height = canvas.height) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    save() {},
    restore() {},
    translate() {},
    rotate() {},
    scale() {},
    beginPath() {},
    closePath() {},
    moveTo() {},
    lineTo() {},
    arc() {},
    ellipse() {},
    fill() {},
    stroke() {},
  };
}
