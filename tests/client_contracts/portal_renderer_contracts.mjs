import { assert } from "./assertions.mjs";
import { KIND } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { installFakePixi } from "./pixi_fakes.mjs";

const restorePixi = installFakePixi();
try {
  const parent = {
    clientWidth: 640,
    clientHeight: 480,
    appendChild(view) { view.parentNode = this; },
    removeChild(view) { view.parentNode = null; },
  };
  let renderNow = 1600;
  const renderer = await Renderer.create(parent, { renderClock: { now: () => renderNow } });
  renderer._map = { tileSize: 32 };
  const portal = {
    id: 506,
    owner: 1,
    kind: KIND.PORTAL,
    x: 320,
    y: 160,
    hp: 165,
    maxHp: 165,
    state: "idle",
  };
  const ownerColors = new Map([[1, 0x4878c8]]);
  const state = { playerId: 1, players: [{ id: 1, color: "#4878c8" }] };

  renderer._drawBuilding(portal, ownerColors, state);
  const aperture = renderer._pools.buildingShadows.get(portal.id);
  const texture = renderer._jadeSilkPortal.texture;
  assert(aperture?.silk.length === 3, "completed Portal renders three silk layers");
  const [background, mask, flow] = aperture.children;
  assert(flow.mask === mask, "animated layers are clipped behind the gate frame");
  assert(background.calls.some(call => call[0] === "beginFill"),
    "opaque background seals the full opening behind the animated layers");
  const rotations = aperture.silk.map(sprite => sprite.rotation);
  const staticCalls = JSON.stringify([background.calls, mask.calls]);
  renderNow = 2600;
  // Exercise the effect at a second visual time without replacing pooled GPU resources.
  renderer._drawBuilding(portal, ownerColors, state);
  assert(aperture.silk.some((sprite, i) => sprite.rotation !== rotations[i]),
    "silk animation advances with renderer visual time");
  assert(aperture.silk.every(sprite => sprite.texture === texture),
    "all layers share the single reusable texture");
  assert(JSON.stringify([background.calls, mask.calls]) === staticCalls,
    "animation does not rebuild aperture geometry");
  assert(texture.sourceUpdateCount === 0, "animation never uploads new texture pixels");
  renderer._seen.buildingShadows.clear();
  renderer._sweep();
  assert(!aperture.visible, "portal effect disappears when no longer rendered/visible");
  renderer._drawBuilding(portal, ownerColors, state);
  assert(aperture.visible && renderer._pools.buildingShadows.get(portal.id) === aperture,
    "returning visibility reuses the aperture");

  const producingPortal = { ...portal, id: 508, prodProgress: 0.5 };
  renderer._drawBuilding(producingPortal, ownerColors, state);
  assert(
    renderer._pools.buildingShadows.get(producingPortal.id)?.silk.length === 3 &&
      renderer._pools.buildingOverlays.get(producingPortal.id)?.calls.length > 0,
    "Portal effects coexist with production and status overlays",
  );

  const portalScaffold = { ...portal, id: 507, buildProgress: 0.5 };
  renderer._drawBuilding(portalScaffold, ownerColors, state);
  assert(
    !renderer._pools.buildingShadows.has(portalScaffold.id),
    "Portal vortex remains dormant during construction",
  );
  assert(
    renderer.world.children.indexOf(renderer.layers.buildingShadows) <
      renderer.world.children.indexOf(renderer.layers.buildings),
    "Portal aperture renders behind gate artwork and overlays",
  );

  renderer.destroy();
  assert(texture.destroyed, "renderer teardown releases shared portal texture");
  renderer.destroy();
} finally {
  restorePixi();
}

console.log("portal_renderer_contracts: ok");
