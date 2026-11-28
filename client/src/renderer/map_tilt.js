// Experimental full-battlefield plane; the HUD and marquee stay in screen space.
// Overscan prevents missing terrain at the far corners after perspective projection.
export class MapTiltPresentation {
  constructor(app) {
    this.app = app;
    this.texture = null;
    this.mesh = null;
  }

  present(world, degrees) {
    const { renderer, stage } = this.app;
    const w = renderer.screen.width;
    const h = renderer.screen.height;
    const resolution = renderer.resolution;
    if (!this.texture || this.texture.width !== w * 2 || this.texture.height !== h * 2
      || this.texture.source.resolution !== resolution) {
      this.destroy();
      this.texture = PIXI.RenderTexture.create({ width: w * 2, height: h * 2, resolution });
      this.mesh = new PIXI.PerspectiveMesh({ texture: this.texture, verticesX: 32, verticesY: 32 });
      stage.addChildAt(this.mesh, 0);
    }
    const angle = degrees * Math.PI / 180;
    const project = (x, y) => {
      const depth = 1 - y * Math.sin(angle) / (h * 3);
      return [w / 2 + x / depth, h / 2 + y * Math.cos(angle) / depth];
    };
    this.mesh.setCorners(...project(-w, -h), ...project(w, -h),
      ...project(w, h), ...project(-w, h));
    world.visible = true;
    try {
      renderer.render({ container: world, target: this.texture, clear: true });
      world.visible = false;
      this.app.render();
    } finally {
      world.visible = true;
    }
  }

  destroy() {
    this.mesh?.destroy();
    this.texture?.destroy(true);
    this.mesh = null;
    this.texture = null;
  }
}

export function presentBattlefield(renderer) {
  if (renderer._mapTiltDegrees) {
    renderer._mapTiltPresentation ||= new MapTiltPresentation(renderer.app);
    renderer._mapTiltPresentation.present(renderer.world, renderer._mapTiltDegrees);
  } else {
    renderer._mapTiltPresentation?.destroy();
    renderer._mapTiltPresentation = null;
    renderer.app.render();
  }
}
