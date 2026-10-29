import { createMinimapAnimation } from "./rigs/minimap_animation.js";
import { MinimapGpuTextures } from "./minimap_gpu_textures.js";

export class MinimapGpuLayer {
  constructor(renderer) {
    this.renderer = renderer;
    this.pixi = globalThis.PIXI;
    this.content = new this.pixi.Container();
    this.root = new this.pixi.Sprite();
    this.mask = new this.pixi.Graphics();
    this.root.mask = this.mask;
    renderer.app.stage.addChild(this.root, this.mask);
    this.textures = new MinimapGpuTextures(this.pixi);
    this.animation = createMinimapAnimation({ simpleTank: true });
    this.pool = [];
  }

  update(frame) {
    this.root.visible = !!frame && !frame.hidden;
    this.mask.visible = this.root.visible;
    if (!frame || frame.hidden) return;
    const bounds = frame.bounds;
    this.root.position.set(bounds.x, bounds.y);
    this.root.scale.set(bounds.width / frame.size, bounds.height / frame.size);
    const maskKey = `${bounds.x}:${bounds.y}:${bounds.width}:${bounds.height}`;
    if (maskKey !== this.maskKey) {
      this.mask.clear().roundRect(bounds.x, bounds.y, bounds.width, bounds.height, 4).fill(0xffffff);
      this.maskKey = maskKey;
    }
    if (!this.target || this.target.width !== frame.size) {
      this.target?.destroy(true);
      this.target = this.pixi.RenderTexture.create({ width: frame.size, height: frame.size, resolution: 1, antialias: true });
      this.target.source.scaleMode = "linear";
      this.root.texture = this.target;
    }
    const units = frame.commands.filter(command => command.type === "unit");
    const recoil = new Map(units.map(command => [command.entity.id, command]));
    this.animation.beginFrame(units.map(command => command.entity), {
      weaponRecoil: id => recoil.get(id)?.recoil || 0,
      weaponRecoilPhase: id => recoil.get(id)?.recoilPhase || 0,
      weaponRecoilKind: id => recoil.get(id)?.recoilKind || undefined,
    }, frame.visualTimeMs);
    const bundles = new Map(frame.bundles.map(bundle => [bundle.id, bundle.parts]));
    this.used = 0;
    this.particleRun = null;
    for (const command of frame.commands) {
      if (command.type === "unit") this._unit(command, bundles, frame.size);
      else if (command.type === "image") {
        this._finishRun();
        const image = this.textures.get(command.image);
        const sprite = this._object("sprite");
        sprite.texture = image.texture;
        this._matrix(sprite, command.matrix);
        const [x, y, width, height] = command.rect;
        sprite.pivot.set(-x * image.width / width, -y * image.height / height);
        sprite.scale.x *= width / image.width; sprite.scale.y *= height / image.height;
        sprite.alpha = command.alpha;
      } else if (command.type === "shape") { this._finishRun(); this._shape(command); }
      else throw new Error(`Unknown minimap command ${command.type}`);
    }
    this._finishRun();
    for (let index = this.used; index < this.pool.length; index++) this.pool[index].object.visible = false;
    this.renderer.app.renderer.render({ container: this.content, target: this.target, clear: true });
  }

  _object(type) {
    const index = this.used++;
    let record = this.pool[index];
    if (record?.type !== type) {
      record?.object.destroy();
      const object = type === "sprite" ? new this.pixi.Sprite()
        : type === "particles" ? new this.pixi.ParticleContainer({
          dynamicProperties: { position: true, rotation: true, vertex: true, uvs: true, color: true },
          boundsArea: new this.pixi.Rectangle(0, 0, this.textures.size, this.textures.size),
        }) : new this.pixi.Graphics();
      record = { type, object };
      this.pool[index] = record;
      this.content.addChildAt(object, Math.min(index, this.content.children.length));
    }
    const object = record.object;
    object.visible = true;
    object.alpha = 1;
    if (type === "sprite") { object.pivot.set(0); object.scale.set(1); object.rotation = 0; }
    return object;
  }

  _matrix(object, values) { object.setFromMatrix(new this.pixi.Matrix(...values)); }

  _shape(command) {
    const object = this._object("shape");
    const signature = JSON.stringify([command.path, command.fill, command.color, command.width]);
    if (object.rtsMinimapShape !== signature) {
      object.clear();
      for (const [method, ...args] of command.path) object[method](...args);
      const color = new this.pixi.Color(command.color).toRgba();
      const style = { color: command.color, alpha: color.a, width: command.width };
      if (command.fill) object.fill(style); else object.stroke(style);
      object.rtsMinimapShape = signature;
    }
    this._matrix(object, command.matrix);
    object.alpha = command.alpha;
  }

  _unit(command, bundles, size) {
    const { entity } = command;
    const bundle = bundles.get(command.bundle);
    if (!bundle) throw new Error(`Missing minimap icon bundle ${command.bundle}`);
    const layers = this.animation.layers(entity) || [{ part: null,
      rotation: (Number.isFinite(entity.facing) ? entity.facing : 0), x: 0, y: 0, scaleX: 1, scaleY: 1, alpha: 1 }];
    for (const layer of layers) {
      const image = this.textures.get(bundle[layer.part || "portrait"]);
      if (!this.particleRun || this.particleRun.texture.source !== image.texture.source) {
        this._finishRun();
        const container = this._object("particles");
        container.texture = image.texture;
        container.rtsParticlePool ||= [];
        container.rtsCount = 0;
        this.particleRun = container;
      }
      const container = this.particleRun;
      const index = container.rtsCount++;
      let particle = container.rtsParticlePool[index];
      if (!particle) {
        particle = new this.pixi.Particle({ texture: image.texture });
        container.rtsParticlePool[index] = particle;
      }
      container.particleChildren[index] = particle;
      particle.texture = image.texture;
      particle.anchorX = (image.fullWidth / 2 - image.offsetX) / image.width;
      particle.anchorY = (image.fullHeight / 2 - image.offsetY) / image.height;
      particle.x = command.x + layer.x * command.visualSize;
      particle.y = command.y + layer.y * command.visualSize;
      particle.rotation = layer.rotation;
      particle.scaleX = layer.scaleX / 4;
      particle.scaleY = layer.scaleY / 4;
      particle.alpha = layer.alpha;
    }
  }

  _finishRun() {
    const container = this.particleRun;
    if (!container) return;
    container.particleChildren.length = container.rtsCount;
    if (container.rtsPreviousCount !== container.rtsCount) {
      container.update(); container.rtsPreviousCount = container.rtsCount;
    }
    this.particleRun = null;
  }

  destroy() {
    this.content.destroy({ children: true }); this.root.destroy(); this.mask.destroy();
    this.target?.destroy(true);
    this.textures.clear(); this.animation.destroy(); this.pool.length = 0;
  }
}
