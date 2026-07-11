# Phase 6 - Lazy Backend Loading and Lifecycle Kernel

## Phase Status

- [ ] Not started.

## Depends On

- Phase 5 merged with renderer-neutral presentation, projection, marquee, event, and capture contracts.

## Objective

Introduce the production backend seam and smallest honest Babylon kernel without expanding graphics
coverage. Preload the explicitly selected experimental factory/dependencies before room join so
`START` stays synchronous and no snapshot/event/control message is lost behind async construction.
Keep normal Pixi startup Babylon-free and keep `Match` as the sole frame/capture owner.

## Work

- Add an app-owned backend resolver selected through namespaced launch configuration. For Babylon
  mode, finish loading the backend module and runtime manifest before sending join/start or enabling
  an auto-launch; after `START`, construct the presentation bundle and install listeners
  synchronously as today. Do not introduce a partial message queue that can lose transient events,
  command receipts, room-time/pause state, or observer diagnostics.
- Tokenize/cancel preload across route change, disconnect, lobby return, repeated join, or destroy.
  Load failure prevents the experimental join and shows a bounded actionable error; stale
  completion cannot start a room or attach a canvas.
- Keep Pixi as the default factory through the same bundle contract. Remove static renderer-class
  imports from `Match` while proving a normal route's static module graph and browser resource
  timeline contain no Babylon renderer/runtime request.
- Pin and self-host every runtime file already known to be needed through the GLB foundation: Babylon
  core plus its glTF loader, their licenses, versions, checksums, served paths, and update procedure.
  Do not include optional decoder assets unless Phase 7 deliberately uses the matching format;
  optional future modules must extend this same manifest/loader rather than add an ad hoc path.
- Load asynchronously without `document.write`, cross-site parser blocking, or default-route
  preload. Record a clear WebGL 2 capability baseline and unsupported/failure presentation.
- Define a presentation bundle covering semantic camera, static map build/reset, resize,
  `render(frame)`, screen marquee, fixed capture/readiness, diagnostics, freeze, and idempotent
  destroy. Engine-specific objects never leave the backend.
- Create one Babylon engine/scene/canvas and fixed elevated perspective adapter satisfying Phase 1.
  Until Phase 9 fog/secrecy merges, enable the runtime backend only in an explicit controlled
  foundation Lab/no-fog scene and admit only ordinary currently visible renderables. Omit
  remembered buildings, `visionOnly` intel, shot/event reveals, effects, and every category whose
  semantics depend on fog layering; do not render them as undifferentiated generic markers.
- Call only `scene.render()` when `Match` invokes an ordinary/fixed frame. Prohibit and test
  `runRenderLoop`, engine-owned rAF, tickers, and secondary clocks.
- Handle dependency/capability/engine initialization failure, partial construction, context loss
  where practical, reset, resize, freeze, capture, destroy, and late completion with bounded
  diagnostics and soft failure. Test replay/vision lifecycle hooks in isolation, but reject normal
  live/replay Babylon launch until Phase 9 explicitly removes that gate.
- Report backend/version, runtime files, canvas/context count, frame/render timing, viewport/DPR,
  basic mesh/instance counts, capture state, and bounded errors. Reset/advance internal per-frame
  counters explicitly rather than labeling cumulative counts as current-frame.
- Prove at least two complete enter/leave cycles. Use `lab-interact` with explicit
  `RTS_CLIENT_DIR=<worktree>/client`, capture the placeholder kernel, and inspect one PNG once.

## Expected Touch Points

- `client/src/app.js`
- `client/src/match.js`
- `client/src/bootstrap.js` and/or `client/src/launch_url.js`
- presentation backend factory/contract modules
- Pixi factory/adapter
- isolated `client/src/renderer_babylon/` kernel
- self-hosted Babylon core/glTF loader and license/version/checksum manifest
- static asset serving/deployment docs as needed
- `tests/client_contracts/renderer_backend_contracts.mjs`
- `tests/client_contracts/renderer_loading_contracts.mjs`
- `tests/browser_renderer_loading.mjs`
- durable rendering/client docs and parity ledger
- `plans/render3d/phase-6.md` status update in the implementation commit

## Lifecycle Requirements

- Experimental runtime preload completes before room join/start; `START` remains synchronous and no
  server message class waits in an incomplete backend queue.
- One presentation bundle, canvas, engine/scene, camera adapter, and active world backend per match.
- Runtime Babylon access is namespaced to the controlled Lab/no-fog foundation scene until Phase 9;
  unsupported live/replay routes fail before join and never receive fog-layered presentation data.
- One rAF owner: `Match`; Babylon owns no ticker/rAF/`runRenderLoop`.
- Reset/resize/freeze/capture/destroy are safe before readiness and after partial failure.
- Destroy invalidates late async completion and is idempotent.
- Default absence has two proofs: static import-graph policy and normal Pixi browser resource timing.

## Explicit Exclusions

- No GLB loading/validation despite vendoring the loader; Phase 7 owns use.
- No resource registry beyond minimal kernel root cleanup; Phase 8 owns shared/child scopes.
- No fog, remembered building, particle, shadow, vegetation, batching, or broad overlay work.
- No simultaneous Pixi/Babylon world rendering, default switch, or Pixi removal.

## Implementation Checklist

- [ ] Preload/cancel the selected experimental factory/runtime before join while keeping `START` synchronous.
- [ ] Prove default static graph and browser timeline load no Babylon code/bytes.
- [ ] Pin/self-host core and glTF loader with license/version/checksum/update metadata.
- [ ] Inject Pixi/Babylon presentation bundles; keep Match as sole rAF/capture owner.
- [ ] Implement bounded Babylon kernel, failure paths, diagnostics, and idempotent lifecycle.
- [ ] Complete two enter/leave cycles and inspect one Lab Interact PNG.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/renderer_backend_contracts.mjs
    node tests/client_contracts/renderer_loading_contracts.mjs
    node tests/browser_renderer_loading.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    node tests/select-suites.mjs --verify
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Verify ordinary Pixi launch/network loading first. Then launch Babylon with
`RTS_CLIENT_DIR=<worktree>/client RTS_ADDR=0.0.0.0:<port> cargo run --release`, test pre-join load
failure/cancel, pan/dolly, shared selection/ground commands, resize, fixed capture, Lab reset,
freeze/back-to-lobby, and two enter/leave cycles in the controlled foundation Lab scene. Confirm
ordinary live/replay Babylon routes are rejected before join and no extra canvas, loop, listener,
context, queued message, or late scene remains.

## Handoff Expectations

Report preload timing relative to join/`START`, runtime manifest/version/checksums, static and browser
default-absence evidence, failure behavior, lifecycle counts, exact preview command/URL, and
inspected artifact. Name Phase 7 as next and identify coordinate/facing round trips, CSS/render
scale boundaries, manifest validation, loader fixtures, and the prohibition on asset-authored
gameplay selection geometry.
