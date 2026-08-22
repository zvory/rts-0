import { assertDeepEqual } from "./assertions.mjs";
import { _drawMortarImpacts, _drawMortarShells } from "../../client/src/renderer/feedback.js";
import { drawPanzerfaustProjectile } from "../../client/src/renderer/panzerfaust_feedback.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

const priorNow = performance.now;
const fixedNow = 2000;
const rocket = {
  fromX: 100,
  fromY: 50,
  toX: 300,
  toY: 250,
  durationMs: 2000,
  createdAt: 1500,
  rocket: true,
};
const barrageGfx = new RecordingGraphics();
const panzerfaustGfx = new RecordingGraphics();

performance.now = () => fixedNow;
try {
  _drawMortarShells.call({ _feedbackGfx: barrageGfx }, { liveMortarShells: () => [rocket] });
  drawPanzerfaustProjectile(panzerfaustGfx, rocket, fixedNow);
} finally {
  performance.now = priorNow;
}
assertDeepEqual(
  barrageGfx.calls,
  panzerfaustGfx.calls,
  "Rocket Truck barrage rockets reuse the Panzerfaust projectile visual exactly",
);

const impact = { x: 320, y: 240, radiusTiles: 2, seed: 77, createdAt: 1700 };
const mortarGfx = new RecordingGraphics();
const rocketGfx = new RecordingGraphics();
performance.now = () => fixedNow;
try {
  _drawMortarImpacts.call(
    { _feedbackGfx: mortarGfx, _map: { tileSize: 32 } },
    { liveMortarImpacts: () => [impact] },
  );
  _drawMortarImpacts.call(
    { _feedbackGfx: rocketGfx, _map: { tileSize: 32 } },
    { liveMortarImpacts: () => [{ ...impact, rocket: true }] },
  );
} finally {
  performance.now = priorNow;
}
assertDeepEqual(
  rocketGfx.calls,
  mortarGfx.calls,
  "Rocket Truck impacts reuse the Mortar explosion visual exactly",
);
