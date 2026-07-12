# Phase 6 - Lazy Backend Loading

## Phase Status

- [ ] Not started.

## Depends On

- Phase 5 merged with renderer-neutral presentation, projection, event, and capture contracts.

## Objective

Introduce the backend selection/loading seam without constructing a Babylon engine. Resolve the
exact `rtsRenderer` choice and any experimental runtime dependencies before the app opens a socket
or auto-joins, keeping `START` handling synchronous once networking begins. Preserve a
Babylon-free default static graph and browser resource path.

## Work

- Parse exactly `rtsRenderer=pixi|babylon` at bootstrap. Missing selects Pixi, invalid values fail
  visibly before app/network startup, and the choice is not persisted.
- Add a bootstrap-owned backend resolver. For Pixi it returns synchronously from checked-in code;
  for Babylon it completes module/runtime manifest loading before constructing/starting `App`, so
  no server message queue or async `onStart` path is introduced.
- Tokenize/cancel Babylon preparation across route invalidation or page teardown where applicable.
  A stale completion cannot construct `App`, open a socket, join a room, or attach a canvas.
- Remove the direct renderer-class construction dependency from `Match` and inject a backend
  factory/bundle descriptor. In this phase both normal execution and tests still instantiate Pixi;
  the Babylon factory remains a not-ready descriptor for Phase 6.5.
- Make `App.onStart` construction transactional. Catch factory/module failures locally instead of
  relying on `Net` handler behavior, destroy every partially created match/backend/Lab shell/module/
  listener, restore a bounded lobby/error state, and ensure later messages do not reach a partial
  match. Inject/forward the factory through both `Match` and `ReplayViewer`.
- Add fake-factory failure tests at each construction stage, including replay and Lab composition,
  and assert exactly-once rollback plus visible bounded error reporting.
- Pin and self-host official Babylon core and glTF-loader distributions under one versioned vendor
  directory using the plan-locked highest-stable-patch rule. Vendor/load the minified UMD core then
  minified UMD loader sequentially; record official package/source URL, exact version, package
  integrity, file SHA-256, license/copyright text, served paths, update command/procedure, and
  optional-decoder policy in a machine-readable manifest; do not vendor decoders or unrelated modules.
- Load the experimental runtime asynchronously without `document.write`, cross-site requests, or a
  default-route preload. Extend deploy/static-asset checks so missing or mismatched runtime files
  fail an experimental launch with a bounded error, not a half-started match.
- Prove default absence twice: a static import-graph rule rejects Babylon imports from the default
  bootstrap graph, and a normal Pixi browser resource-timeline test observes no Babylon module,
  vendor file, prefetch, or preload request.
- Add loading contracts for invalid selector, dependency failure, integrity/manifest mismatch,
  cancellation, stale completion, repeated preparation, and the rule that network start occurs
  only after successful experimental resolution.
- Update the durable rendering contract and parity ledger with exact selector, bootstrap order,
  manifest path/version, and default-absence evidence.

## Expected Touch Points

- `client/src/main.js` and a focused bootstrap/backend resolver
- `client/src/launch_url.js` or a renderer-selection parser
- `client/src/app.js` startup injection
- `client/src/match.js`, `client/src/replay_viewer.js`, and Pixi backend factory/adapter
- `client/vendor/babylon/` runtime, license, and manifest files
- deployment/static-asset checks and docs
- `tests/client_contracts/renderer_loading_contracts.mjs` (create it in this phase)
- `tests/browser_renderer_loading.mjs` (create it in this phase)
- durable rendering/client docs and parity ledger
- `plans/render3d/phase-6.md` status update in the implementation commit

## Loading Requirements

- No socket connect, join, auto-launch, or `START` subscription becomes active before selected
  experimental dependencies resolve.
- After app/network start, `START` construction remains synchronous and no message class waits in a
  backend queue.
- Synchronous `START` construction is transactional; every partial failure rolls back exactly once
  and is surfaced before a later message can observe a partial match.
- The default Pixi static graph and browser network path contain no Babylon code or bytes.
- Experimental load failure leaves no app, socket, canvas, listener, or late completion.
- WebGL 2 is required and tested before join; there is no WebGL 1, WebGPU, or mid-launch Pixi fallback.
- Runtime provenance, integrity, license, and update procedure are reproducible from committed data.

## Explicit Exclusions

- No Babylon engine, scene, canvas, camera, render call, GLB load, or Lab screenshot.
- No resource registry, fog, effects, shadows, vegetation, batching, or route unlock.
- No simultaneous renderers, default switch, Pixi removal, or CDN fallback.

## Implementation Checklist

- [ ] Add exact selector and pre-network backend resolution/cancellation.
- [ ] Inject the Pixi factory and remove direct renderer construction from `Match`.
- [ ] Add transactional live/replay/Lab `START` construction and staged rollback tests.
- [ ] Pin/self-host Babylon core/glTF loader with integrity/license/update metadata.
- [ ] Prove default static-graph and browser-timeline absence.
- [ ] Cover failure/stale/repeated loading and synchronous post-start behavior.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/renderer_loading_contracts.mjs
    node tests/browser_renderer_loading.mjs
    node scripts/check-deploy-assets.mjs
    node scripts/check-client-architecture.mjs
    node tests/select-suites.mjs --verify
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Verify an ordinary Pixi route starts normally and requests no Babylon resource. Then test an
explicit Babylon selector with successful preparation, missing runtime, integrity/manifest failure,
invalid selector, and navigation/cancel before completion; no experimental match is expected to
render until Phase 6.5.

## Handoff Expectations

Report exact bootstrap order, selector parser, runtime version/paths/integrity/license, default-
absence evidence, failure/cancellation behavior, and remaining not-ready Babylon descriptor. Name
Phase 6.5 as next and identify engine/scene/canvas ownership, controlled-Lab gate, one-rAF rendering,
partial construction, resize/reset/capture/freeze/destroy, and repeated lifecycle evidence.
