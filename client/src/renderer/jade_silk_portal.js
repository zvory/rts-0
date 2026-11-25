import { createWorkerSafeCanvas } from "./raster_primitives.js";

// Traditional layered-sprite effect. The small source texture is generated once;
// frames only update three transforms and their opacity.
const SIZE = 256;
const RATES = [.32, -.19, .47];
const SIZES = [254, 282, 133];
const ASPECTS = [1.1, .86, 1.1];
const OPACITIES = [.86, .6, .72];
function makeSilkTexture(pixi) {
  const canvas = createWorkerSafeCanvas(SIZE, SIZE);
  const ctx = canvas.getContext("2d");
  const image = ctx.createImageData(SIZE, SIZE);
  for (let y = 0; y < SIZE; y++) for (let x = 0; x < SIZE; x++) {
    const px = (x + .5 - SIZE / 2) / (SIZE / 2);
    const py = (y + .5 - SIZE / 2) / (SIZE / 2);
    const radius = Math.hypot(px, py);
    const angle = Math.atan2(py, px);
    const twist = angle * 5 + 11 * Math.log(radius + .16);
    const fine = Math.sin(angle * 13 + radius * 27 + Math.sin(angle * 3) * 1.3);
    const plume = Math.pow(.5 + .5 * Math.cos(twist + fine * .24), 9);
    const silk = Math.pow(.5 + .5 * Math.cos(twist * 1.8 + Math.sin(radius * 13) * 1.5), 20);
    const envelope = Math.max(0, 1 - radius) ** .7 * Math.min(1, radius * 10);
    const inner = Math.exp(-radius * radius * 28);
    const light = (plume * .85 + silk * .34) * envelope;
    const i = (y * SIZE + x) * 4;
    image.data[i] = Math.min(255, light * 110 + inner * 70);
    image.data[i + 1] = Math.min(255, light * 250 + inner * 150);
    image.data[i + 2] = Math.min(255, light * 153 + inner * 125);
    image.data[i + 3] = Math.min(255, (light * 1.8 + inner * .3) * 255);
  }
  ctx.putImageData(image, 0, 0);
  const texture = pixi.Texture.from(canvas);
  texture.source.scaleMode = "linear";
  return texture;
}

export class JadeSilkPortal {
  constructor(renderer, pixi = globalThis.PIXI) {
    this.renderer = renderer;
    this.pixi = pixi;
    this.texture = makeSilkTexture(pixi);
  }
  draw(entity, width, now) {
    const { renderer, pixi } = this;
    let root = renderer._pools.buildingShadows.get(entity.id);
    if (!root) {
      root = new pixi.Container();
      // Atlas coordinates, strictly underneath the slanted beam and pillars.
      // The lower edge extends behind the opaque platform to seal every gap.
      const aperture = [105, 104, 285, 139, 286, 278, 100, 252];
      const background = new pixi.Graphics().poly(aperture).fill(0x011c1c);
      root.addChild(background);
      const mask = new pixi.Graphics().poly(aperture).fill(0xffffff);
      root.addChild(mask);
      const flow = new pixi.Container();
      flow.mask = mask;
      root.addChild(flow);
      root.silk = [];
      for (let i = 0; i < 3; i++) {
        const sprite = new pixi.Sprite(this.texture);
        sprite.anchor.set(.5);
        sprite.position.set(194, 186);
        sprite.blendMode = "add";
        sprite.tint = [0x71ffbe, 0x23bfb2, 0xffe49b][i];
        flow.addChild(sprite);
        root.silk.push(sprite);
      }
      const originalDestroy = root.destroy.bind(root);
      root.destroy = () => originalDestroy({ children: true });
      renderer.layers.buildingShadows.addChild(root);
      renderer._pools.buildingShadows.set(entity.id, root);
    }
    renderer._seen.buildingShadows.add(entity.id);
    root.visible = true;
    root.position.set(entity.x - width * .55, entity.y - width * .55);
    root.scale.set(width * 1.1 / 384);
    const time = now / 1000;
    for (let i = 0; i < 3; i++) {
      const sprite = root.silk[i];
      sprite.rotation = time * RATES[i] + i * 2.3;
      const size = SIZES[i] * (1 + .035 * Math.sin(time * 1.5 + i));
      sprite.scale.set(size / SIZE, size / SIZE * ASPECTS[i]);
      sprite.alpha = OPACITIES[i] * (.92 + .08 * Math.sin(time * 2 + i));
    }
  }
  destroy() {
    this.texture.destroy(true);
  }
}
