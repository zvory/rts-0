const IDENTITY = [1, 0, 0, 1, 0, 0];

// Narrow Canvas-shaped scene recorder. Cached raster layers keep their original authoring;
// dynamic shapes and unit instances become ordered GPU commands, never main-thread image draws.
export class MinimapGpuContext {
  constructor(assets, unit) {
    this.assets = assets;
    this.unit = unit;
    this.reset();
  }

  reset() {
    this.commands = [];
    this.matrix = [...IDENTITY];
    this.stack = [];
    this.path = [];
    this.fillStyle = "#000000";
    this.strokeStyle = "#000000";
    this.lineWidth = 1;
    this.globalAlpha = 1;
  }

  save() {
    this.stack.push({ matrix: [...this.matrix], fillStyle: this.fillStyle,
      strokeStyle: this.strokeStyle, lineWidth: this.lineWidth, globalAlpha: this.globalAlpha });
  }
  restore() { const value = this.stack.pop(); if (value) Object.assign(this, value); }
  translate(x, y) {
    const m = this.matrix;
    m[4] += m[0] * x + m[2] * y; m[5] += m[1] * x + m[3] * y;
  }
  rotate(angle) {
    const [a, b, c, d] = this.matrix, cos = Math.cos(angle), sin = Math.sin(angle);
    this.matrix[0] = a * cos + c * sin; this.matrix[1] = b * cos + d * sin;
    this.matrix[2] = c * cos - a * sin; this.matrix[3] = d * cos - b * sin;
  }
  scale(x, y) {
    this.matrix[0] *= x; this.matrix[1] *= x; this.matrix[2] *= y; this.matrix[3] *= y;
  }
  beginPath() { this.path = []; }
  moveTo(x, y) { this.path.push(["moveTo", x, y]); }
  lineTo(x, y) { this.path.push(["lineTo", x, y]); }
  closePath() { this.path.push(["closePath"]); }
  arc(x, y, radius, start, end) { this.path.push(["arc", x, y, radius, start, end]); }
  fill() { this._shape(this.path, true); }
  stroke() { this._shape(this.path, false); }
  fillRect(x, y, width, height) { this._shape([["rect", x, y, width, height]], true); }
  strokeRect(x, y, width, height) { this._shape([["rect", x, y, width, height]], false); }
  _shape(path, fill) {
    this.commands.push({ type: "shape", path: path.slice(), matrix: [...this.matrix],
      color: fill ? this.fillStyle : this.strokeStyle, fill, width: this.lineWidth, alpha: this.globalAlpha });
  }
  drawImage(image, ...args) {
    const id = this.assets.image(image);
    let rect;
    if (args.length === 2) rect = [args[0], args[1], image.width, image.height];
    else if (args.length === 4) rect = args;
    else throw new Error("Unsupported minimap image crop");
    this.commands.push({ type: "image", image: id, rect, matrix: [...this.matrix], alpha: this.globalAlpha });
  }
}
