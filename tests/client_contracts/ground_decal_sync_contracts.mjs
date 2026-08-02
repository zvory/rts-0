import { assert } from "./assertions.mjs";
import { GroundDecalSync } from "../../client/src/match_ground_decal_sync.js";
import { GroundDecalBuffer } from "../../client/src/state_ground_decals.js";
import { GroundDecalLayer } from "../../client/src/renderer/decals.js";
import { TrenchDecalLayer } from "../../client/src/renderer/trenches.js";
import { KIND, msg } from "../../client/src/protocol.js";
import { installFakePixi } from "./pixi_fakes.mjs";

assert(
  JSON.stringify(msg.requestGroundDecals(7)) === JSON.stringify({ t: "requestGroundDecals", afterRevision: 7 }),
  "ground-decal request builder mirrors the reliable wire contract",
);

{
  const requests = [];
  const timers = [];
  const cleared = new Set();
  let resets = 0;
  const buffer = new GroundDecalBuffer();
  const state = {
    groundDecals: buffer,
    applyAuthoritativeGroundDecals: (message) => buffer.applyAuthoritativeBatch(message, {
      players: [{ id: 1, color: "#4878c8" }],
      tileSize: 32,
    }),
    resetAuthoritativeGroundDecals: () => buffer.clear(),
  };
  const sync = new GroundDecalSync({
    net: { requestGroundDecals(afterRevision) { requests.push(afterRevision); return true; } },
    state,
    resetPresentation: () => { resets += 1; },
    retryDelaysMs: [10, 20],
    setTimer(fn, ms) { const id = timers.length; timers.push({ fn, ms }); return id; },
    clearTimer(id) { cleared.add(id); },
  });

  assert(sync.observeSnapshot(3) && requests.join(",") === "0",
    "a newer snapshot revision requests authoritative changes after the applied revision");
  assert(!sync.observeSnapshot(3) && requests.length === 1,
    "repeated snapshots coalesce while one revision request is outstanding");
  assert(timers[0].ms === 10, "the first lost response uses the bounded retry schedule");
  timers[0].fn();
  assert(requests.join(",") === "0,0", "a lost authoritative response retries from the same applied revision");

  assert(sync.applyResponse({
    t: "groundDecals",
    revision: 2,
    decals: [
      { id: 10, decalClass: "infantry", sourceKind: KIND.RIFLEMAN, x: 32, y: 48, owner: 1, seed: 5 },
      { id: 10, decalClass: "infantry", sourceKind: KIND.RIFLEMAN, x: 32, y: 48, owner: 1, seed: 5 },
    ],
  }), "a valid authoritative delta advances applied state");
  assert(buffer.authoritativeRevision === 2 && buffer.pendingCount === 1,
    "authoritative application advances only from the response and deduplicates decal ids");
  assert(requests.at(-1) === 2, "a partial response immediately requests the remainder from its revision");

  sync.applyResponse({
    t: "groundDecals",
    revision: 3,
    decals: [
      { id: 10, decalClass: "infantry", sourceKind: KIND.RIFLEMAN, x: 32, y: 48, owner: 1, seed: 5 },
      { id: 11, decalClass: "mortarBlast", sourceKind: KIND.MORTAR_TEAM, x: 64, y: 80, owner: 0, seed: 6, radiusTiles: 1.5 },
    ],
  });
  assert(buffer.authoritativeRevision === 3 && buffer.peekPending().length === 2,
    "overlapping server deltas stamp each authoritative mark once");
  buffer.consumePending();
  assert(buffer.requeueAuthoritative() === 2 && buffer.peekPending().length === 2,
    "normalized authoritative records remain available to repaint a replacement renderer generation");
  assert(!sync.applyResponse({ revision: 2, decals: [] }) && buffer.authoritativeRevision === 3,
    "late responses cannot move the applied revision backward");

  sync.reset({ awaitSnapshot: true });
  assert(buffer.authoritativeRevision === 0 && buffer.pendingCount === 0 && resets === 1,
    "perspective reset clears local decal records, pixels, and applied revision");
  assert(!sync.applyResponse({ revision: 4, decals: [] }),
    "a response from the old perspective is ignored until its replacement snapshot arrives");
  sync.observeSnapshot(4);
  assert(requests.at(-1) === 0, "the first replacement-perspective snapshot requests full history");
  sync.destroy();
  assert(cleared.size > 0, "destroy cancels outstanding repair work");
}

{
  const restorePixi = installFakePixi();
  try {
    const map = { width: 4, height: 4, tileSize: 32 };
    const decalLayer = new GroundDecalLayer({ layer: new PIXI.Container() });
    decalLayer.resetForMap(map);
    decalLayer.assetStatus = "ready";
    decalLayer.stampBatch([{
      id: 1,
      decalClass: "infantry",
      kind: KIND.RIFLEMAN,
      x: 32,
      y: 32,
      owner: 1,
      color: "#4878c8",
      seed: 1,
      facing: 0,
    }]);
    assert(decalLayer.texture.sourceUpdateCount === 1 && decalLayer.texture.textureUpdateCount === 0,
      "ground decals upload dynamic canvas pixels through Pixi v8 TextureSource.update");

    const trenchLayer = new TrenchDecalLayer({ layer: new PIXI.Container() });
    trenchLayer.resetForMap(map);
    trenchLayer.drawSnapshot([{ id: 2, x: 64, y: 64, radiusTiles: 0.375 }], { tileSize: 32 });
    assert(trenchLayer.texture.sourceUpdateCount === 1 && trenchLayer.texture.textureUpdateCount === 0,
      "trenches upload dynamic canvas pixels through Pixi v8 TextureSource.update");
  } finally {
    restorePixi();
  }
}
