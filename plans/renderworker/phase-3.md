# Phase 3 - Single-Path Worker Cutover and Beta Handoff

## Phase Status

- [x] Done locally. The user explicitly omitted PR, merge, beta deployment, and `/version` gates;
  all Phase 3 implementation and local verification gates passed on the Phase 2 commit lineage.

## Objective

Make a module worker owning an `OffscreenCanvas` and the existing Pixi/WebGL renderer the only Pixi
implementation. Connect it to the asynchronous presentation lifecycle and worker-safe data/assets,
remove main-thread Pixi construction and direct renderer access, prove correctness and the measured
performance win, then verify the merged commit has deployed to beta for the user's playtest.

## Entry Gate

- Phase 2 is merged and its head is reachable from `origin/main`.
- Presentation and worker-message versions, frame ordering, durable decal revision, plain projection
  reconstruction, asset readiness, and Map Editor records are documented in the handoff.
- Phase 2 exact parity and all worker-safe asset checks pass.

## Atomic Worker Cutover

- Create one module worker per Pixi renderer lifecycle and transfer the renderer host's canvas with
  `transferControlToOffscreen()` during construction. The worker imports the same pinned Pixi 8.19
  ESM build, exposes only the environment adapter required by the existing renderer, and explicitly
  initializes Pixi with `preference: "webgl"`.
- Do not initialize, probe, or silently select WebGPU. Add a focused assertion that the resulting
  backend is WebGL and that the worker bundle contains no WebGPU preference or fallback path.
- Disable or omit Pixi DOM event, accessibility, resize-observer, and other browser-main-thread
  systems that the worker does not own. Main-thread input and resize owners send only the Phase 2
  detached control/data messages.
- Replace the default Pixi backend bundle with the worker host. Normal live play, replay, spectator,
  Lab, fixed capture, stress tests, and Map Editor all use it; retain no direct `Renderer.create`,
  main-thread `PIXI.Application`, hidden canvas, or query-selected sync path.
- Keep existing explicit Babylon selection behavior outside the worker host. A failed Pixi worker
  must not switch to Babylon or another canvas renderer.
- Delete obsolete main-thread Pixi adapter/construction code and update the architecture checker so
  future code cannot import the worker-owned renderer or Pixi runtime from a main-thread client
  area. Tests may instantiate narrow pure renderer helpers, but production code has one Pixi owner.

## Scheduling and Correctness

- Use one in-flight frame and one latest pending frame. When a newer frame replaces the pending
  frame, report the replaced id as `superseded`; never present its selection scene or count it as
  displayed, while separately retained durable decal updates remain safe.
- Keep the queue bounded so a slow worker cannot create seconds of visual latency or unbounded
  memory. Measure submitted, retained, presented, superseded, failed, queue age, display age, cloned
  bytes, main submission time, worker update time, worker present time, and worker long frames.
- Apply only monotonically valid acknowledgments for the current generation. Match the presented
  frame id to its stored main-thread `SelectionSceneV1`, and discard all pending metadata on reset or
  destroy.
- Fixed capture submits its exact frame, waits for that frame's `presented` acknowledgment, then
  reads the canvas. Capture must not be superseded by live work; entering capture drains or cancels
  ordinary pending work, and exiting capture resumes exactly one Match RAF.
- A worker startup, asset, protocol, render, or context failure is a visible bounded fatal renderer
  error for that match. It must settle pending promises and tear down listeners/resources, but it
  must not construct a replacement renderer.
- Resize messages carry CSS dimensions and DPR, are ordered with frame ids/generation, and cannot
  let a pre-resize frame overwrite a post-resize canvas. Destroy terminates the worker idempotently
  and prevents late messages from affecting the next match or editor session.

## Performance and Parity Evidence

- Extend the canonical harness itself rather than adding a worker-specific workload. The workload
  id remains exactly `supply-300-hellhole-stream`; there is no `-worker` fixture or URL parameter.
- Profile the main page and render worker independently because page CPU profiles do not include
  worker CPU. Emit both ranked summaries/flame graphs and include worker timing plus queue outcomes
  in the ordinary summary.
- Compare current `origin/main` and the candidate on the same host, viewport, DPR, duration, browser,
  and unchanged stream. Report actual completed presentations per second, not main-thread RAF calls
  or submitted frames.
- Require at least a 50% reduction in main-thread `frame.work` P95, completed presentation rate no
  worse than baseline, zero failed frames, and no worse display age. Superseded frames remain an
  explicit caveat and pass only when completed cadence and display age still meet the baseline; do
  not weaken fidelity or timing to reduce that count.
