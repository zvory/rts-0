# Phase 6.5 - Babylon Lifecycle Kernel

## Phase Status

- [ ] Not started.

## Depends On

- Phase 6 merged with pre-network loading, injected Pixi factory, and pinned runtime manifest.

## Objective

Create the smallest honest Babylon backend in an explicitly selected authoritative Lab route.
Keep `Match` as the sole ordinary and fixed-capture frame owner while proving engine/scene/canvas
lifecycle, failure, and teardown. Do not broaden state categories or normal routes before fog and
interaction gates merge.

## Work

- Replace the Phase 6 not-ready descriptor with a presentation bundle implementing semantic camera,
  static map build/reset, resize, `render(frame)`, screen marquee hook, fixed-capture readiness,
  diagnostics, freeze, and idempotent destroy.
- Construct exactly one engine, scene, canvas, and fixed elevated perspective adapter per match.
  Engine-specific objects never leave the backend or enter shared presentation/input models.
- Permit Babylon only on `/lab` with explicit `rtsRenderer=babylon`. Reject normal live, replay,
  spectator, non-Lab dev, and invalid/no-fog initialization before join; Phase 10.5 owns the later
  route unlock.
- Render a bounded kernel: map boundary/ground plus a deliberately small controlled set of ordinary
  currently received visible placeholder records sufficient to prove camera and lifecycle. Omit
  remembered buildings, `visionOnly` intel, shot/event reveals, transient effects, and every
  category whose semantics depend on fog layering.
- Call only `scene.render()` when `Match` invokes an ordinary or fixed frame. Prohibit and test
  `runRenderLoop`, engine-owned rAF, tickers, recurring timers, and secondary visual clocks.
- Handle unsupported WebGL capability, dependency access, partial engine/scene construction,
  deterministic simulated context loss, reset, resize, freeze, fixed capture, destroy, and late completion
  through bounded diagnostics and one idempotent cleanup path.
- Exercise real context loss through `WEBGL_lose_context` when the browser exposes it and record
  `extension-unavailable` otherwise; the simulated cleanup contract remains mandatory.
- Report backend/runtime version, canvas/context count, frame/render timing, viewport/DPR, basic
  mesh/instance counts, capture state, and bounded errors. Label cumulative counters accurately;
  Phase 11 owns production per-frame counter semantics.
- Prove at least two complete enter/leave cycles with canvas, context, rAF, listener, and pending
  work returning to baseline.
- Update Lab Interact backend selection if Phase 5 did not already cover it, then use the
  `lab-interact` skill with explicit `RTS_CLIENT_DIR=<worktree>/client`, capture the controlled
  kernel, and inspect one PNG once.

## Expected Touch Points

- presentation backend factory/contract modules from Phase 6
- isolated `client/src/renderer_babylon/` kernel
- `client/src/match.js` only through the injected bundle seam
- Lab launch/backend selection and capture readiness
- `tests/client_contracts/renderer_backend_contracts.mjs` (create it in this phase)
- `tests/client_contracts/babylon_lifecycle_contracts.mjs` (create it in this phase)
- browser lifecycle/capture smoke coverage
- `tests/browser_babylon_lifecycle.mjs` wired into the authoritative browser runner
- durable rendering/client docs and parity ledger
- `plans/render3d/phase-6.5.md` status update in the implementation commit

## Lifecycle Requirements

- One presentation bundle, canvas, engine/scene, camera adapter, and active world backend per match.
- One rAF owner: `Match`; Babylon owns no ticker/rAF/`runRenderLoop`.
- Reset/resize/freeze/capture/destroy are safe before readiness and after partial failure.
- Destroy invalidates late work and is idempotent.
- Controlled route rejection happens before join and cannot receive an unsupported presentation stream.

## Explicit Exclusions

- No GLB loading despite the pinned loader; Phase 7 owns first use.
- No shared/child resource registry; Phase 8 owns it.
- No production fog, memory, reveal, particle, shadow, vegetation, batching, broad overlay, or route unlock.

## Implementation Checklist

- [ ] Implement the controlled-Lab Babylon presentation bundle and fixed perspective adapter.
- [ ] Keep Match as the sole ordinary/fixed frame owner and prohibit secondary loops.
- [ ] Cover capability/partial-failure/reset/resize/freeze/capture/destroy lifecycle.
- [ ] Prove two enter/leave cycles return owned counts to baseline.
- [ ] Capture and inspect one controlled Lab Interact PNG.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/renderer_backend_contracts.mjs
    node tests/client_contracts/babylon_lifecycle_contracts.mjs
    node tests/client_contracts/renderer_loading_contracts.mjs
    node tests/browser_babylon_lifecycle.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Launch the controlled Babylon Lab route with
`RTS_CLIENT_DIR=<worktree>/client RTS_ADDR=0.0.0.0:<port> cargo run --release`. Test pan/dolly,
resize, fixed capture, Lab reset, freeze/back-to-lobby, partial failure, and two enter/leave cycles;
confirm normal live/replay/spectator routes reject before join and no extra canvas, loop, listener,
context, queued message, or late scene remains.

## Handoff Expectations

Report controlled-route predicate, bundle hooks, engine/scene/canvas ownership, one-rAF proof,
failure behavior, lifecycle counts, exact preview command/URL, and inspected PNG. Name Phase 7 as
next and identify coordinate/facing round trips, CSS/render scale boundaries, manifest validation,
loader fixtures, and the prohibition on asset-authored gameplay selection geometry.
