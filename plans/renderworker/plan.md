# Pixi Render Worker Cutover

## Goal

Move the existing Pixi/WebGL world renderer off the browser's main thread and make the worker-backed
renderer the only Pixi path. Preserve the same authoritative presentation data, pixels, visual time,
input meaning, asset readiness, and player-visible cadence while freeing the main thread for input,
networking, frame assembly, HUD, and minimap work. Ship the completed cutover directly through the
normal merged-PR path to beta; the user will playtest that beta build and will either promote that
exact commit or revert this plan's PRs.

This is not a renderer rewrite. The worker continues to run the existing Pixi 8.19 renderer with
WebGL, and the work stops once the asynchronous contract, worker-safe boundary, single-path cutover,
and beta handoff are complete.

## Planning Evidence

The canonical `supply-300-hellhole-stream` profile was refreshed on `origin/main` commit
`ec295c5d` on 2026-07-20 with the standard 15-second, 1440x900, DPR 1 workload. It presented 1,225
frames (81.7 frames/second), with `frame.work` averaging 11.8 ms at 16 ms P95 and `match.renderer`
averaging 9.3 ms at 12 ms P95; renderer update and present averaged 5.3 ms and 4.0 ms respectively.
The CPU profile is dominated by Pixi collection/packing plus the existing per-frame renderer work,
which is work a render worker can actually remove from the main thread.

The disposable worker prototype on the immediately preceding base showed the opportunity and the
correctness traps. It reduced main-thread frame work from 13.2 ms average / 24 ms P95 to 2.6 ms
average / 4 ms P95 and completed about 92.5 worker presentations per second versus 73.5 synchronous
presentations per second, but it superseded 25.9% of submitted frames. Its deterministic captures
read stale pixels because capture did not await the requested worker frame, selection could advance
ahead of visible pixels, decal acknowledgment was still coupled to synchronous presentation, and
worker SVG decoding left the authored ground-decal atlas unavailable. Those are requirements for
this plan, not acceptable beta caveats.

## Overall Constraints

- Do not add a query parameter, environment variable, startup switch, runtime flag, capability
  fallback, or retained main-thread Pixi renderer. Phase 3 replaces the Pixi path atomically and
  deletes the obsolete main-thread construction path in the same PR.
- Do not add a fallback period. A browser that cannot create the required module worker,
  `OffscreenCanvas`, or WebGL context gets one clear startup error instead of silently selecting a
  different Pixi implementation.
- Force Pixi to use WebGL. Do not initialize, probe, import, or add an option for WebGPU.
- Existing explicit Babylon work is outside this plan. It is a separate renderer backend, not a
  fallback for a failed Pixi worker, and this plan must not broaden or redesign it.
- Keep the server-authoritative simulation, fog filtering, Match-owned animation-frame loop,
  semantic camera, main-thread input, HUD, minimap, audio, and detached presentation boundary.
- The worker owns the transferred canvas, Pixi application, Pixi scene graph, textures, asset
  readiness, and WebGL calls. Main-thread code must not retain a Pixi display object, WebGL context,
  or direct escape hatch into the worker renderer.
- A submitted frame is not a displayed frame. Selection publication, displayed-frame counters, and
  fixed capture complete only from the matching worker presentation acknowledgment.
- Persistent ground decals are durable updates. A frame may be superseded without losing a decal,
  and the game-state queue is acknowledged only through the matching durable worker receipt.
- Keep scheduling bounded and latest-oriented under overload, but never call a superseded frame
  presented or use submitted-frame rate as performance evidence. Selection for a superseded frame
  is discarded, and every superseded/failed frame remains visible in bounded diagnostics.
- Send static map data once per generation and send revisioned large data only when its revision
  changes. Start with the existing detached per-frame records rather than designing a generalized
  delta protocol or shared-memory transport.
- All Pixi-backed surfaces must use the one worker path at cutover: normal live play, Lab, replay,
  spectator, fixed capture, stress tests, and the Map Editor. The Map Editor must stop constructing
  Pixi objects directly and send a small detached editor presentation instead.
- Preserve exact state, timing, viewport, DPR, and pixels. Do not claim a win by lowering
  reconciliation frequency, reducing visual cadence, presenting stale state, culling the canonical
  whole-map workload, or moving unmeasured work outside the main-frame summary.
- Keep raw profiles, screenshots, parity output, and benchmark artifacts under ignored `target/`
  paths. Commit only durable code, tests, generated worker-safe assets where required, and updated
  design documentation.

## Phase Summaries

### [Phase 1 - Asynchronous Presentation Contract](phase-1.md)

Replace the synchronous `render(frame) -> presented` assumption with explicit submitted, retained,
presented, superseded, failed, and destroyed outcomes while the ordinary main-thread Pixi renderer
still supplies immediate outcomes. Tie selection publication, ground-decal queue acknowledgment,
frame diagnostics, teardown, and fixed capture to the correct outcome and frame id. Finish with the
same pixels and ordinary behavior so the risky timing contract is proven before a worker exists.

