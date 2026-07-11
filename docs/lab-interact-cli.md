# Lab Interact CLI

Lab Interact is a project-local command-line tool for arranging and inspecting small authoritative
Lab scenes through a machine-readable local interface. It starts this worktree's normal Rust
server and a headless Pixi client. Mutations are ephemeral and never edit source files.

## Commands

Run commands from the worktree root. The optional second argument must be one JSON object:

```bash
node scripts/lab-interact/cli.mjs open '{"viewport":{"width":1000,"height":700,"deviceScaleFactor":1}}'
node scripts/lab-interact/cli.mjs catalog '{"sessionId":"<id>","categories":["players","units","commands"]}'
node scripts/lab-interact/cli.mjs spawn '{"sessionId":"<id>","spawns":[{"owner":1,"kind":"rifleman","x":960,"y":960,"alias":"subject"}]}'
node scripts/lab-interact/cli.mjs inspect '{"sessionId":"<id>","refs":["subject"]}'
node scripts/lab-interact/cli.mjs camera '{"sessionId":"<id>","camera":{"action":"focus","refs":["subject"]}}'
node scripts/lab-interact/cli.mjs screenshot '{"sessionId":"<id>","name":"subject","presentation":"clean","subjects":["subject"]}'
node scripts/lab-interact/cli.mjs record-start '{"sessionId":"<id>","name":"motion","maxDurationMs":10000}'
node scripts/lab-interact/cli.mjs order '{"sessionId":"<id>","playerId":1,"command":{"c":"move","units":["subject"],"x":1100,"y":960}}'
node scripts/lab-interact/cli.mjs record-stop '{"sessionId":"<id>"}'
node scripts/lab-interact/cli.mjs capture-fixed '{"sessionId":"<id>","name":"motion-fixed","fps":30,"frameCount":60}'
node scripts/lab-interact/cli.mjs capture-cancel '{"sessionId":"<id>"}'
node scripts/lab-interact/cli.mjs export '{"sessionId":"<id>","kind":"setup","name":"two-unit-scene","reproduction":true}'
node scripts/lab-interact/cli.mjs artifact-inspect '{"sessionId":"<id>","artifactId":"<artifact-id>"}'
node scripts/lab-interact/cli.mjs import '{"sessionId":"<id>","kind":"setup","artifactId":"<artifact-id>"}'
node scripts/lab-interact/cli.mjs close '{"sessionId":"<id>"}'
node scripts/lab-interact/cli.mjs shutdown
node scripts/lab-interact/cli.mjs --help
```

The complete surface is `open`, `close`, `reset`, `catalog`, `spawn`, `update`, `remove`, `order`,
`time`, `inspect`, `camera`, `screenshot`, `record-start`, `record-stop`, `export`, `import`,
`artifact-inspect`, `capture-fixed`, `capture-cancel`, `status`, and `shutdown`. Success writes exactly one JSON
envelope to stdout. Failure writes a concise JSON error to stderr and exits nonzero. Every command
has an exact, bounded input shape; arbitrary state patches, protocol messages, browser evaluation,
and caller-selected artifact paths are not accepted.

`open` returns the `sessionId` required by session commands. It is idempotent: repeated or
concurrent calls return the one active session instead of starting another browser. Run `close`
before `open` when a fresh session or different launch options are required. Optional aliases match
`[A-Za-z][A-Za-z0-9_-]{0,31}` and remain private to that session. Unknown, duplicate, stale, or
cross-session aliases are rejected rather than guessed. Only one authoritative session may be open
per worktree.

A cold first `open` may spend tens of seconds building the selected worktree's Rust server before
it writes its single JSON response. Keep that CLI process attached until it exits. A concurrent
`status` reports `opening: true` while startup is still in progress, and an idempotent `open` retry
recovers the same completed session if the original caller was interrupted. `--help`, `-h`, and
`help` return the bounded command catalog without requiring a Git worktree or starting a daemon.

