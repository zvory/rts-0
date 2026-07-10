# Phase 6 - Deterministic Frame-Time Capture

Status: planned.

## Prerequisite

Do not start this phase merely to improve ordinary screenshot or real-time-video quality. Begin
only after Phase 5 evidence identifies concrete repeatability needs and confirms that an injected
visual clock is worth the cross-renderer change.

## Goal

Add a fixed-step capture mode for repeatable animation inspection. The driver should control both
authoritative simulation advancement and client visual time, render known frame times, write a
bounded PNG sequence, and encode it without changing normal runtime timing or gameplay semantics.

## Scope

- Inventory every client visual-time read that affects captured output, including renderer rigs,
  setup transitions, tread/frame-strip animation, recoil, shot reveals, command feedback, smoke,
  muzzle flashes, projectiles, impacts, miss toasts, and state visual-effect lifetimes.
- Introduce a small injected `RenderClock`/visual-time interface composed by `Match` and passed
  through existing renderer/state view-model seams. Normal mode must continue to use monotonic
  browser performance time with the same semantics.
- Add an agent capture clock that can freeze and advance by exact milliseconds without globally
  monkey-patching `performance.now()` or affecting networking, health measurement, timeouts, or
  server control.
- Provide a fixed-step driver path that:
  - pauses authoritative lab time and confirms the pause;
  - applies the requested existing lab mutations/orders through normal tools;
  - advances room ticks explicitly at the 30 Hz simulation cadence;
  - advances visual time at a declared output FPS;
  - renders/captures one PNG per requested frame after readiness;
  - records the tick/visual-time mapping and relevant event/state evidence;
  - encodes the sequence with FFmpeg and generates the same contact-sheet/manifest outputs as
    Phase 5.
- Decide and document interpolation policy when output FPS differs from simulation tick rate. Do
  not silently mix live requestAnimationFrame interpolation with fixed capture.
- Temporarily suspend the ordinary RAF loop only through an explicit Match capture API, then resume
  or tear it down safely. Renderer ownership and teardown must remain unchanged.
- Extend media tools with a clearly named deterministic capture operation rather than silently
  changing the semantics of Phase 5 real-time recording.
- Add repeatability diagnostics: scene/replay artifact identity, seed, start/end ticks, frame
  index, visual timestamp, output FPS, tick mapping, asset versions, and hashes of generated frames.
- Define the supported initial deterministic cases narrowly: idle/frame-strip animation, movement,
  and one direct-fire/recoil sequence. Defer effects whose server/client timing cannot yet be
  controlled rather than faking them.

## Expected Touch Points

- `client/src/frame_recovery.js`, `client/src/match.js`, and a new visual/render clock abstraction
- renderer rig, feedback, and visual-effect modules currently reading `performance.now()` directly
- Agent Lab bridge/driver/MCP deterministic capture schemas
- Phase 5 encoder/contact-sheet/manifest helpers
- `docs/design/client-ui.md` renderer-loop and timing contracts
- focused client clock contracts, animation tests, and deterministic media integration tests

## Constraints

- Do not change the server's 30 Hz simulation, command semantics, snapshot protocol, gameplay
  cooldowns, or normal client wall-clock behavior.
- Do not replace all performance timing globally. Network health, profiler, latency, timeouts, and
  process watchdogs must continue using real monotonic time.
- Do not advance private simulation state in the browser or fabricate combat events. All world
  state still comes from the authoritative lab room.
- Do not promise cross-browser, cross-GPU, or cross-operating-system pixel identity. First prove
  repeatability within the pinned local capture environment.
- Do not introduce committed golden PNG/video assets unless a later reviewed testing policy
  defines size, update, and platform behavior.
- Keep frame count, duration, dimensions, FPS, temporary disk use, and encode time bounded.

## Verification

- Add pure clock tests showing normal and capture clocks stay isolated and monotonic in their own
  domains.
- Add renderer/client contracts demonstrating known frame-strip indices, recoil phases, setup
  progress, and effect lifetimes at explicit visual timestamps.
- Add Match lifecycle coverage for entering fixed capture, suspending/resuming RAF, resize/camera
  stability, rematch teardown, and error recovery.
- Run the same short deterministic capture twice in the pinned environment and verify frame count,
  tick/timestamp manifest, and frame hashes match. Treat cross-environment hash differences as
  diagnostics, not automatic failures, until explicitly approved.
- Encode the sequence and validate codec, dimensions, FPS, duration, and contact sheet with
  ffprobe/image metadata.
- Re-run the Phase 3 screenshot and Phase 5 real-time recording smokes.
- Run `node scripts/check-client-architecture.mjs`, focused client contracts, docs health, and suite
  selection verification.

## Manual Testing Focus

- Capture the same moving unit twice and compare contact sheets/frame hashes.
- Capture one firing sequence and confirm the chosen fixed frames expose recoil, muzzle flash, and
  tracer progression at understandable timestamps.
- Exit deterministic capture and play a normal lab session to confirm animation, input, health
  reporting, and teardown still use real time normally.

## Handoff

After implementation, mark this phase done and summarize the visual clock boundary, direct time
reads migrated or deliberately deferred, simulation-to-frame mapping, supported deterministic
cases, repeatability evidence, encode outputs, and remaining platform variance. State whether the
result is suitable only for agent/human review or whether a separate future plan should consider
visual regression testing.
