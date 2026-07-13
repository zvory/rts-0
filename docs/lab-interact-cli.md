# Lab Interact CLI

Lab Interact is a project-local command-line tool for arranging and inspecting small authoritative
Lab scenes through a machine-readable local interface. It starts this worktree's normal Rust
server and a headless client using Pixi by default. Mutations are ephemeral and never edit source
files; `open` accepts `renderer:"babylon"` for the explicit Babylon Lab route.

## Commands

Run commands from the worktree root. The optional second argument must be one JSON object:

```bash
node scripts/lab-interact/cli.mjs open '{"viewport":{"width":1000,"height":700,"deviceScaleFactor":1}}'
node scripts/lab-interact/cli.mjs open '{"renderer":"babylon","viewport":{"width":1000,"height":700,"deviceScaleFactor":1}}'
node scripts/lab-interact/cli.mjs catalog '{"sessionId":"<id>","categories":["players","units","commands"]}'
node scripts/lab-interact/cli.mjs spawn '{"sessionId":"<id>","spawns":[{"owner":1,"kind":"rifleman","x":960,"y":960,"alias":"subject"}]}'
node scripts/lab-interact/cli.mjs update '{"sessionId":"<id>","updates":[{"operation":"move","entity":"subject","x":1100,"y":960}]}'
node scripts/lab-interact/cli.mjs remove '{"sessionId":"<id>","refs":["subject"]}'
node scripts/lab-interact/cli.mjs inspect '{"sessionId":"<id>","refs":["subject"]}'
node scripts/lab-interact/cli.mjs camera '{"sessionId":"<id>","camera":{"action":"focus","refs":["subject"]}}'
node scripts/lab-interact/cli.mjs screenshot '{"sessionId":"<id>","name":"subject","presentation":"clean","subjects":["subject"]}'
node scripts/lab-interact/cli.mjs record-start '{"sessionId":"<id>","name":"motion","maxDurationMs":10000,"resumeSpeed":1}'
node scripts/lab-interact/cli.mjs order '{"sessionId":"<id>","playerId":1,"command":{"c":"move","units":["subject"],"x":1100,"y":960}}'
node scripts/lab-interact/cli.mjs order '{"sessionId":"<id>","playerId":1,"command":{"c":"setProductionRepeat","buildings":["barracks-a","barracks-b"],"unit":"rifleman","enabled":true}}'
node scripts/lab-interact/cli.mjs record-wait '{"sessionId":"<id>"}'
node scripts/lab-interact/cli.mjs capture-fixed '{"sessionId":"<id>","name":"motion-fixed","fps":30,"frameCount":60}'
node scripts/lab-interact/cli.mjs capture-cancel '{"sessionId":"<id>"}'
node scripts/lab-interact/cli.mjs export '{"sessionId":"<id>","kind":"setup","name":"two-unit-scene","reproduction":true}'
node scripts/lab-interact/cli.mjs artifact-inspect '{"sessionId":"<id>","artifactId":"<artifact-id>"}'
node scripts/lab-interact/cli.mjs import '{"sessionId":"<id>","kind":"setup","artifactId":"<artifact-id>"}'
node scripts/lab-interact/cli.mjs close '{"sessionId":"<id>"}'
node scripts/lab-interact/cli.mjs shutdown
node scripts/lab-interact/cli.mjs --help
node scripts/lab-interact/cli.mjs help screenshot
node scripts/lab-interact/cli.mjs screenshot --help
```

The complete surface is `open`, `close`, `reset`, `catalog`, `spawn`, `update`, `remove`, `order`,
`time`, `inspect`, `camera`, `screenshot`, `record-start`, `record-stop`, `record-wait`, `export`,
`import`, `artifact-inspect`, `capture-fixed`, `capture-cancel`, `status`, and `shutdown`. Success
writes exactly one JSON envelope to stdout. Failure writes a concise JSON error to stderr and exits
nonzero. Every command has an exact, bounded input shape; arbitrary state patches, protocol
messages, browser evaluation, and caller-selected artifact paths are not accepted.

Global help returns the command catalog. `help <command>` and `<command> --help` return that
command's exact accepted shape and variants, defaults, bounds, and one JSON example. All help
forms work outside a Git checkout and never inspect or start a daemon; descriptor coverage is
checked against the public command catalog.

