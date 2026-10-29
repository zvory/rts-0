import { MinimapGpuFog } from "./minimap_gpu_fog.js";
import { unitPngSize } from "./minimap_unit_icons.js";
import { MinimapGpuAssets } from "./minimap_gpu_assets.js";
import { MinimapGpuContext } from "./minimap_gpu_context.js";

const ENTITY_FIELDS = ["id", "kind", "owner", "state", "facing", "weaponFacing", "setupFacing",
  "setupState", "panzerfaustLoaded", "panzerfaustWindupProgress"];

export class MinimapGpuBridge {
  constructor(canvas, worldCanvas) {
    this.canvas = canvas;
    this.worldCanvas = worldCanvas;
    this.assets = new MinimapGpuAssets();
    this.bundles = new Map();
    this.pending = new Set();
    this.errors = new Map();
    this.context = new MinimapGpuContext(this.assets, (...args) => this.unit(...args));
    this.fog = new MinimapGpuFog(this.assets, () => this.createCanvas());
    this.context.fog = (...args) => this.fog.draw(this.context, ...args);
    this.active = false;
    this.frame = null;
    this.chrome = null;
  }

  createCanvas() { return this.assets.track(this.canvas.ownerDocument.createElement("canvas")); }

  begin(minimap, capture) {
    this.active = !capture;
    if (capture) { this.captured = true; return; }
    if (this.captured) { minimap.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height); this.captured = false; }
    if (this.lastSize !== minimap.size) {
      this.lastSize = minimap.size;
      this.epoch = (this.epoch || 0) + 1;
      this.bundles.clear(); this.pending.clear(); this.errors.clear(); this.assets.records.clear();
    }
    this.context.reset();
    this.minimap = minimap;
    const rect = this.canvas.getBoundingClientRect(), world = this.worldCanvas.getBoundingClientRect();
    this.bounds = { x: rect.left + this.canvas.clientLeft - world.left,
      y: rect.top + this.canvas.clientTop - world.top,
      width: this.canvas.clientWidth, height: this.canvas.clientHeight };
    this.hidden = this.canvas.offsetParent === null;
    this._chrome(rect);
    this.originalContext = minimap.ctx;
    minimap.ctx = this.context;
  }

  end(minimap, visualTimeMs) {
    if (!this.active) return;
    minimap.ctx = this.originalContext;
    this.frame = Object.freeze({ version: 1, size: minimap.size, bounds: this.bounds,
      hidden: this.hidden, visualTimeMs, commands: this.context.commands,
      images: [...this.assets.records.values()], bundles: [...this.bundles.values()] });
    this.active = false;
  }

  unit(entity, color, point, size, flash, icons) {
    const key = `${entity.kind}:${color}:${size}:${flash ? 1 : 0}`;
    let bundle = this.bundles.get(key);
    const parts = icons._parts(entity.kind);
    if (!bundle && !this.pending.has(key) && !this.errors.has(key)) {
      this.pending.add(key);
      const epoch = this.epoch;
      const build = () => {
        if (this.destroyed || epoch !== this.epoch) return;
        for (const variant of [false, true]) {
          const variantKey = `${entity.kind}:${color}:${size}:${variant ? 1 : 0}`;
          const images = {};
          for (const part of parts) {
            const entry = icons.request(entity.kind, color, part);
            const sprite = icons._sprite(entry, entity.kind, size, variant);
            images[part || "portrait"] = this.assets.image(sprite, { atlas: true });
          }
          const value = { id: variantKey, parts: images };
          this.bundles.set(variantKey, value);
          if (variant === flash) bundle = value;
        }
        this.pending.delete(key);
      };
      if (parts.every(part => icons.request(entity.kind, color, part).image)) build();
      else icons.prepare([{ kind: entity.kind, color }]).then(build)
        .catch(error => this.errors.set(key, error)).finally(() => this.pending.delete(key));
    }
    if (!bundle) return;
    const view = {};
    for (const field of ENTITY_FIELDS) if (entity[field] !== undefined) view[field] = entity[field];
    if (entity.abilities) view.abilities = entity.abilities.map(({ ability, cooldownLeft }) => ({ ability, cooldownLeft }));
    const state = this.minimap.state, now = state.visualNow?.() ?? performance.now();
    this.context.commands.push({ type: "unit", entity: view, bundle: key, x: point.x, y: point.y,
      visualSize: unitPngSize(entity.kind, size),
      recoil: state.weaponRecoil?.(entity.id, entity.kind, now) || 0,
      recoilPhase: state.weaponRecoilPhase?.(entity.id, entity.kind, now) || 0,
      recoilKind: state.weaponRecoilKind?.(entity.id) || null });
  }

  buildingOutlineChanged(entities, size) {
    const signature = `${size}|${entities.map(e => `${e.id}:${e.x}:${e.y}:${e.kind}`).join("|")}`;
    const changed = signature !== this.buildingSignature;
    this.buildingSignature = signature;
    return changed;
  }

  readiness() {
    return { ready: !this.pending.size && !this.errors.size,
      pendingAssets: [...this.pending].map(id => ({ id, status: "pending" })),
      failedAssets: [...this.errors].map(([id, error]) => ({ id, status: "failed", message: error.message })) };
  }

  _chrome(rect) {
    const parent = this.canvas.parentElement;
    if (!parent) return;
    if (!this.chrome) {
      this.chrome = { parent, parentStyle: parent.getAttribute("style"), canvasStyle: this.canvas.getAttribute("style") };
    }
    const panel = parent.getBoundingClientRect();
    const top = rect.top - panel.top, bottom = panel.bottom - rect.bottom;
    const left = rect.left - panel.left, right = panel.right - rect.right;
    const signature = `${top}:${bottom}:${left}:${right}:${panel.width}:${panel.height}`;
    if (signature === this.chromeSignature) return;
    this.chromeSignature = signature;
    parent.style.backgroundColor = "transparent";
    parent.style.backgroundImage = "linear-gradient(#27251ff0,#27251ff0),".repeat(3) + "linear-gradient(#27251ff0,#27251ff0)";
    parent.style.backgroundRepeat = "no-repeat";
    parent.style.backgroundSize = `100% ${top}px,100% ${bottom}px,${left}px ${rect.height}px,${right}px ${rect.height}px`;
    parent.style.backgroundPosition = `top,bottom,left 0 top ${top}px,right 0 top ${top}px`;
    this.canvas.style.background = "transparent";
  }

  destroy() {
    this.destroyed = true;
    if (this.chrome) {
      for (const [node, value] of [[this.chrome.parent, this.chrome.parentStyle], [this.canvas, this.chrome.canvasStyle]]) {
        if (value === null) node.removeAttribute("style"); else node.setAttribute("style", value);
      }
    }
    this.assets.destroy(); this.bundles.clear(); this.pending.clear(); this.errors.clear();
    this.frame = null;
  }
}
