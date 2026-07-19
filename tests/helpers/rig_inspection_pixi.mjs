export function createInspectionPixiFactory() {
  return {
    createContainer: () => new FakeContainer(),
    createGraphics: () => new FakeGraphics(),
  };
}

export function createInspectionPngPixiFactory() {
  return {
    ...createInspectionPixiFactory(),
    createRectangle: (x, y, width, height) => ({ x, y, width, height, w: width, h: height }),
    createTexture: (baseTexture, rectangle) => ({ baseTexture, frame: rectangle }),
    createSprite: (texture) => new FakeSprite(texture),
  };
}

export class FakeContainer {
  constructor() {
    this.children = [];
    this.position = makePointSetter(this, "x", "y");
    this.scale = makePointSetter(this, "scaleX", "scaleY");
    this.pivot = makePointSetter(this, "pivotX", "pivotY");
    this.x = 0;
    this.y = 0;
    this.scaleX = 1;
    this.scaleY = 1;
    this.visible = true;
    this.alpha = 1;
    this.rotation = 0;
    this.destroyed = false;
  }

  addChild(child) {
    child.parent = this;
    this.children.push(child);
  }

  removeChild(child) {
    child.parent = null;
    this.children = this.children.filter((candidate) => candidate !== child);
  }

  destroy() {
    this.destroyed = true;
  }
}

class FakeSprite extends FakeContainer {
  constructor(texture) {
    super();
    this.texture = texture;
    this.tint = 0xffffff;
    this.anchorX = 0;
    this.anchorY = 0;
    this.anchor = makePointSetter(this, "anchorX", "anchorY");
    this.destroyOptions = null;
  }

  destroy(options = null) {
    this.destroyed = true;
    this.destroyOptions = options;
  }
}

export class FakeGraphics extends FakeContainer {
  constructor() {
    super();
    this.commands = [];
    this.lineWidth = 0;
    this.clearCount = 0;
  }

  clear() {
    this.clearCount += 1;
    this.commands = [];
    this.lineWidth = 0;
  }

  beginFill(color, alpha = 1) {
    this.commands.push({ op: "beginFill", color, alpha });
  }

  endFill() {
    this.commands.push({ op: "endFill" });
  }

  lineStyle(width = 0, color = 0, alpha = 1) {
    this.lineWidth = width;
    this.commands.push({ op: "lineStyle", width, color, alpha });
  }

  moveTo(x, y) {
    this.commands.push({ op: "moveTo", x, y });
  }

  lineTo(x, y) {
    this.commands.push({ op: "lineTo", x, y });
  }

  drawPolygon(points) {
    this.commands.push({ op: "drawPolygon", points });
  }

  drawCircle(x, y, radius) {
    this.commands.push({ op: "drawCircle", x, y, radius });
  }

  drawEllipse(x, y, rx, ry) {
    this.commands.push({ op: "drawEllipse", x, y, rx, ry });
  }

  drawRect(x, y, width, height) {
    this.commands.push({ op: "drawRect", x, y, width, height });
  }
}

function makePointSetter(target, xKey, yKey) {
  return {
    set(x, y = x) {
      target[xKey] = x;
      target[yKey] = y;
    },
  };
}