`open` returns the `sessionId` required by session commands. It is idempotent: repeated or
concurrent calls return the one active session instead of starting another browser. Run `close`
before `open` when a fresh session or different launch options are required. Optional aliases match
`[A-Za-z][A-Za-z0-9_-]{0,31}` and remain private to that session. Unknown, duplicate, stale, or
cross-session aliases are rejected rather than guessed. A session may retain up to 400 aliases.
Only one authoritative session may be open per worktree.

`spawn`, `update`, and `remove` accept 1–400 items and each command reaches the authoritative game
as one atomic plural operation. `update` accepts `updates:[...]`; the legacy singular `update:{...}`
shape remains accepted and is normalized to a one-item `applyUpdates` request. Alias changes occur
only after a complete accepted batch. A rejection leaves the scene and aliases unchanged and
includes `error.details.failedIndex`; placement failures additionally preserve `attempted`, typed
`blockers`, and at most eight authoritative `suggestions`. Retrying the original batch with a
returned suggestion therefore uses the same placement rules rather than a client-side guess.
Successful `spawn` output is compact by default: `spawned` reports the count, whether its ordered
sample is truncated, and at most 12 `{index,alias,id}` rows, plus the authoritative snapshot tick.
Use `details:true` only when the caller needs every decorated entity and the raw authoritative
outcome. This does not reduce or truncate rejection diagnostics.

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

The daemon state and compatible probe publish the checkout commit captured at daemon startup. The
CLI compares it with the selected worktree's current `HEAD` before dispatch. `status` remains
available and reports both commits and whether they match; `shutdown` also remains available. An
idle mismatched daemon with no session, opening/closing lifecycle, or other active request is
atomically shut down and refreshed. An active mismatch returns `daemonCheckoutMismatch`, preserves
the scene, and includes `node scripts/lab-interact/cli.mjs shutdown` as the explicit recovery
command. A pre-feature daemon with no checkout field is treated the same as any other mismatch.
Because it may predate the atomic refresh handshake, it is preserved for explicit `status` and
`shutdown` rather than automatically restarted. Checkout metadata is optional IPC v1 probe/state
data and is not part of daemon authentication.

## Capture workflow

Query `catalog` before selecting owners, entity kinds, upgrades, abilities, or commands. Confirm
mutations with `inspect`, control authoritative time with `time`, and compose with `camera`.
Aliases, inspection, camera focus, and screenshot subjects accept up to 400 entity references.
`screenshot` waits for fonts, relevant assets, two error-free render frames, and
authoritative state. The CLI returns an opaque Tailnet Preview URL plus bounded metadata; it
deliberately withholds local PNG and manifest paths so callers share the Tailnet URL rather than a
raw file. Readiness checks cover every requested subject, while
the response and manifest record the subject count, `truncated` state, and at most 24 detailed
subject rows. `presentation: "clean"` hides UI chrome; `presentation: "normal"` retains visible Lab
panels and game UI. Inspect the Tailnet preview once before delivery.

Successful visual responses include this delivery-shaped field (the URL is opaque and host-specific):

```json
{
  "preview": {
    "available": true,
    "url": "http://100.x.y.z:port/lab-interact-preview/<opaque-token>",
    "instruction": "Share this Tailnet URL with the user to preview the Lab artifact. Do not share a local file path."
  }
}
```

Private servers use the production 30 Hz simulation clock by default; an explicitly inherited
`RTS_TEST_TICK_MS` remains available to tests. Successful `order` results include an authoritative
enqueue receipt outcome `{accepted:true, admission:"enqueued", playerId, queuedAtTick}`. This
confirms validated queue admission, not a completed or non-no-op gameplay effect. Inspected entities
keep `state` and `orderPlan` for explicit simulation orders, while `activity: "engaging"`,
`targetId`, and `weaponFacing` expose a
visible acquired combat target without mislabeling autonomous fire as an explicit order.
Paused setup mutations fan out their accepted authoritative state without advancing combat. A
paused `order` still advances one bounded tick so the queued command can be consumed; use explicit
`time step` for any additional simulation progress.

Repeat production uses the normal authoritative `setProductionRepeat` game command. Its
`buildings` field accepts 1–100 aliases or ids, resolves the entire producer set before enqueue,
and toggles the requested `unit` with `enabled:true|false` in one command. The producer ownership,
production compatibility, resources, supply, and retry behavior remain ordinary simulation rules.

