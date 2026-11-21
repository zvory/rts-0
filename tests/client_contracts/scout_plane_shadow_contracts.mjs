import { assert } from "./assertions.mjs";
import { KIND } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { installFakePixi } from "./pixi_fakes.mjs";

{
  const restorePixi = installFakePixi();
  try {
    const parent = {
      clientWidth: 640,
      clientHeight: 480,
      appendChild(view) {
        view.parentNode = this;
      },
      removeChild(view) {
        view.parentNode = null;
      },
    };
    const renderer = await Renderer.create(parent);
    renderer._map = { tileSize: 32 };
    const stripTexture = PIXI.Texture.from("scout-plane-strip-test-texture");
    renderer._map.sun = { azimuthDegrees: 315, elevationDegrees: 35 };
    renderer._liveFrameStripTextures.set(KIND.SCOUT_PLANE, stripTexture);
    const entity = {
      id: 507,
      owner: 1,
      kind: KIND.SCOUT_PLANE,
      x: 260,
      y: 160,
      facing: 0.25,
      hp: 40,
      maxHp: 40,
      state: "move",
      scoutPlane: {
        orbitCenter: [320, 320],
      },
    };

    renderer._drawUnit(entity, new Map([[1, 0x4878c8]]), {
      playerId: 1,
      selection: new Set([entity.id]),
      resources: { oil: 100 },
    });

    const unitStrip = renderer._liveRigPools.liveUnitRigs.get(entity.id);
    const shadowRig = renderer._liveRigPools.liveUnitRigShadows.get(entity.id);
    const ring = renderer._ringRadius(entity);
    assert(unitStrip?.strip?.unit === KIND.SCOUT_PLANE, "Scout Plane live rendering uses the PNG frame strip");
    assert(unitStrip?.container.zIndex === entity.y, "frame-strip unit bodies use world-Y depth");
    assert(unitStrip?.texture === stripTexture, "Scout Plane frame-strip renderer uses the preloaded strip texture");
    assert(
      unitStrip?.frameTextures?.length === unitStrip?.strip?.frameCount,
      "Scout Plane frame strip exposes one runtime texture per declared frame",
    );
    assert(shadowRig?.texture === stripTexture, "Scout Plane shadow uses the exact aircraft alpha silhouette");
    assert(shadowRig.sprite.tint === 0 && shadowRig.container.alpha === 0.25,
      "Scout Plane shadow is translucent black, independent of team tint");
    const shadowOffset = 3 * 32 / Math.tan(35 * Math.PI / 180) / Math.sqrt(2);
    assert(Math.abs(shadowRig.container.x - entity.x - shadowOffset) < 0.001
      && Math.abs(shadowRig.container.y - entity.y - shadowOffset) < 0.001,
    "Three-tile Scout Plane altitude projects southeast away from the authored northwest sun");
    assert(shadowRig.container.rotation === unitStrip.container.rotation
      && shadowRig.sprite.scaleX === unitStrip.sprite.scaleX,
    "Scout Plane silhouette preserves aircraft orientation and size");
    assert(shadowRig.container.filters[0].options.strength === 2,
      "Scout Plane shadow has a soft blurred edge");
    assert(
      ring.rx === 31 && ring.ry === 24 && ring.cy === 2,
      "Scout Plane selection ring uses the mirrored 48x34 aircraft body with vehicle-body padding",
    );
    assert(
      renderer.layers.units.children.includes(unitStrip.container) &&
        renderer.layers.unitShadows.children.includes(shadowRig.container),
      "Scout Plane frame strip renders through the normal unit and shadow layers",
    );
    renderer._map.sun = { azimuthDegrees: 90, elevationDegrees: 45 };
    entity.facing = 1.5;
    renderer._drawUnit(entity, new Map([[1, 0x4878c8]]), { selection: new Set() }, { alpha: 0.4 });
    assert(renderer._liveRigPools.liveUnitRigShadows.get(entity.id) === shadowRig,
      "Scout Plane shadow reuses its instance across frames");
    assert(Math.abs(shadowRig.container.x - (entity.x - 96)) < 0.001
      && Math.abs(shadowRig.container.y - entity.y) < 0.001,
    "Scout Plane shadow follows a changed sun direction and elevation");
    assert(Math.abs(shadowRig.container.alpha - 0.1) < 0.001
      && shadowRig.container.rotation === unitStrip.container.rotation,
    "Scout Plane shadow follows aircraft rotation and visibility fading");
    const blur = shadowRig.shadowBlur;
    shadowRig.destroy();
    assert(blur.destroyed, "Scout Plane shadow releases its owned blur filter");
  } finally {
    restorePixi();
  }
}