## Automatic daemon lifecycle

The first command starts a background daemon automatically. It is isolated by the real worktree
path and communicates over a mode-0600 Unix socket in a mode-0700 temporary runtime directory. A
versioned daemon identity and random capability in its mode-0600 state file must match every
request. This prevents a stale or unrelated local listener from being mistaken for the selected
worktree's daemon.

The daemon preserves its browser, private Rust server, aliases, and authoritative session across
CLI processes. Each accepted interaction resets a 30-minute idle deadline. An in-flight command
cannot expire. Idle expiry or `shutdown` closes the driver, browser, and Rust server, removes its
socket/state/runtime files, and exits. `RTS_LAB_INTERACT_IDLE_MS` is a bounded test-only override;
normal use should leave it unset.

## Capture workflow

Query `catalog` before selecting owners, entity kinds, upgrades, abilities, or commands. Keep scenes
small, confirm mutations with `inspect`, control authoritative time with `time`, and compose with
`camera`. `screenshot` waits for fonts, relevant assets, two error-free render frames, and
authoritative state. It returns the absolute PNG and adjacent manifest paths plus bounded metadata;
it never sends image bytes through the CLI. Inspect the PNG once with the local image viewer.

Private servers use the production 30 Hz simulation clock by default; an explicitly inherited
`RTS_TEST_TICK_MS` remains available to tests. Successful `order` results include an authoritative
receipt outcome `{accepted:true, playerId}`. Inspected entities keep `state` and `orderPlan` for
explicit simulation orders, while `activity: "engaging"`, `targetId`, and `weaponFacing` expose a
visible acquired combat target without mislabeling autonomous fire as an explicit order.

Artifacts are confined to `target/lab-interact/<session-id>/captures/` and ignored by Git. For a
single-unit detail capture, camera `focus` defaults to close 32-world-pixel padding. Multi-subject
and non-unit focus defaults to 48 world pixels.

## Portable artifacts

`export` writes either a checkpoint-backed `setup` or an authoritative `replay` under
`target/lab-interact/artifacts/`. It returns an opaque `artifactId`, safe absolute paths, counts,
map/tick/build metadata, and optional concise reproduction text; it never prints the embedded
checkpoint or replay operation stream. An adjacent `.aliases.json` sidecar keeps aliases outside
protocol schemas. Setup imports reconcile ids through the server-returned `sourceEntityIdMap`;
replay imports restore aliases that still exist and report stale entries.

`import` destructively replaces only the current ephemeral Lab session. Select an artifact by its
opaque id or by a path already confined beneath this worktree's `target/lab-interact/`; URLs and
external paths are rejected. `artifact-inspect` reports schema, authoring, map, tick/duration,
entity/operation, build, and alias metadata without returning full artifact bytes. Files are capped
at 8 MiB and aliases retain the CLI's 100-entry cap.

Large replays move through a loopback-only, environment-gated bridge between the driver and its
private Rust server. Every request needs the driver's random capability and uses a temporary opaque
transfer id. Transfers expire after ten minutes and are cleared for the room on `close`; server
teardown clears the full in-memory store. The room task remains authoritative for accepted ticks,
operation ordering, validation, future-history truncation, and destructive replay rebuild.
Production startup has no bridge capability and returns 404 for these routes.

## Real-time recording

`record-start` begins one 30 FPS, audio-free VP9 WebM from the persistent headless page. It keeps
clean presentation active and crops to the game viewport, so ordinary `order`, `time`, mutation,
inspection, and `camera` commands can continue through the same session while recording. Inputs
accept only a safe name, a 1–30 second maximum duration (10 seconds by default), an optional
viewport or in-viewport crop, and scale from 0.25 through 1. A second start returns
`recordingActive`; `status` with the current session id reports recorder state.

