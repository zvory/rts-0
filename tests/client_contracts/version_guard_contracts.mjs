import assert from "node:assert/strict";
import { VersionGuard } from "../../client/src/version_guard.js";
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
  assert.equal(f.guard.button.disabled, true);
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

// Reproduce the incident: a newer snapshot schema must surface recovery, not just disappear.
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
  const f = fixture({ fetch: async () => { throw new Error("offline"); } });
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

// A hung request is bounded, non-overlapping, and aborted on teardown.
{
  let signal;
  const f = fixture({ fetch: async (_url, options) => {
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

if (previousBuild === undefined) delete globalThis.__RTS_BUILD__;
else globalThis.__RTS_BUILD__ = previousBuild;
console.log("version guard contracts passed");
