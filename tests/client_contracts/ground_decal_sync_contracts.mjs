import { assert } from "./assertions.mjs";
import { GroundDecalSync } from "../../client/src/match_ground_decal_sync.js";
import { GroundDecalBuffer } from "../../client/src/state_ground_decals.js";
import { GroundDecalLayer } from "../../client/src/renderer/decals.js";
import { TrenchDecalLayer } from "../../client/src/renderer/trenches.js";
import { KIND, msg } from "../../client/src/protocol.js";
import { installFakePixi } from "./pixi_fakes.mjs";

const inlineDecal = (id, overrides = {}) => ({
  id,
  decalClass: "infantry",
  sourceKind: KIND.RIFLEMAN,
  x: 32 + id,
  y: 48,
  owner: 1,
  seed: id,
  ...overrides,
});

assert(
  JSON.stringify(msg.requestGroundDecals(3, 7)) === JSON.stringify({
    t: "requestGroundDecals", requestId: 3, afterRevision: 7,
  }),
  "ground-decal request builder mirrors the reliable wire contract",
);

{
  const requests = [];
  const timers = [];
  const cleared = new Set();
  const buffer = new GroundDecalBuffer();
  const context = { players: [{ id: 1, color: "#4878c8" }], tileSize: 32 };
  const state = {
    groundDecals: buffer,
    applyAuthoritativeGroundDecals: (message) => buffer.applyAuthoritativeBatch(message, context),
    resetAuthoritativeGroundDecals: () => buffer.clear(),
  };
  const sync = new GroundDecalSync({
    net: {
      requestGroundDecals(requestId, afterRevision) {
        requests.push({ requestId, afterRevision });
        return true;
      },
    },
    state,
    retryDelaysMs: [10],
    setTimer(fn, ms) { const id = timers.length; timers.push({ fn, ms }); return id; },
    clearTimer(id) { cleared.add(id); },
  });

  sync.observeSnapshot(2, { afterRevision: 0, decals: [inlineDecal(20)] });
  assert(buffer.authoritativeRevision === 2 && buffer.pendingCount === 1 && requests.length === 0,
    "a contiguous snapshot decal delta advances immediately without repair");
  sync.observeSnapshot(2, { afterRevision: 0, decals: [inlineDecal(20)] });
  assert(buffer.pendingCount === 1,
    "a repeated snapshot decal delta is idempotent by stable decal id");
  sync.observeSnapshot(3, {
    afterRevision: 1,
    decals: [inlineDecal(20), inlineDecal(21)],
  });
  assert(buffer.authoritativeRevision === 3 && buffer.pendingCount === 2,
    "an overlapping covered range advances and queues only the newly learned row");

  sync.observeSnapshot(70, { afterRevision: 6, decals: [inlineDecal(22)] });
  assert(buffer.authoritativeRevision === 3 && buffer.pendingCount === 3,
    "a forward gap presents entitled recent rows without claiming missing cache coverage");
  assert(requests.length === 1 && requests[0].afterRevision === 3,
    "a forward gap falls back to repair from the last complete cursor");
  sync.observeSnapshot(70, { afterRevision: 6, decals: [inlineDecal(22)] });
  assert(requests.length === 1 && buffer.pendingCount === 3,
    "repeated gapped deltas coalesce repair and do not duplicate presentation rows");

  sync.observeSnapshot(70, {
    afterRevision: 3,
    decals: [inlineDecal(22), inlineDecal(23)],
  });
  assert(buffer.authoritativeRevision === 70 && buffer.pendingCount === 4,
    "a later covering delta can satisfy the outstanding repair target");
  assert(sync.outstandingRequestId === null && cleared.has(0),
    "catching up through the fast path cancels the obsolete repair retry");

  sync.reset({ resetPresentation: false });
  sync.observeSnapshot(80, { afterRevision: 70, decals: [inlineDecal(24)] });
  assert(buffer.authoritativeRevision === 0 && buffer.pendingCount === 0,
    "the first queued snapshot after a perspective reset cannot inject old-view inline rows");
  assert(requests.at(-1).afterRevision === 0,
    "perspective replacement still uses correlated repair from zero");
  sync.observeSnapshot(81, { afterRevision: 70, decals: [inlineDecal(25)] });
  assert(buffer.authoritativeRevision === 0 && buffer.pendingCount === 0,
    "all queued old-perspective snapshot tails stay blocked until correlated repair");
  assert(requests.length === 2,
    "additional queued snapshots coalesce behind the perspective repair");
  assert(sync.applyResponse({
    requestId: requests.at(-1).requestId,
    revision: 1,
    decals: [inlineDecal(26)],
  }), "the correlated repair establishes replacement perspective authority");
  assert(buffer.authoritativeRevision === 1 && buffer.pendingCount === 1 &&
    buffer.authoritativeDecals.has(26) && !buffer.authoritativeDecals.has(24) &&
    !buffer.authoritativeDecals.has(25),
  "old-perspective inline rows never enter the replacement cache");
  sync.destroy();
}