### [Phase 2 - Worker-Safe Presentation and Assets](phase-2.md)

Make every Pixi input structurally cloneable by replacing projection functions with plain camera
data, separating static/revisioned payloads, and defining the small initialization, frame, decal,
resize, capture, and teardown message vocabulary. Convert the authored SVG decal masks into a
checked-in worker-decodable raster atlas while retaining SVG as source art, and make all other Pixi
texture paths work without DOM `Image` or `document` dependencies. Remove the Map Editor's direct
Pixi access by expressing its terrain and overlay as detached worker-ready records, without starting
a production worker or creating a second runtime renderer path in this phase.

### [Phase 3 - Single-Path Worker Cutover and Beta Handoff](phase-3.md)

Transfer the canvas, run the existing Pixi renderer in a module worker with WebGL, connect it to the
Phase 1 lifecycle, and delete the main-thread Pixi construction path in the same change. Prove exact
deterministic pixels, matching selection and decal behavior, honest presentation cadence, resize,
capture, Map Editor, replay/spectator/Lab, teardown, and worker profiling on the sole
`supply-300-hellhole-stream` benchmark. Merge through the normal gate, verify the tested phase head
is the beta deployment, and hand that exact beta build to the user for the promote-or-revert
decision.

## Success Gate

The final beta candidate is ready for the user's playtest only when all of these are true:

- At least 16 randomly selected deterministic Hellhole ticks compare exactly in decoded RGBA with
  identical state, camera, viewport, DPR, visual time, and ready assets.
- Ground decals, fog, trenches, rigs, effects, observer overlays, and other current Pixi assets are
  ready, with no missing or failed asset suppressed from the parity result.
- Selection and command targeting use the scene matching the last acknowledged visible frame.
- Fixed capture waits for its requested frame id, and repeated captures are neither stale nor
  duplicated.
- The canonical workload reports actual worker-completed presentations, superseded/failed counts,
  main submission/clone cost, queue age, worker update/present cost, and worker CPU profile.
- Main-thread `frame.work` P95 improves by at least 50% from the same-host current-main baseline,
  actual completed presentation rate is no worse than the baseline, and the run has zero failed
  frames. Superseded frames are reported honestly and are acceptable only when display age and
  actual presentation rate both beat or match the baseline; they are never themselves counted as a
  performance win.
- A normal live match, replay, spectator view, Lab session, and Map Editor session start, resize,
  render, and tear down without page, console, request, worker, asset, or WebGL errors.
- The beta `/version` commit matches the merged Phase 3 head after the successful `Main test gate`.

## Deployment and Rollback

There is no software switch and no automatic renderer fallback. Successful PR merges to `main`
flow through the repository's automatic tested-main beta deployment; Phase 3 stops after verifying
that beta is running the exact merged head and providing the playtest handoff. The implementing
agent must not deploy or promote mainline as part of this plan.

After the user's beta playtest, promotion means deploying the exact accepted Phase 3 commit to
mainline. Rejection means the user reverts Phase 3, Phase 2, and Phase 1 in reverse order and lets
the tested reverted head redeploy to beta; do not prepare a hidden alternate path in anticipation of
that decision.

## Phase Process and Handoffs

Implement each phase from current `origin/main` in its own clean `zvorygin/` branch and owned PR.
Arm auto-merge, wait for a definite merge, fetch `origin/main`, and verify the phase head is
reachable from `origin/main` before reporting completion or starting the next phase. Mark the phase
document done in that phase's implementation commit.

After every phase, provide a handoff that states what changed, the exact contracts introduced or
removed, focused verification and retained evidence, what the next agent should do, and the core
features that should be manually tested. The manual test list should cover the phase's central
player-facing behavior rather than an exhaustive matrix.

Run phases serially:

```bash
scripts/phase-runner.sh --plan renderworker phase-1 --pr --wait
scripts/phase-runner.sh --plan renderworker phase-2 --pr --wait
scripts/phase-runner.sh --plan renderworker phase-3 --pr --wait
```

Do not run the three phases as one unattended range: review each merged handoff before starting the
next because Phase 1 changes the presentation contract and Phase 2 changes the detached frame
boundary that Phase 3 consumes.

## Explicitly Deferred

- WebGPU, `SharedArrayBuffer`, `Atomics`, shared-memory ring buffers, generalized render deltas, or a
  custom binary frame codec.
- New visual design, renderer fidelity changes, rig redesign, unrelated Pixi optimization, supply
  cap changes, or a second stress fixture.
- Device certification or a large browser/device matrix beyond the browser targets already
  supported by the beta client.
- Mainline promotion, a fallback period, a runtime rollback switch, or maintenance of both worker
  and main-thread Pixi paths.
