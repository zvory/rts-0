import { ReplaySeekOverlay } from "../../client/src/replay_seek_overlay.js";
import { assert } from "./assertions.mjs";
import { withFakeOverlayDocument } from "./fakes.mjs";

withFakeOverlayDocument(({ FakeElement }) => {
  const root = new FakeElement("section");
  let stopRequests = 0;
  const overlay = new ReplaySeekOverlay({
    root,
    onStopSeeking: () => { stopRequests += 1; },
  });
  overlay.show("Seeking backward 5 seconds…");
  assert(!overlay.el.hidden && overlay.title.textContent === "Seeking",
    "replay seek overlay presents the centered seeking label");
  assert(overlay.detail.textContent === "Seeking backward 5 seconds…",
    "replay seek overlay preserves authoritative direction and duration detail");
  assert(overlay.stopButton.textContent === "Stop seeking",
    "replay seek overlay presents the stop-seeking action");
  overlay.stopButton.listeners.click();
  assert(stopRequests === 1,
    "stop-seeking action requests an immediate authoritative pause");
  overlay.closeButton.listeners.click();
  assert(overlay.dismissed && overlay.el.hidden,
    "replay seek overlay can be dismissed for the current seek");
  overlay.show("Seeking forward 10 seconds…");
  assert(!overlay.dismissed && !overlay.el.hidden,
    "a later replay seek shows the dismissed overlay again");
  overlay.destroy();
  assert(root.children.length === 0,
    "replay seek overlay removes its generated DOM on destroy");
});