`record-stop` finalizes the WebM, extracts at most six representative PNGs, creates a 3×2 contact
sheet, probes the media, and returns confined absolute paths plus bounded codec/frame diagnostics.
The adjacent manifest records authoritative start/end ticks and room time, accepted CLI operations,
camera/time changes, aliases, workspace/build/browser/tool versions, probe results, and estimated
dropped or duplicated frames. Those frame counts are diagnostics, not deterministic evidence:
Chrome composition, screencast delivery, wall scheduling, and VP9 encoding all vary between runs.

Recordings live under `target/lab-interact/<session-id>/recordings/`, are capped at 64 MiB, and are
never printed through the CLI. The duration watchdog finalizes automatically. Session `close`,
daemon `shutdown`, and idle teardown attempt bounded finalization and remove a partial directory if
finalization fails; they do not leave FFmpeg owned by the session. Recording checks require
`ffmpeg`, `ffprobe`, and the `libvpx-vp9` encoder on `PATH`, or explicit
`RTS_LAB_INTERACT_FFMPEG`/`RTS_LAB_INTERACT_FFPROBE` paths.

## Fixed-step capture

`capture-fixed` requires an open session whose authoritative room time is paused. Arrange the
scene, issue a normal `order` if movement or direct fire is wanted, and then request 1–180 frames
at an integer 10–60 FPS. The command temporarily suspends the ordinary rAF loop, advances room
time only through the existing 30 Hz `time step` operation, advances the client render clock to
exact fractional milliseconds, and writes one PNG per frame plus a VP9 WebM, contact sheet, and
manifest under `target/lab-interact/<session-id>/fixed/`.

Frame `i` uses `startTick + floor(i * 30 / outputFps)`. Thus 60 FPS intentionally renders each
authoritative state twice at two visual timestamps, while 15 FPS advances two ticks per frame;
fixed capture never mixes live rAF interpolation into either case. The manifest records the
scenario/seed, branch/head/build/runtime identity, tick and visual timestamp for every frame,
SHA-256 frame hashes, and media paths. Hash repeatability is evidence only within the pinned local
browser/GPU environment, not a cross-browser, cross-GPU, or cross-OS golden-image promise.

The command is serialized with other session mutation, rejects an active real-time recorder, and
is treated as one in-flight daemon interaction so idle teardown cannot interrupt it. While it runs,
`status` returns bounded frame progress without touching the page and `capture-cancel` requests
cleanup at the next frame boundary. On failure or cancellation,
partial fixed media is removed and the normal render clock, rAF loop, viewport, and presentation
are restored. The initial supported review cases are idle/frame-strip animation, authoritative
movement, and direct-fire/recoil; audio and other wall-clock-only UI are intentionally outside the
fixed visual-time contract.

## Recovery

| Error | Correction |
| --- | --- |
| `unknownSession` | Run `open` and use its current session id. |
| `unknownAlias` / `staleAlias` | Inspect current state or create a new alias. |
| `invalidKind`, `invalidUpgrade`, or `invalidAbility` | Query `catalog` and use an exposed id. |
| `chromeUnavailable` | Install Chrome/Chromium or set `CHROME` before `open`. |
| `daemonStateUnavailable` / `daemonUnreachable` | Do not remove the socket; restore its owned state or stop the recorded daemon, then retry. |
| `assetLoadFailed`, `captureRenderError`, or `captureTimeout` | Fix the reported source/render problem; do not accept a fallback capture. |
| `ffmpegUnavailable`, `ffprobeUnavailable`, or `vp9Unavailable` | Install a VP9-capable FFmpeg toolchain or set the explicit tool paths, then retry. |
| `recordingActive` / `recordingInactive` | Check session `status`, then stop the active recorder or start a new one. |

## Focused verification

```bash
node tests/lab_interact_cli_contracts.mjs
node tests/lab_interact_driver_contracts.mjs
node tests/lab_interact_recording_contracts.mjs
node tests/lab_interact_cli_smoke.mjs
node tests/lab_interact_driver_smoke.mjs
```