{
  const requests = [];
  const buffer = new GroundDecalBuffer();
  const sync = new GroundDecalSync({
    net: {
      requestGroundDecals(requestId, afterRevision) {
        requests.push({ requestId, afterRevision });
        return true;
      },
    },
    state: {
      groundDecals: buffer,
      applyAuthoritativeGroundDecals: (message) => buffer.applyAuthoritativeBatch(message),
      resetAuthoritativeGroundDecals: () => buffer.clear(),
    },
    setTimer() { return 1; },
    clearTimer() {},
  });

  sync.reset();
  sync.observeSnapshot(0);
  assert(requests.length === 1 && requests[0].afterRevision === 0,
    "a zero-revision reset still requests correlated perspective authority");
  sync.observeSnapshot(0);
  assert(sync.outstandingRequestId === requests[0].requestId,
    "repeated zero-revision snapshots cannot cancel the establishing repair");
  sync.observeSnapshot(2, { afterRevision: 0, decals: [inlineDecal(27)] });
  assert(buffer.pendingCount === 0,
    "a later queued old snapshot stays blocked after an initial zero-revision snapshot");
  assert(sync.applyResponse({ requestId: requests[0].requestId, revision: 0, decals: [] }),
    "a zero-revision correlated response settles the replacement perspective");
  sync.destroy();
}

