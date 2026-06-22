class FakeGraphics {
  constructor() {
    this.position = { set() {} };
  }
  clear() {}
  lineStyle() {}
  beginFill() {}
  endFill() {}
  drawPolygon() {}
  drawCircle() {}
  drawRect() {}
  drawRoundedRect() {}
  moveTo() {}
  lineTo() {}
  arc() {}
}

export class RecordingGraphics extends FakeGraphics {
  constructor() {
    super();
    this.calls = [];
  }
  lineStyle(width, color, alpha) {
    this.calls.push(["lineStyle", width, color, alpha]);
  }
  moveTo(x, y) {
    this.calls.push(["moveTo", x, y]);
  }
  lineTo(x, y) {
    this.calls.push(["lineTo", x, y]);
  }
  beginFill(color, alpha) {
    this.calls.push(["beginFill", color, alpha]);
  }
  clear() {
    this.calls.push(["clear"]);
  }
  drawCircle(x, y, radius) {
    this.calls.push(["drawCircle", x, y, radius]);
  }
  arc(x, y, radius, start, end, anticlockwise) {
    this.calls.push(["arc", x, y, radius, start, end, anticlockwise]);
  }
  drawRect(x, y, width, height) {
    this.calls.push(["drawRect", x, y, width, height]);
  }
  drawPolygon(points) {
    this.calls.push(["drawPolygon", points]);
  }
  drawRoundedRect(x, y, width, height, radius) {
    this.calls.push(["drawRoundedRect", x, y, width, height, radius]);
  }
}

export function installFakePixi() {
  const priorPixi = globalThis.PIXI;
  const priorWindow = globalThis.window;

  class FakeContainer {
    constructor() {
      this.children = [];
      this.position = { set: (x = 0, y = 0) => { this.x = x; this.y = y; } };
      this.scale = { set: (value = 1) => { this.scaleValue = value; } };
      this.visible = true;
    }
    addChild(child) {
      this.children.push(child);
      child.parent = this;
      return child;
    }
    removeChild(child) {
      this.children = this.children.filter((item) => item !== child);
      child.parent = null;
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

  class FakeApplication {
    constructor(options = {}) {
      this.options = options;
      this.stage = new FakeContainer();
      this.view = { style: {}, parentNode: null };
      this.renderer = {
        roundPixels: false,
        resize: (w, h) => {
          this.width = w;
          this.height = h;
        },
      };
    }
    destroy() {
      this.destroyed = true;
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
    SCALE_MODES: { NEAREST: "nearest" },
    settings: {},
  };

  return () => {
    if (priorPixi === undefined) delete globalThis.PIXI;
    else globalThis.PIXI = priorPixi;
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  };
}
