import assert from "node:assert/strict";
import { VersionGuard, canRefreshClient } from "../../client/src/version_guard.js";
import { Net } from "../../client/src/net.js";
import { COMPACT_SNAPSHOT_VERSION } from "../../client/src/protocol.js";

const previousBuild = globalThis.__RTS_BUILD__;

function fixture({ lobby = false, clientBuild = "loaded-build", storage = new Map(), fetch } = {}) {
  const events = new Map();
  const makeElement = () => ({
    children: [], disabled: false,
    setAttribute() {}, addEventListener() {}, removeEventListener() {}, remove() {},
    append(...children) { this.children.push(...children); },
  });
  const doc = {
    hidden: false, body: makeElement(), createElement: makeElement,
    addEventListener(type, fn) { events.set(type, fn); },
    removeEventListener(type) { events.delete(type); },
  };
  const win = {
    sessionStorage: { getItem: (k) => storage.get(k), setItem: (k, v) => storage.set(k, v) },
    setInterval(fn) { this.interval = fn; return 1; }, clearInterval() { this.interval = null; },
    setTimeout(fn) { this.timeout = fn; return 2; }, clearTimeout() { this.timeout = null; },
    addEventListener(type, fn) { events.set(type, fn); },
    removeEventListener(type) { events.delete(type); },
  };
  const net = new Net("ws://example.test/ws");
  const state = { lobby, reloads: 0, fetches: 0 };
  const guard = new VersionGuard({
    net, clientBuild, document: doc, window: win,
    isLobby: () => state.lobby,
    reload: () => { state.reloads++; },
    fetch: fetch || (async (_url, options) => {
      state.fetches++;
      assert.equal(options.cache, "no-store");
      return { ok: true, text: async () => clientBuild };
    }),
  });
  return { guard, net, state, win, doc, events, storage };
}

// Same page stays stamped with its loaded build, even when the server changes.
{
  const f = fixture();
  f.guard.start();
  await f.guard.check();
  f.net._onMessage({ data: JSON.stringify({ t: "welcome", playerId: 1, buildId: "loaded-build" }) });
  assert.equal(f.guard.mismatch, false);
  f.net._onMessage({ data: JSON.stringify({ t: "pong", ts: 1, buildId: "new-server" }) });
  assert.equal(f.guard.mismatch, true);
  assert.equal(globalThis.__RTS_BUILD__, "loaded-build");
  assert.equal(f.state.reloads, 0, "active match is never refreshed");
  assert.equal(f.guard.el, undefined, "active rooms never mount a warning");
  f.guard.refresh();
  assert.equal(f.state.reloads, 0, "manual refresh also respects active-game safety");
  f.state.lobby = true;
  f.guard.update();
  assert.equal(f.state.reloads, 1, "returning to lobby refreshes the embedded browser");
  f.guard.update();
  assert.equal(f.state.reloads, 1, "only one navigation per page");
  f.guard.destroy();
  assert.equal(f.win.interval, null);
  assert.equal(f.events.size, 0);
  assert.equal(f.net._handlers.get("pong").size, 0);
  const next = fixture({ lobby: true, storage: f.storage });
  next.guard.warn("new-server");
  assert.equal(next.state.reloads, 0, "a stale page surviving reload cannot loop");
  assert.equal(next.guard.button.disabled, false);
  next.guard.refresh();
  assert.equal(next.state.reloads, 1, "manual retry remains available");
  next.guard.destroy();
}

// A lobby without any socket still notices deployment changes through HTTP.
{
  const f = fixture({ lobby: true, fetch: async () => ({ ok: true, text: async () => "deployed-build" }) });
  await f.guard.check();
  assert.equal(f.state.reloads, 1);
  f.guard.destroy();
}

// A newer snapshot schema records pending recovery silently until the player leaves the room.
{
  const f = fixture();
  f.guard.start();
  f.net._onMessage({ data: JSON.stringify({ t: "snapshot", v: COMPACT_SNAPSHOT_VERSION + 1 }) });
  assert.equal(f.guard.mismatch, true);
  assert.equal(f.state.reloads, 0);
  f.guard.destroy();
}

// Ordinary malformed packets, missing build ids, offline mode, and failed probes are not mismatches.
{
  const f = fixture({ lobby: true, fetch: async () => { throw new Error("offline"); } });
  f.guard.start();
  f.net._onMessage({ data: "broken JSON" });
  f.guard.observe("unknown");
  f.guard.observe(undefined);
  await f.guard.check();
  assert.equal(f.guard.mismatch, false);
  f.guard.destroy();
  const offline = fixture();
  offline.guard.start({ offline: true });
  assert.equal(offline.state.fetches, 0);
  assert.equal(offline.win.interval, undefined);
  offline.guard.destroy();
}