Artifacts are confined to `target/lab-interact/<session-id>/captures/` and ignored by Git. The
daemon starts an artifact-only HTTP listener on this machine's Tailnet IP on first visual capture.
Each URL has an unguessable per-artifact token, serves only its registered PNG or MP4 with no
directory route, and remains available until daemon shutdown or idle teardown. It never exposes the
private Lab game server or local filesystem paths. If Tailscale is unavailable, the visual capture
still completes and its response reports an actionable unavailable preview instead of a raw path.
For a single-unit detail capture, camera `focus` defaults to close 32-world-pixel padding.
Multi-subject and non-unit focus defaults to 48 world pixels.

## Portable artifacts

`export` writes either a checkpoint-backed `setup` or an authoritative `replay` under
`target/lab-interact/artifacts/`. It returns an opaque `artifactId`, safe absolute paths, counts,
map/tick/build metadata, and optional concise reproduction text; it never prints the embedded
checkpoint or replay operation stream. An adjacent `.aliases.json` sidecar keeps aliases outside
protocol schemas. Setup imports reconcile ids through the server-returned `sourceEntityIdMap`;
replay imports restore aliases that still exist and report stale entries.
Import responses summarize restored and stale aliases independently with counts, truncation state,
and at most 12 rows each. `details:true` opts into every reconciliation row and the raw import result.

`import` destructively replaces only the current ephemeral Lab session. Select an artifact by its
opaque id or by a path already confined beneath this worktree's `target/lab-interact/`; URLs and
external paths are rejected. `artifact-inspect` reports schema, authoring, map, tick/duration,
entity/operation, build, and alias metadata without returning full artifact bytes. Files are capped
at 8 MiB. Alias sidecars accept 400 maximum-length aliases within the retained 64 KiB byte cap.

Large replays move through a loopback-only, environment-gated bridge between the driver and its
private Rust server. Every request needs the driver's random capability and uses a temporary opaque
transfer id. Transfers expire after ten minutes and are cleared for the room on `close`; server
teardown clears the full in-memory store. The room task remains authoritative for accepted ticks,
operation ordering, validation, future-history truncation, and destructive replay rebuild.
Production startup has no bridge capability and returns 404 for these routes.

## Real-time recording

`record-start` begins one 30 FPS, audio-free H.264 MP4 recording from the persistent headless page.
It keeps clean presentation active and crops to the game viewport, so ordinary `order`, `time`,
mutation, inspection, and `camera` commands can continue through the same session while recording. Inputs
accept only a safe name, a 1–60 second maximum duration (10 seconds by default), an optional
viewport or in-viewport crop, and scale from 0.25 through 1. A second start returns
`recordingActive`; `status` with the current session id reports recorder state. Optional
`resumeSpeed` from 0.01 through 16 resumes authoritative time only after Chrome has delivered the
initial capture frame, within the same serialized command. This avoids paused dead air between
separate `record-start` and `time resume` calls.

Lab Interact acknowledges raw Chrome DevTools screencast frames and streams them directly to a
mobile-compatible H.264 MP4 with `yuv420p`, an `avc1` tag, and fast-start metadata. One timing
authority maps cumulative monotonic wall time to 30 FPS output slots; each slot receives the newest
raw frame. There is no intermediate WebM timeline and no second timestamp redistribution pass.
Odd dimensions are normalized to even values for H.264 compatibility. Finalization extracts
at most six representative PNGs,
creates a 3×2 contact sheet, probes the media, and returns a Tailnet MP4 preview plus a Tailnet
contact-sheet preview with bounded codec/frame diagnostics. Local video, frame, contact-sheet, and
manifest paths are deliberately withheld from CLI output.
The adjacent manifest records authoritative start/end ticks and room time, accepted CLI operations,
camera/time changes, aliases, workspace/build/browser/tool versions, and probe results. Frame
diagnostics report raw screencast events, raw Chrome timestamp span and largest gap, source frames
actually used, exact output-slot reuse, and source coverage. Coverage below 80% is marked
`deficient` and carries a warning to use `capture-fixed`; it is never described as an estimated
capture count. Alias summary metadata records the total and `truncated` state with at most 40
detailed rows. These diagnostics remain nondeterministic because Chrome composition, screencast
delivery, and wall scheduling vary between runs.