{
  const requests = [];
  const timers = [];
  const cleared = new Set();
  let resets = 0;
  let labResultHandler = null;
  let labUnsubscribed = 0;
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
    net: {
      requestGroundDecals(requestId, afterRevision) {
        requests.push({ requestId, afterRevision });
        return true;
      },
    },
    state,
    labClient: {
      subscribeResult(handler) {
        labResultHandler = handler;
        return () => { labUnsubscribed += 1; };
      },
    },
    resetPresentation: () => { resets += 1; },
    retryDelaysMs: [10, 20],
    setTimer(fn, ms) { const id = timers.length; timers.push({ fn, ms }); return id; },
    clearTimer(id) { cleared.add(id); },
  });

  assert(sync.observeSnapshot(3) && requests[0].requestId === 1 && requests[0].afterRevision === 0,
    "a newer snapshot revision requests authoritative changes after the applied revision");
  assert(!sync.observeSnapshot(3) && requests.length === 1,
    "repeated snapshots coalesce while one revision request is outstanding");
  assert(timers[0].ms === 10, "the first lost response uses the bounded retry schedule");
  timers[0].fn();
  assert(requests[1].requestId === 1 && requests[1].afterRevision === 0,
    "a lost authoritative response retries the same correlated request");

  assert(sync.applyResponse({
    t: "groundDecals",
    requestId: 1,
    revision: 3,
    decals: [
      { id: 10, decalClass: "infantry", sourceKind: KIND.RIFLEMAN, x: 32, y: 48, owner: 1, seed: 5 },
      { id: 10, decalClass: "infantry", sourceKind: KIND.RIFLEMAN, x: 32, y: 48, owner: 1, seed: 5 },
    ],
  }), "a valid authoritative delta advances applied state");
  assert(buffer.authoritativeRevision === 3 && buffer.pendingCount === 1,
    "authoritative application advances only from the response and deduplicates decal ids");
  assert(requests.length === 2,
    "a correlated response supersedes the snapshot target that prompted it");

  sync.observeSnapshot(4);
  assert(requests.at(-1).requestId === 2 && requests.at(-1).afterRevision === 3,
    "a later snapshot starts a new request after the applied revision");

  sync.applyResponse({
    t: "groundDecals",
    requestId: 2,
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
  assert(!sync.applyResponse({ requestId: 2, revision: 2, decals: [] }) && buffer.authoritativeRevision === 3,
    "late responses cannot move the applied revision backward");

  labResultHandler({ ok: true, op: "setVision" });
  assert(buffer.authoritativeRevision === 0 && buffer.pendingCount === 0 && resets === 1,
    "Lab perspective changes clear local decal records, pixels, and applied revision");
  assert(!sync.applyResponse({ requestId: 2, revision: 4, decals: [] }),
    "a response from the old perspective is ignored until its replacement snapshot arrives");
  sync.observeSnapshot(9);
  assert(requests.at(-1).requestId === 3 && requests.at(-1).afterRevision === 0,
    "the first post-reset snapshot requests full history with a fresh id");
  assert(sync.applyResponse({ requestId: 3, revision: 0, decals: [] }),
    "the correlated replacement response can settle below a stale old-perspective snapshot");
  assert(buffer.authoritativeRevision === 0 && sync.targetRevision === 0,
    "an empty replacement perspective does not keep chasing the stale snapshot revision");
  const requestCountAfterEmptyReplacement = requests.length;
  sync.observeSnapshot(0);
  assert(requests.length === requestCountAfterEmptyReplacement,
    "the replacement perspective's zero revision remains fully reconciled");
  assert(!sync.applyResponse({
    requestId: 3,
    revision: 4,
    decals: [{
      id: 99, decalClass: "infantry", sourceKind: KIND.RIFLEMAN,
      x: 16, y: 16, owner: 1, seed: 9,
    }],
  }) && buffer.authoritativeRevision === 0,
  "a delayed duplicate response is rejected after the replacement request settles");
  sync.destroy();
  assert(cleared.size > 0 && labUnsubscribed === 1,
    "destroy cancels outstanding repair work and its Lab result subscription");
}

{
  const restorePixi = installFakePixi();
  try {
    const map = { width: 4, height: 4, tileSize: 32 };
    const layer = new PIXI.Container();
    const decalLayer = new GroundDecalLayer({ layer });
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
    const tank = { id: 40, kind: KIND.TANK, owner: 2, hp: 100, x: 40, y: 80, facing: 0 };
    assert(decalLayer.stampLiveTankTreads([tank]) === 0,
      "the first visible enemy tank pose initializes tread contact without painting");
    assert(decalLayer.stampLiveTankTreads([{ ...tank, x: 48, facing: 0.12 }]) === 1,
      "ordinary fog-filtered poses paint precise live tracks for every visible tank");
    assert(decalLayer.texture.sourceUpdateCount === 1 &&
      decalLayer.diagnostics().tankTreads.tileCount === 1 &&
      layer.children[1].texture.sourceUpdateCount === 1,
    "treads upload one bounded tile without modifying the whole-map permanent decal texture");
    assert(decalLayer.stampBatch([{
      id: 77,
      decalClass: "tankTreads",
      poses: [[160, 320, 0], [192, 320, 1252]],
    }]) === 1 && decalLayer.diagnostics().tankTreads.totalSegments === 1,
    "authoritative history already painted precisely from live poses does not darken twice");
    assert(decalLayer.stampBatch([{
      id: 78,
      decalClass: "tankTreads",
      poses: [[256, 256, 0], [320, 256, 0]],
    }]) === 1 && decalLayer.diagnostics().tankTreads.totalSegments === 2,
    "unseen checkpointed trail chunks still paint through the tiled tread layer");

    const trenchLayer = new TrenchDecalLayer({ layer: new PIXI.Container() });
    trenchLayer.resetForMap(map);
    trenchLayer.drawSnapshot([{ id: 2, x: 64, y: 64, radiusTiles: 0.375 }], { tileSize: 32 });
    assert(trenchLayer.texture.sourceUpdateCount === 1 && trenchLayer.texture.textureUpdateCount === 0,
      "trenches upload dynamic canvas pixels through Pixi v8 TextureSource.update");
  } finally {
    restorePixi();
  }
}
