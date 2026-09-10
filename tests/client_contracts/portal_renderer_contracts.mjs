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
  const renderer = await Renderer.create(parent, { renderClock: { now: () => 1600 } });
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
  const portalCalls = renderer._pools.buildingOverlays.get(portal.id)?.calls || [];
  assert(
    portalCalls.filter((call) => call[0] === "drawCircle").length >= 40,
    "completed Portal renders a dense animated vortex particle field",
  );
  assert(
    portalCalls.some((call) => call[0] === "lineTo"),
    "completed Portal renders rotating vortex arcs and energy spokes",
  );

  const portalScaffold = { ...portal, id: 507, buildProgress: 0.5 };
  renderer._drawBuilding(portalScaffold, ownerColors, state);
  assert(
    !renderer._pools.buildingOverlays.has(portalScaffold.id),
    "Portal vortex remains dormant during construction",
  );

  renderer.destroy();
} finally {
  restorePixi();
}

console.log("portal_renderer_contracts: ok");