`record-wait` observes the current recorder outside the session mutation queue. An active or
finalizing recording awaits the same completion used by its watchdog and `record-stop`; an already
completed current recording returns the same result again. This lets callers start a recording,
continue issuing authoritative mutations, orders, time controls, inspections, and camera changes,
then receive the finalized artifact without sleeps or status polling. A session that has never
started a recording returns `recordingInactive`.

Recorder flush, MP4 transcode, and auxiliary FFmpeg stages derive bounded timeouts from the target
duration, capped at 45, 75, and 30 seconds respectively. `record-stop`, `record-wait`, `close`, and
`shutdown` use a dedicated 420-second IPC deadline; ordinary commands retain their existing
120-second deadline. Close, shutdown, and idle cleanup initiate recorder settlement before draining
queued session work, so an outstanding waiter cannot prevent the lifecycle action that resolves it.

Recordings live under `target/lab-interact/<session-id>/recordings/`, are capped at 64 MiB, and are
never printed through the CLI. The duration watchdog finalizes automatically. Session `close`,
daemon `shutdown`, and idle teardown attempt bounded finalization and remove a partial directory if
finalization fails; they do not leave FFmpeg owned by the session. Recording checks require
`ffmpeg`, `ffprobe`, and the `libx264` encoder on `PATH`, or explicit
`RTS_LAB_INTERACT_FFMPEG`/`RTS_LAB_INTERACT_FFPROBE` paths.

Real-time recordings are silent. Chrome's screencast API does not expose the page's WebAudio
graph, and Lab Interact does not depend on macOS system-audio routing.

## Fixed-step capture

`capture-fixed` requires an open session whose authoritative room time is paused. Arrange the
scene, issue a normal `order` if movement or direct fire is wanted, and then request 1–1,800 frames
at an integer 10–60 FPS (one minute at 30 FPS). The command temporarily suspends the ordinary rAF loop, advances room
time only through the existing 30 Hz `time step` operation, advances the client render clock to
exact fractional milliseconds, and streams each PNG directly into FFmpeg instead of retaining the
full sequence. It keeps at most six representative PNGs plus an H.264 MP4, contact sheet, and
manifest under `target/lab-interact/<session-id>/fixed/`.

Frame `i` uses `startTick + floor(i * 30 / outputFps)`. Thus 60 FPS intentionally renders each
authoritative state twice at two visual timestamps, while 15 FPS advances two ticks per frame;
fixed capture never mixes live rAF interpolation into either case. The manifest records the
scenario/seed, branch/head/build/runtime identity, tick and visual timestamp for every frame,
SHA-256 frame hashes, optional representative paths, and media paths. The bounded CLI response
returns only frame count, unique-hash count, and a Tailnet MP4/contact-sheet preview; full rows
remain in the manifest. Representative local paths are deliberately withheld. Hash repeatability is
evidence only within the pinned local browser/GPU environment, not a cross-browser, cross-GPU, or
cross-OS golden-image promise.
Fixed-capture scene alias metadata uses the same 40-row detailed-summary cap.

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
| `daemonCheckoutMismatch` | Run `status` to inspect the preserved scene. When it is safe to discard, run the returned `shutdown` recovery command and retry from the current checkout. |
| `assetLoadFailed`, `captureRenderError`, or `captureTimeout` | Fix the reported source/render problem; do not accept a fallback capture. |
| `ffmpegUnavailable`, `ffprobeUnavailable`, or `h264Unavailable` | Install an FFmpeg toolchain with `libx264`, or set the explicit tool paths, then retry. |
| `tailnetUnavailable` / `tailnetPreviewBindFailed` | Start Tailscale and capture again; share the returned Tailnet URL rather than a local path. |
| `recordingActive` / `recordingInactive` | Check session `status`, then stop/wait for the active recorder or start a new one. A wait before any start is inactive. |

## Focused verification

```bash
node tests/lab_interact_cli_contracts.mjs
node tests/lab_interact_tailnet_preview_contracts.mjs
node tests/lab_interact_driver_contracts.mjs
node tests/lab_interact_bulk_contracts.mjs
node tests/lab_interact_recording_contracts.mjs
node tests/lab_interact_cli_smoke.mjs
RTS_LAB_INTERACT_RECORDING_CANARY_MS=60000 node tests/lab_interact_cli_smoke.mjs
node tests/lab_interact_driver_smoke.mjs
```
