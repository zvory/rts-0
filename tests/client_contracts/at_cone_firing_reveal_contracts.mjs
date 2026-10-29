import { assert, assertDeepEqual } from "./assertions.mjs";
import { KIND, SETUP } from "../../client/src/protocol.js";
import { markActionableFiringReveal } from "../../client/src/state_firing_reveal.js";
import { buildRendererFeedbackView } from "../../client/src/renderer/feedback_view_model.js";
import { _drawSelectedUnitRanges } from "../../client/src/renderer/unit_ranges.js";
import { RecordingGraphics } from "./pixi_fakes.mjs";

const map = { width: 32, height: 32, tileSize: 32 };
const gun = {
  id: 701, owner: 2, kind: KIND.ANTI_TANK_GUN, x: 320, y: 320,
  setupState: SETUP.DEPLOYED, setupFacing: 0.75, facing: 1.0, weaponFacing: 1.0,
};
const memory = {
  id: gun.id, owner: gun.owner, x: gun.x, y: gun.y, facing: gun.setupFacing,
  observedTick: 100,
};
const revealed = markActionableFiringReveal(gun, map, new Uint8Array(1024), 1);
assert(revealed.aboveFogReveal === true, "fixture must exercise a firing-only above-fog gun");

for (const spectator of [false, true]) {
  const state = {
    playerId: 1, spectator, map,
    players: [{ id: 1, teamId: 1, color: "#0000ff" }, { id: 2, teamId: 2, color: "#ff0000" }],
  };
  const view = (entities, memories = []) => buildRendererFeedbackView(state, {
    entities, rememberedEnemyAntiTankGunThreats: memories,
  });
  for (const entity of [gun, revealed]) {
    const feedback = view([entity], [memory]);
    const threats = feedback.enemyAntiTankGunThreats();
    assert(threats.length === 1 && threats[0].threatMemory === false,
      "ordinary sight and authoritative firing reveals show one live cone, overriding memory");
    assert(threats[0].setupFacing === gun.setupFacing,
      "firing reveal cone retains fixed setup facing rather than tracked weapon facing");
    const gfx = new RecordingGraphics();
    _drawSelectedUnitRanges.call({ _feedbackGfx: gfx, _map: map }, feedback);
    assert(gfx.calls.some((call) => call[0] === "arc"), "above-fog live cone reaches drawing");
    assert(!gfx.calls.some((call) => call[0] === "lineStyle" && call[1] === 1.25 && call[2] === 0xfff1f5),
      "live cone has no stale question-mark marker");
  }
  const expired = view([], [memory]);
  assertDeepEqual(expired.enemyAntiTankGunThreats().map((e) => [e.id, e.threatMemory, e.facing]),
    [[gun.id, true, gun.setupFacing]], "expired firing reveal becomes a fixed last-observed memory cone");
  const gfx = new RecordingGraphics();
  _drawSelectedUnitRanges.call({ _feedbackGfx: gfx, _map: map }, expired);
  assert(gfx.calls.some((call) => call[0] === "lineStyle" && call[1] === 1.25 && call[2] === 0xfff1f5),
    "expired reveal draws the question-mark memory marker");
  for (const excluded of [{ ...revealed, shotReveal: true }, { ...revealed, visionOnly: true },
    { ...revealed, setupState: SETUP.PACKED }]) {
    assert(view([excluded]).enemyAntiTankGunThreats().length === 0,
      "event ghosts, render-only intel, and packed guns cannot produce live cones");
  }
  assert(view([]).enemyAntiTankGunThreats().length === 0,
    "server-cleared memory leaves no client-invented cone");
}
