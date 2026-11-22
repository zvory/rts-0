import assert from "node:assert/strict";
import { GameState } from "../../client/src/state.js";
import { S } from "../../client/src/protocol.js";
import { createMatchStartupInbox } from "../../client/src/match_startup_inbox.js";

function connection() {
  const handlers = new Map();
  return {
    on(type, handler) {
      if (!handlers.has(type)) handlers.set(type, new Set());
      handlers.get(type).add(handler);
    },
    off(type, handler) { handlers.get(type)?.delete(handler); },
    emit(type, message) {
      for (const handler of handlers.get(type) || []) handler(message);
    },
    listenerCount() { return [...handlers.values()].reduce((n, set) => n + set.size, 0); },
  };
}

const start = {
  playerId: 1,
  players: [],
  map: {
    width: 2, height: 2, tileSize: 64, terrain: [0, 0, 0, 0],
    resources: [
      { id: 1, kind: "steel", x: 32, y: 32, remaining: 625 },
      { id: 2, kind: "oil", x: 96, y: 32, remaining: 962 },
      { id: 3, kind: "steel", x: 32, y: 96, remaining: 625 },
    ],
  },
};
const amounts = (state) => state.map.resources.map((node) => node.remaining);
const snapshot = (tick, resourceDeltas = [], resourceReset = false) => ({
  t: S.SNAPSHOT, tick, resourceDeltas, resourceReset,
  entities: [{ id: 273, kind: "resource_depot", owner: 1, x: 64, y: 64 }],
  visibleTiles: [true, false, false, false], exploredTiles: [true, true, true, true],
});
const baseline = () => snapshot(2, start.map.resources.map(({ id, remaining }) => ({ id, remaining })), true);

// Sequential delivery and delayed/coalesced delivery must have the same persistent resource state.
for (const messages of [
  [baseline(), snapshot(3)],
  [baseline(), snapshot(3, [{ id: 1, remaining: 600 }]), snapshot(4, [{ id: 1, remaining: 0 }]), snapshot(5)],
  [baseline(), snapshot(3, [{ id: 1, remaining: 0 }], true), snapshot(4, [{ id: 2, remaining: 10 }]), snapshot(5)],
  [baseline(), snapshot(3, [], true), snapshot(4)],
  // A receiver with an existing baseline may buffer only incremental messages.
  [snapshot(3, [{ id: 1, remaining: 0 }]), snapshot(4, [{ id: 2, remaining: 500 }])],
]) {
  const net = connection();
  const direct = new GameState(start);
  const delayed = new GameState(start);
  if (!messages[0].resourceReset) {
    direct.applySnapshot(baseline());
    delayed.applySnapshot(baseline());
  }
  const inbox = createMatchStartupInbox(net);
  for (const message of messages) {
    direct.applySnapshot(message);
    net.emit(S.SNAPSHOT, message);
  }
  let delivered = 0;
  const match = { onSnapshot(message) { delivered++; delayed.applySnapshot(message); } };
  inbox.stop();
  inbox.flush(match);
  assert.deepEqual(amounts(delayed), amounts(direct), "resource state survives snapshot coalescing");
  assert.equal(delivered, 1, "only the latest entity/fog snapshot is applied");
  assert.equal(delayed._cur.tick, messages.at(-1).tick);
  assert.deepEqual(delayed.visibleTiles, messages.at(-1).visibleTiles);
  assert.equal(net.listenerCount(), 0);
  delayed.applySnapshot(snapshot(6, [{ id: 3, remaining: 77 }]));
  inbox.flush(match);
  assert.equal(delivered, 1, "consumed handoff cannot reapply an old replacement");
  assert.equal(delayed.resourceById.get(3).remaining, 77);
}

// Slow loading must retain one row per id, even across many replacement epochs.
{
  const net = connection();
  const direct = new GameState(start);
  const delayed = new GameState(start);
  const inbox = createMatchStartupInbox(net);
  for (let tick = 0; tick < 10000; tick++) {
    const message = snapshot(tick, [{ id: tick % 3 + 1, remaining: tick % 97 }], tick % 101 === 0);
    direct.applySnapshot(message);
    net.emit(S.SNAPSHOT, message);
  }
  inbox.stop();
  inbox.flush({ onSnapshot(message) {
    assert.ok(message.resourceDeltas.length <= start.map.resources.length,
      "loading duration cannot grow the retained resource row count");
    delayed.applySnapshot(message);
  } });
  assert.deepEqual(amounts(delayed), amounts(direct));
}