// Back/forward-cache navigation preserves the guard; a real page exit tears it down.
{
  const f = fixture();
  f.guard.start();
  await f.guard.check();
  f.events.get("pagehide")({ persisted: true });
  assert.equal(f.guard.destroyed, false);
  assert.equal(typeof f.win.interval, "function");
  f.net._onMessage({ data: JSON.stringify({ t: "pong", ts: 1, buildId: "new-server" }) });
  assert.equal(f.guard.mismatch, true, "restored pages still detect server build changes");
  f.events.get("pagehide")({ persisted: false });
  assert.equal(f.guard.destroyed, true);
  assert.equal(f.win.interval, null);
  assert.equal(f.events.size, 0);
}

// A hung request is bounded, non-overlapping, and aborted on teardown.
{
  let signal;
  const f = fixture({ lobby: true, fetch: async (_url, options) => {
    signal = options.signal;
    await new Promise((resolve) => signal.addEventListener("abort", resolve));
    throw new Error("aborted");
  } });
  const pending = f.guard.check();
  await f.guard.check();
  f.win.timeout();
  await pending;
  assert.equal(signal.aborted, true);
  assert.equal(f.guard.mismatch, false);
  f.guard.destroy();
}

// A probe begun in the lobby can finish during room startup without interrupting it.
{
  let finish;
  const f = fixture({ lobby: true, fetch: () => new Promise((resolve) => { finish = resolve; }) });
  const pending = f.guard.check();
  f.state.lobby = false;
  finish({ ok: true, text: async () => "new-server" });
  await pending;
  assert.equal(f.guard.mismatch, true);
  assert.equal(f.guard.el, undefined);
  assert.equal(f.state.reloads, 0);
  f.state.lobby = true;
  f.guard.update();
  assert.equal(f.state.reloads, 1);
  f.guard.destroy();
}

// Main screen and ordinary lobby are eligible; every room, authoring flow, and startup is protected.
{
  assert.equal(canRefreshClient({}, true), true, "main screen without a joined lobby refreshes");
  assert.equal(canRefreshClient({}, false), false);
  for (const field of ["match", "versionTransitionActive", "labLaunch", "labCatalogLaunch",
    "labHandoffLaunch", "replayLaunch", "matchLaunch", "snapshotStreamLaunch",
    "stressTestLaunch", "devWatch", "mapPreviewLaunch"]) {
    assert.equal(canRefreshClient({ [field]: {} }, true), false, `${field} blocks recovery`);
  }
  const f = fixture();
  f.guard.start();
  await f.guard.check();
  assert.equal(f.state.fetches, 0, "protected rooms do not run HTTP probes");
  f.guard.observe("new-server");
  assert.equal(f.guard.el, undefined, "protected rooms remain visually untouched");
  f.state.lobby = true;
  f.guard.update();
  assert.equal(f.state.reloads, 1);
  f.state.lobby = false;
  f.guard.update();
  assert.equal(f.guard.el.hidden, true, "a stale reload notice disappears on entering a room");
  f.guard.destroy();
}

if (previousBuild === undefined) delete globalThis.__RTS_BUILD__;
else globalThis.__RTS_BUILD__ = previousBuild;
console.log("version guard contracts passed");

export function testVersionGuardAppLifecycle(App) {
  {
    const app = Object.create(App.prototype);
    let resetCount = 0;
    let showCount = 0;
    let warningCount = 0;
    app.stopHeartbeat = () => {};
    app.socketOpen = true;
    app.intentionalIdleDisconnect = false;
    app.match = null;
    app.versionTransitionActive = true;
    let versionUpdates = 0;
    app.versionGuard = { update() {
      assert(!app.versionTransitionActive && showCount === 1,
        "disconnect recovery permits version refresh only after restoring the lobby");
      versionUpdates += 1;
    } };
    app.requiresConnectionOnStart = () => false;
    app.lobby = {
      resetToBrowser() { resetCount += 1; },
      show() { showCount += 1; },
    };
    app.showConnectionWarning = () => { warningCount += 1; };
    app.showConnectionLost = () => { warningCount += 1; };
    app.onClose();
    assert(resetCount === 1 && showCount === 1,
      "an ordinary lobby disconnect resets and shows the main lobby browser");
    assert(warningCount === 0, "an ordinary lobby disconnect does not show a warning");
    assert(versionUpdates === 1, "disconnect recovery updates a pending version warning");
  }
  {
    const app = Object.create(App.prototype);
    const transitions = [];
    app.versionGuard = { update() { transitions.push(app.versionTransitionActive); } };
    app.onLobbyForMatchLaunch({ countdownActive: true });
    app.onLobbyForMatchLaunch({ countdownActive: true });
    app.onLobbyForMatchLaunch({ countdownActive: false });
    assert(JSON.stringify(transitions) === "[true,true,false]",
      "lobby membership updates preserve countdown refresh protection until cancellation");
  }
}
