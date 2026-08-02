import { Minimap } from "../client/src/minimap.js";
import { EVENT, KIND } from "../client/src/protocol.js";

function assert(condition, message) {
  if (!condition) throw new Error(message || "Assertion failed");
}

function assertApprox(actual, expected, epsilon, message) {
  assert(Math.abs(actual - expected) <= epsilon, `${message}: expected ${expected}, got ${actual}`);
}

function recordingContext() {
  return {
    calls: [], fillStyle: "", strokeStyle: "", lineWidth: 1, globalAlpha: 1,
    fillRect(...args) { this.calls.push({ op: "fillRect", args, fillStyle: this.fillStyle }); },
    strokeRect(...args) { this.calls.push({ op: "strokeRect", args }); },
    save() {}, restore() {}, beginPath() {},
    arc(...args) { this.calls.push({ op: "arc", args }); },
    stroke() {
      this.calls.push({ op: "stroke", strokeStyle: this.strokeStyle, lineWidth: this.lineWidth });
    },
  };
}

export function runMinimapAttackAlertContracts() {
  const context = recordingContext();
  const canvas = {
    width: 220, height: 220,
    getContext: () => context,
    addEventListener() {},
    removeEventListener() {},
  };
  const entities = [
    { id: 11, kind: KIND.RIFLEMAN, owner: 1, x: 42, y: 60 },
    { id: 12, kind: KIND.CITY_CENTRE, owner: 1, x: 160, y: 160 },
    { id: 13, kind: KIND.RIFLEMAN, owner: 2, x: 40, y: 60 },
  ];
  const state = {
    map: { tileSize: 32 }, playerId: 1, events: [],
    entitiesInterpolated() { return entities; },
  };
  const minimap = new Minimap(canvas, state, {}, {}, { issueCommand() {} });
  minimap._worldToCanvas = (x, y) => ({ x, y });

  minimap.ping(40, 60, "alert", true);
  minimap._pings[0].startedAt = 100;
  minimap._drawPings(1200);
  const arcs = context.calls.filter((call) => call.op === "arc");
  const strokes = context.calls.filter((call) => call.op === "stroke");
  assert(arcs.length === 2, "attack alert draws both the red ring and its inner rim");
  assertApprox(arcs[0].args[2], 11.5, 0.001, "attack alert advances halfway through 2.2 seconds");
  assertApprox(arcs[1].args[2], 9.5, 0.001, "attack alert rim stays two pixels inside");
  assert(strokes[0].strokeStyle === "#ff4d4d" && strokes[0].lineWidth === 2,
    "attack alert keeps its strong red outer stroke");
  assert(strokes[1].strokeStyle === "rgba(255,255,255,0.95)" && strokes[1].lineWidth === 1,
    "attack alert draws a crisp white inner stroke");

  let flashing = minimap._underAttackFlashEntityIds(entities, 250);
  assert(flashing.has(11) && !flashing.has(13), "attack alert resolves the nearest local entity");
  context.calls.length = 0;
  minimap._drawEntities([entities[0]], { attackFlashIds: flashing });
  assert(context.calls.some((call) => call.op === "fillRect" && call.fillStyle === "#ffffff"),
    "attack alert paints the victim icon white");
  entities[0].x = 180;
  flashing = minimap._underAttackFlashEntityIds(entities, 450);
  assert(!flashing.has(11),
    "attack alert restores the team color in the second phase");
  context.calls.length = 0;
  minimap._drawEntities([entities[0]], { attackFlashIds: flashing });
  assert(context.calls.some(
    (call) => call.op === "fillRect" && call.fillStyle === minimap._blipColor(entities[0]),
  ), "the normal strobe phase paints the victim in its team color");
  assert(minimap._underAttackFlashEntityIds(entities, 750).has(11),
    "the resolved victim keeps flashing after moving");
  entities[0].owner = 2;
  assert(!minimap._underAttackFlashEntityIds(entities, 850).has(11),
    "an entity that is no longer locally owned stops flashing");
  entities[0].owner = 1;

  minimap._pings.length = 0;
  entities[0].x = 42;
  state.events = [{ e: EVENT.DEATH, id: 99, x: 40, y: 60, kind: KIND.RIFLEMAN }];
  minimap.ping(40, 60, "alert", true);
  minimap._pings[0].startedAt = 100;
  assert(!minimap._underAttackFlashEntityIds(entities, 250).has(11),
    "a lethal attack does not transfer its strobe to a nearby survivor");
  state.events = [];
  assert(minimap._underAttackFlashEntityIds(entities, 750).size === 0,
    "a lethal attack stays resolved without a replacement target");

  minimap._pings.length = 0;
  minimap.ping(160, 160, "alert", true);
  minimap._pings[0].startedAt = 100;
  assert(minimap._underAttackFlashEntityIds(entities, 250).has(12),
    "attack alerts strobe local buildings too");
  state.events = [{ e: EVENT.DEATH, id: 99, x: 160, y: 160, kind: KIND.RIFLEMAN }];
  assert(minimap._underAttackFlashEntityIds(entities, 750).has(12),
    "a later snapshot cannot suppress an earlier nonlethal alert target");
  assert(minimap._underAttackFlashEntityIds(entities, 2300).size === 0,
    "attack icon strobe ends after 2.2 seconds");

  minimap._pings.length = 0;
  minimap.ping(40, 60, "alert", true);
  minimap._pings[0].startedAt = 100;
  minimap._drawPings(2200);
  assert(minimap._pings.length === 1, "attack alert remains after the former 1.1-second lifetime");
  minimap._drawPings(2300);
  assert(minimap._pings.length === 0, "attack alert expires after 2.2 seconds");

  minimap.ping(40, 60, "alert");
  minimap._pings[0].startedAt = 100;
  context.calls.length = 0;
  minimap._drawPings(550);
  assert(context.calls.filter((call) => call.op === "arc").length === 1,
    "generic positional alerts retain their single-ring treatment");
  minimap._drawPings(1000);
  assert(minimap._pings.length === 0, "generic alerts retain their 900-millisecond lifetime");
  minimap.destroy();

  const wideState = {
    map: { width: 8, height: 4, tileSize: 32, terrain: new Array(32).fill(0) },
    playerId: 1,
    events: [],
    entitiesInterpolated() { return []; },
  };
  const wideMinimap = new Minimap(canvas, wideState, {}, {}, { issueCommand() {} });
  assert(wideMinimap._ensureTransform(), "rectangular minimap transform resolves");
  assertApprox(wideMinimap._scale, 220 / 256, 0.000001, "rectangular minimap uses one undistorted scale");
  assertApprox(wideMinimap._offX, 0, 0.000001, "wide minimap reaches the horizontal edges");
  assertApprox(wideMinimap._offY, 55, 0.000001, "wide minimap letterboxes the shorter vertical axis");
  const wideBottomRight = wideMinimap._canvasToWorld(220, 165);
  assertApprox(wideBottomRight.x, 255, 0.000001, "rectangular minimap clamps clicks to its horizontal world bound");
  assertApprox(wideBottomRight.y, 127, 0.000001, "rectangular minimap clamps clicks to its vertical world bound");
  wideMinimap.destroy();
}