- Run at least 16 randomly selected deterministic parity ticks with identical ready assets, state,
  camera, viewport, DPR, and visual clock. Exact decoded pixels are required, and a repeated/stale
  candidate image, missing decal, pending texture, or capture timeout is a failure.
- Do not use page-only CPU throttling as whole-system worker evidence. If worker-inclusive throttling
  is unavailable, state that limitation and rely on the same-host unthrottled comparison plus the
  independent worker CPU profile.

## Beta Delivery

- Run focused local checks and the repository PR helper, arm auto-merge, and wait for the Phase 3 PR
  to merge through `Main test gate`. Fetch `origin/main` and verify the exact phase head is reachable
  before treating the implementation as complete.
- Wait for the automatic beta deployment triggered by the successful tested-main workflow. Verify
  beta `/version` matches the merged Phase 3 head and inspect startup plus bounded worker/client
  errors; do not deploy mainline.
- Hand the beta URL and exact commit to the user with the local performance/parity table and core
  playtest focus. The user owns the decision to promote that commit or revert Phase 3, Phase 2, and
  Phase 1 in reverse order.
- Do not add a flag, fallback, emergency selector, or second renderer during beta preparation. Git
  revert is the rollback mechanism requested by the user.

## Expected Touch Points

- worker host, worker entry, environment adapter, and wire modules under `client/src/renderer/`
- `client/src/renderer/backend_bundle.js` and architecture allowlists/checks
- `client/src/renderer/index.js` plus existing Pixi presentation adapter code moved behind the worker
  boundary or removed where obsolete
- `client/src/frame_recovery.js`, `client/src/match_fixed_capture.js`, and renderer health/profiling
- resize, canvas host, Match teardown, replay/spectator/Lab construction, and Map Editor integration
- `scripts/client-perf-harness.mjs`
- `scripts/client-flamegraph.mjs`
- `scripts/client-render-parity.mjs`
- focused worker lifecycle/wire, selection, decal, capture, resize, route, Map Editor, and teardown
  browser contracts
- `docs/design/client-rendering.md`
- `docs/design/client-ui.md`
- `docs/design/rendering-parity.md`
- `docs/design/client-stress-tests.md` if the persisted report schema changes
- `docs/perf-tracing.md` if worker fields become durable diagnostics

## Verification

- Focused worker construction, WebGL-only selection, message ordering, queue supersession, durable
  decal, stale acknowledgment, resize, generation reset, fatal error, and idempotent teardown tests.
- Focused live, replay, spectator, Lab, fixed-capture, stress-test, and Map Editor browser checks with
  page/console/request/worker/asset error collection.
- Existing presentation, projection, selection, Pixi renderer, ground-decal, rig/texture, fog,
  trench, feedback, observer, replay, Lab, Map Editor, and frame-profiler contracts.
- `node scripts/client-render-parity.mjs --baseline-worktree <phase-2-baseline> --candidate-worktree <phase-worktree> --samples 16 --seed renderworker-phase-3`
- `node scripts/client-flamegraph.mjs --preview`
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`
- `scripts/agent-pr.sh --verification "<focused worker, browser, exact parity, architecture, and canonical Hellhole checks passed>"`
- `scripts/wait-pr.sh <pr>`

## Manual Test Focus

Before merge, play a local normal match through camera movement/zoom, click and marquee selection,
orders, building, production, combat effects, deaths/decals, fog transitions, resize, and
leave/re-enter. Exercise replay seek/vision, spectator join, Lab reset/seek/capture, and Map Editor
paint/symmetry/base editing, then inspect early/middle/late Hellhole captures for full visual
readiness.

For the beta handoff, ask the user to focus on input feel during large fights, selection lining up
with visible units, camera responsiveness, ground marks/effects, resize/fullscreen behavior, and
leave/re-enter. This is the core playtest list, not a request for the user to execute the automated
matrix.

## Completion and Handoff Expectations

Mark this phase done in its implementation commit so `scripts/agent-pr.sh` archives the completed
plan in the final PR. Lead the final handoff with the outcome and beta URL, then give the exact
deployed commit, before/after main-thread and actual-presentation measurements, worker queue/timing
numbers, 16-tick parity result, worker/main flame-graph locations, focused checks, and remaining
caveats. State plainly that Pixi now has one worker path with WebGL, no flag, no fallback, and no
main-thread duplicate, and stop without promoting mainline.
