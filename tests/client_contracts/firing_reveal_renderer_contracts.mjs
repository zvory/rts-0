import { KIND } from "../../client/src/protocol.js";
import { Renderer } from "../../client/src/renderer/index.js";
import { assert } from "./assertions.mjs";
import { installFakePixi } from "./pixi_fakes.mjs";

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
  const damagedReveal = {
    id: 503,
    owner: 2,
    kind: KIND.RIFLEMAN,
    x: 160,
    y: 160,
    hp: 30,
    maxHp: 45,
    aboveFogReveal: true,
    shotReveal: true,
  };
  const fullHealthReveal = { ...damagedReveal, id: 504, hp: 45 };
  const visualOnlyGhost = { ...damagedReveal, id: 505, aboveFogReveal: false };

  renderer._drawAboveFogHp(damagedReveal);
  renderer._drawAboveFogHp(fullHealthReveal);
  renderer._drawAboveFogHp(visualOnlyGhost);

  const hpBar = renderer._pools.aboveFogHpBars.get(damagedReveal.id);
  assert(hpBar?.rtsBackground && hpBar?.rtsFill, "damaged actionable reveals draw an HP bar");
  assert(
    Math.abs(hpBar.rtsFill.scaleX - damagedReveal.hp / damagedReveal.maxHp) < 0.001,
    "actionable reveal HP bars use authoritative health",
  );
  assert(
    !renderer._pools.aboveFogHpBars.has(fullHealthReveal.id),
    "full-health actionable reveals do not draw an HP bar",
  );
  assert(
    !renderer._pools.aboveFogHpBars.has(visualOnlyGhost.id),
    "event-only shot-reveal ghosts never draw an HP bar",
  );
  const fogIndex = renderer.world.children.indexOf(renderer.layers.fog);
  const revealIndex = renderer.world.children.indexOf(renderer.layers.shotReveals);
  const hpIndex = renderer.world.children.indexOf(renderer.layers.aboveFogHpBars);
  const feedbackIndex = renderer.world.children.indexOf(renderer.layers.feedback);
  assert(
    fogIndex < revealIndex && revealIndex < hpIndex && hpIndex < feedbackIndex,
    "actionable reveal HP bars render above fog and revealed units but below tactical feedback",
  );
} finally {
  restorePixi();
}

console.log("firing_reveal_renderer_contracts: ok");
