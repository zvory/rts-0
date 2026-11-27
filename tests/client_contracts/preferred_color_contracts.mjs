import { strict as assert } from "node:assert";
import { createPreferredColorSettings } from "../../client/src/preferred_color_settings.js";
import { Net } from "../../client/src/net.js";

const previous = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
try {
  const values = new Map();
  Object.defineProperty(globalThis, "localStorage", { configurable: true, value: {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: (key) => values.delete(key),
  } });
  const changes = [];
  const settings = createPreferredColorSettings((color) => changes.push(color));
  assert.equal(settings.read(), null);
  settings.write("#009e73");
  assert.equal(createPreferredColorSettings().read(), "#009e73");
  settings.write("#123456");
  assert.equal(settings.read(), "#009e73");
  assert.deepEqual(changes, ["#009e73"]);
  const sent = [];
  Net.prototype.join.call({ preferredColor: settings, _send: (payload) => sent.push(payload) }, "Player", "Room");
  assert.equal(sent[0].t, "join");
  assert.deepEqual(sent[1], { t: "setPreferredColor", color: "#009e73" });
  settings.write(null);
  assert.equal(createPreferredColorSettings().read(), null);
  values.set("rts.player.preferredColor", "#123456");
  assert.equal(createPreferredColorSettings().read(), null);
  Object.defineProperty(globalThis, "localStorage", { configurable: true, get() { throw Error("blocked"); } });
  const unavailable = createPreferredColorSettings();
  unavailable.write("#cc79a7");
  assert.equal(unavailable.read(), "#cc79a7");
} finally {
  if (previous) Object.defineProperty(globalThis, "localStorage", previous);
  else delete globalThis.localStorage;
}
