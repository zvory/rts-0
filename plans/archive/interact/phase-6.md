# Phase 6 - Deterministic Frame-Time Capture

Status: done.

## Prerequisite

Do not start merely to improve ordinary screenshot or real-time video quality. Begin only after
Phase 5 evidence identifies concrete repeatability needs and confirms that an injected visual clock
is worth the cross-renderer change.

## Goal

Add a fixed-step CLI capture mode for repeatable animation inspection. Control authoritative
simulation advancement and client visual time, render known frame times, write a bounded PNG
sequence, and encode it without changing normal runtime timing or gameplay semantics.

## Scope

- Inventory client visual-time reads affecting captured output, including renderer rigs, setup
  transitions, tread/frame-strip animation, recoil, shot reveals, command feedback, smoke, muzzle
  flashes, projectiles, impacts, miss toasts, and effect lifetimes.
- Introduce a small injected `RenderClock`/visual-time interface composed by `Match` and passed
  through existing renderer/state view-model seams. Normal mode continues to use monotonic browser
  performance time with unchanged semantics.
- Add a capture clock that freezes and advances by exact milliseconds without globally patching
  `performance.now()` or affecting networking, health measurement, timeouts, daemon idle, or server
  control.
- Add a clearly named `interact lab capture-fixed` command requiring the current opaque
  `sessionId` that:
  - requires an open authoritative session and confirms paused room time;
  - applies requested existing mutations/orders through their normal CLI commands;
  - advances room ticks explicitly at the 30 Hz simulation cadence;
  - advances visual time at a declared bounded output FPS;
  - renders one PNG per requested frame after readiness;
  - records tick/visual-time mapping and relevant event/state evidence;
  - encodes the sequence with FFmpeg and creates Phase 5-compatible contact-sheet/manifests.
- Decide and document interpolation behavior when output FPS differs from simulation tick rate. Do
  not silently mix live requestAnimationFrame interpolation with fixed capture.
- Suspend the ordinary RAF loop only through an explicit `Match` capture seam, then resume or tear
  it down safely while preserving renderer ownership.
- Record repeatability diagnostics: setup/replay identity, seed, start/end ticks, frame index,
  visual timestamp, output FPS, tick mapping, asset versions, and frame hashes.
- Initially support only idle/frame-strip animation, movement, and one direct-fire/recoil sequence.
  Defer uncontrolled effects instead of faking them.
- Keep the daemon responsive only to bounded status/cancellation semantics during capture, prevent
  concurrent scene mutation, and treat valid requests as interaction for idle timing without
  allowing the 30-minute deadline to interrupt an in-flight capture.

## Expected Touch Points

- `client/src/frame_recovery.js`, `client/src/match.js`, and a visual/render clock abstraction
- renderer rig, feedback, and effect modules currently reading `performance.now()` directly
- Interact bridge/driver/daemon and CLI fixed-capture schemas
- Phase 5 encoder/contact-sheet/manifest helpers
- `docs/design/client-ui.md` renderer-loop/timing contract and relevant context capsules
- focused clock, animation, lifecycle, and deterministic media integration tests

## Constraints

- Do not change the server's 30 Hz simulation, command semantics, snapshot protocol, gameplay
  cooldowns, or normal wall-clock behavior.
- Do not replace performance timing globally. Networking, profiler, latency, timeout, and daemon
  watchdogs must remain on real monotonic time.
- Do not advance private simulation state in the browser or fabricate combat events; world state
  still comes from the authoritative Lab room.
- Do not promise cross-browser, cross-GPU, or cross-OS pixel identity. First prove repeatability in
  the pinned local capture environment.
- Do not commit golden PNG/video assets without a later reviewed policy for size, updates, and
  platforms.
- Bound frame count, duration, dimensions, FPS, temporary disk use, encode time, and CLI response.
- Keep all generated paths confined beneath `target/interact/lab/` and clean transient sequences on
  failure, close, shutdown, or idle teardown.

## Verification

- Add pure clock tests proving normal and capture clocks remain isolated and monotonic in their own
  domains.
- Add renderer/client contracts for known frame-strip indices, recoil phases, setup progress, and
  effect lifetimes at explicit visual timestamps.
- Add `Match` lifecycle coverage for entering capture, suspending/resuming RAF, resize/camera
  stability, rematch teardown, and error recovery.
- Run the same short CLI capture twice in the pinned environment and verify frame count,
  tick/timestamp manifest, and frame hashes. Treat cross-environment differences as diagnostics,
  not automatic failures.
- Validate encoded codec, dimensions, FPS, duration, contact sheet, and confined output paths with
  ffprobe and image metadata.
- Test close, shutdown, and a short configured daemon idle interval around capture startup,
  completion, cancellation, and failure without leaking media or child processes.
- Re-run Phase 3 screenshots and Phase 5 real-time recording, plus client architecture, focused
  client contracts, docs health, and suite-selection verification.

## Manual Testing Focus

- Capture the same moving unit twice and compare contact sheets and frame hashes.
- Capture one firing sequence and confirm chosen fixed frames expose recoil, muzzle flash, and
  tracer progression at understandable timestamps.
- Exit deterministic capture and play a normal Lab session to confirm animation, input, health
  reporting, CLI interaction, and teardown all return to real time.

## Handoff

After implementation, mark this phase done and report the visual-clock boundary, migrated/deferred
time reads, simulation-to-frame mapping, supported cases, repeatability evidence, encoded outputs,
daemon lifecycle behavior, and remaining platform variance. State whether the result is suitable
only for agent/human review or merits a separate future visual-regression plan.