/** Exercise the actual App -> Match.create path with deferred preparation and real GameState. */
export async function testAppResourceStartup({ App, Match, dom }) {
  const originalCreate = Match.create;
  const savedDom = { ...dom };
  const element = () => ({ hidden: true, classList: { remove() {} } });
  for (const key of ["gameOver", "gameScreen", "lobbyScreen", "labEntryScreen", "devLinks", "betaBadge"]) {
    dom[key] = element();
  }
  try {
    // Replay, active-player, and late-joining spectator share the same startup ownership.
    for (const mode of [{ replay: {} }, {}, { spectator: true }]) {
      for (const warmed of [true, false]) {
        const net = connection();
        let settlePreparation;
        let finishColdRenderer;
        const preparation = new Promise((resolve) => { settlePreparation = resolve; });
        let created;
        class Probe {
          constructor(_net, payload) {
            this.state = new GameState(payload);
            this.renderClock = {};
            this.onSnapshot = (message) => this.state.applySnapshot(message);
            net.on(S.SNAPSHOT, this.onSnapshot);
            this.destroy = () => net.off(S.SNAPSHOT, this.onSnapshot);
            created = this;
          }
        }
        let createCalls = 0;
        Match.create = (...args) => { createCalls++; return originalCreate.call(Probe, ...args); };
        const app = Object.create(App.prototype);
        Object.assign(app, {
          net, matchStartGeneration: 0, matchEndedGeneration: 0,
          rendererPreparationSlot: { settleForStart: () => preparation },
          rendererBackendBundle: {
            id: "test",
            createRenderer: () => new Promise((resolve) => { finishColdRenderer = resolve; }),
          },
          branchStaging: { hide() {} },
          setCleanPresentation() {}, destroyLabShell() {}, takeMatchCameraView() { return null; },
          clearScoreboard() {},
        });
        app.onStart({ ...start, ...mode });
        assert.equal(createCalls, 1, "App enters Match.create before waiting on renderer preparation");
        net.emit(S.SNAPSHOT, baseline());
        net.emit(S.SNAPSHOT, snapshot(3));
        const renderer = { destroy() {} };
        settlePreparation(warmed ? { renderer, attach() {}, destroy() {} } : null);
        if (!warmed) {
          // Cross the preparation/disposal barrier; cold renderer startup is another loss window.
          await Promise.resolve();
          await Promise.resolve();
          assert.equal(typeof finishColdRenderer, "function");
          net.emit(S.SNAPSHOT, snapshot(4, [{ id: 1, remaining: 0 }]));
          finishColdRenderer(renderer);
        }
        await app.matchStartPromise;
        assert.equal(app.match, created);
        assert.deepEqual(amounts(created.state), warmed ? [625, 962, 625] : [0, 962, 625]);
        assert.equal(net.listenerCount(), 1, "startup listeners transfer to the live match without leaks");
        net.emit(S.SNAPSHOT, snapshot(5, [{ id: 2, remaining: 900 }]));
        assert.deepEqual(amounts(created.state), warmed ? [625, 900, 625] : [0, 900, 625]);
        // A perspective/seek replacement must discard knowledge absent from the new projection.
        net.emit(S.SNAPSHOT, snapshot(6, [{ id: 3, remaining: 12 }], true));
        assert.deepEqual(amounts(created.state), [0, 0, 12]);
        app.match.destroy();
        assert.equal(net.listenerCount(), 0);
      }
    }
    // Preparation failure must release the inbox even when no Match was constructed.
    const failedNet = connection();
    await assert.rejects(originalCreate.call(class {}, failedNet, start, null, null, null, null, null, {
      rendererPreparation: Promise.reject(new Error("preparation failed")),
    }), /preparation failed/);
    assert.equal(failedNet.listenerCount(), 0);
  } finally {
    Match.create = originalCreate;
    Object.assign(dom, savedDom);
  }
}
