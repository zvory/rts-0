# Interact CLI

Interact is a project-local, namespaced command-line tool. The `lab` namespace arranges and
inspects small authoritative Lab scenes. The `game` namespace opens either one isolated normal
human-vs-AI match or a spectator-only AI-vs-AI match for bounded inspection, screenshots, real-time
video, and sampled time-lapse video. The observation-only `dev-scenario` namespace opens the existing
server-authored dev scenarios for screenshots, before/after stills, real-time video, and sampled
time lapses. All three
start this worktree's normal Rust server and a headless client using Pixi by default; `open` accepts
`renderer:"babylon"` in every namespace. Interact never edits source files.

Interact requires Node 22.18 or newer and runs its TypeScript source directly through Node's
built-in type stripping; there is no compile or generated-output step. Install the repository-owned
tooling with `npm ci`, and run the strict no-emit check with `npm run check:interact-types`.
The stable `cli.mjs` entry point only checks the Node version and imports `cli.ts`; the browser
bridge remains native JavaScript because the client is buildless.

## Commands

Run commands from the worktree root. `lab` is required before every Lab command, and the optional
final argument must be one JSON object:

```bash
node scripts/interact/cli.mjs lab open '{"viewport":{"width":1000,"height":700,"deviceScaleFactor":1}}'
node scripts/interact/cli.mjs lab open '{"renderer":"babylon","viewport":{"width":1000,"height":700,"deviceScaleFactor":1}}'
node scripts/interact/cli.mjs lab catalog '{"sessionId":"<id>","categories":["players","units","commands"]}'
node scripts/interact/cli.mjs lab spawn '{"sessionId":"<id>","spawns":[{"owner":1,"kind":"rifleman","x":960,"y":960,"alias":"subject"}]}'
node scripts/interact/cli.mjs lab update '{"sessionId":"<id>","updates":[{"operation":"move","entity":"subject","x":1100,"y":960}]}'
node scripts/interact/cli.mjs lab remove '{"sessionId":"<id>","refs":["subject"]}'
node scripts/interact/cli.mjs lab inspect '{"sessionId":"<id>","refs":["subject"]}'
node scripts/interact/cli.mjs lab select '{"sessionId":"<id>","refs":["subject"]}'
node scripts/interact/cli.mjs lab camera '{"sessionId":"<id>","camera":{"action":"focus","refs":["subject"]}}'
node scripts/interact/cli.mjs lab screenshot '{"sessionId":"<id>","name":"subject","presentation":"clean","subjects":["subject"]}'
node scripts/interact/cli.mjs lab record-start '{"sessionId":"<id>","name":"motion","maxDurationMs":10000,"resumeSpeed":1}'
node scripts/interact/cli.mjs lab order '{"sessionId":"<id>","playerId":1,"command":{"c":"move","units":["subject"],"x":1100,"y":960}}'
node scripts/interact/cli.mjs lab order '{"sessionId":"<id>","playerId":1,"command":{"c":"adjustProductionRepeat","buildings":["barracks-a","barracks-b"],"unit":"rifleman","delta":1}}'
node scripts/interact/cli.mjs lab record-wait '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs lab capture-fixed '{"sessionId":"<id>","name":"motion-fixed","fps":30,"frameCount":60}'
node scripts/interact/cli.mjs lab capture-cancel '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs lab export '{"sessionId":"<id>","kind":"setup","name":"two-unit-scene","reproduction":true}'
node scripts/interact/cli.mjs lab artifact-inspect '{"sessionId":"<id>","artifactId":"<artifact-id>"}'
node scripts/interact/cli.mjs lab import '{"sessionId":"<id>","kind":"setup","artifactId":"<artifact-id>"}'
node scripts/interact/cli.mjs lab close '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs lab shutdown
node scripts/interact/cli.mjs lab --help
node scripts/interact/cli.mjs lab help screenshot
node scripts/interact/cli.mjs lab screenshot --help
```

The bounded normal-match surface is separate from Lab authoring:

```bash
node scripts/interact/cli.mjs game open '{"opponent":"ai_2_1","viewport":{"width":1200,"height":800,"deviceScaleFactor":1}}'
node scripts/interact/cli.mjs game open '{"spectate":["ai_2_1","ai_turtle"],"viewport":{"width":1200,"height":800,"deviceScaleFactor":1}}'
node scripts/interact/cli.mjs game inspect '{"sessionId":"<id>","ownership":"owned","limit":100}'
node scripts/interact/cli.mjs game select '{"sessionId":"<id>","ids":[42]}'
node scripts/interact/cli.mjs game camera '{"sessionId":"<id>","camera":{"action":"focus","entities":[42]}}'
node scripts/interact/cli.mjs game camera '{"sessionId":"<id>","camera":{"action":"overview"}}'
node scripts/interact/cli.mjs game screenshot '{"sessionId":"<id>","name":"minimap","region":"minimap"}'
node scripts/interact/cli.mjs game record-start '{"sessionId":"<id>","name":"opening-move","maxDurationMs":10000}'
node scripts/interact/cli.mjs game move '{"sessionId":"<id>","units":[42],"x":1100,"y":960}'
node scripts/interact/cli.mjs game record-wait '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs game capture-timelapse '{"sessionId":"<id>","name":"whole-map","maxDurationMs":120000,"sampleEveryMs":1000,"speed":8,"region":"viewport"}'
node scripts/interact/cli.mjs game capture-timelapse '{"sessionId":"<id>","name":"minimap","maxDurationMs":120000,"sampleEveryMs":1000,"speed":8,"region":"minimap"}'
node scripts/interact/cli.mjs game capture-cancel '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs game give-up '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs game close '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs game shutdown
```

Dev scenarios use their existing server-owned launch fields and expose no gameplay mutations:

```bash
node scripts/interact/cli.mjs dev-scenario open '{"id":"direct_reverse_order","unit":"tank","count":1,"viewport":{"width":1000,"height":700,"deviceScaleFactor":1}}'
node scripts/interact/cli.mjs dev-scenario inspect '{"sessionId":"<id>","limit":100}'
node scripts/interact/cli.mjs dev-scenario select '{"sessionId":"<id>","ids":[42]}'
node scripts/interact/cli.mjs dev-scenario camera '{"sessionId":"<id>","camera":{"action":"overview"}}'
node scripts/interact/cli.mjs dev-scenario screenshot '{"sessionId":"<id>","name":"before"}'
node scripts/interact/cli.mjs dev-scenario record-start '{"sessionId":"<id>","name":"full-run","maxDurationMs":10000}'
node scripts/interact/cli.mjs dev-scenario record-wait '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs dev-scenario screenshot '{"sessionId":"<id>","name":"after"}'
node scripts/interact/cli.mjs dev-scenario capture-timelapse '{"sessionId":"<id>","name":"pathing","maxDurationMs":30000,"sampleEveryMs":500,"speed":4}'
node scripts/interact/cli.mjs dev-scenario capture-cancel '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs dev-scenario close '{"sessionId":"<id>"}'
node scripts/interact/cli.mjs dev-scenario shutdown
```

The complete surface is `open`, `close`, `reset`, `catalog`, `spawn`, `update`, `remove`, `order`,
`time`, `inspect`, `select`, `camera`, `screenshot`, `record-start`, `record-stop`, `record-wait`, `export`,
`import`, `artifact-inspect`, `capture-fixed`, `capture-cancel`, `status`, and `shutdown`. Success
writes exactly one JSON envelope to stdout. Failure writes a concise JSON error to stderr and exits
nonzero. Every command has an exact, bounded input shape; arbitrary state patches, protocol
messages, browser evaluation, and caller-selected artifact paths are not accepted.

The complete `game` surface is `open`, `close`, `status`, `inspect`, `select`, `move`, `camera`, `screenshot`,
`record-start`, `record-stop`, `record-wait`, `capture-timelapse`, `capture-cancel`, `give-up`, and
`shutdown`. `game open` creates a fresh public-name-prefixed lobby. `opponent` creates exactly one
local player and one AI; `spectate:[ai,ai]` creates exactly two opposing AI seats and one spectator,
then starts through the ordinary lobby flow. The launch gate requires `interact-game-*`, player or
spectator role, and `interact=game`, so it cannot attach to an arbitrary room. `move` accepts only
currently visible, locally owned unit ids and an in-map destination. There is no arbitrary command,
attack, build, train, economy,
ability, input-event, DOM-selector, or browser-evaluation surface. `give-up` uses the normal player
surrender flow and returns only after the score screen appears. Spectator sessions expose neither
move nor surrender; their only mutation is the AI-only room's existing bounded speed control used
internally by time-lapse capture.
`select` replaces browser-local selection with up to 400 ids from the recipient's normal
fog-filtered snapshot; an empty list clears it. Player and spectator sessions may use selection to
drive the authentic renderer overlays and HUD, but selection itself sends no gameplay command.

The complete `dev-scenario` surface is `open`, `close`, `status`, `inspect`, `select`, `camera`, `screenshot`,
`record-start`, `record-stop`, `record-wait`, `capture-timelapse`, `capture-cancel`, and `shutdown`.
`dev-scenario open` accepts the same `id`, `unit`, `count`, optional `blocker`, and optional `case`
fields listed by `/dev/scenarios`. Its launch gate requires `watchScenario=1` and
`interact=dev-scenario`; the server still selects and constructs the scenario. The namespace exposes no
spawn, order, move, build, arbitrary protocol, input-event, DOM-selector, or browser-evaluation
surface. Scenario media defaults to clean presentation; use `presentation:"normal"` when the HUD
or minimap is part of the review.
Scenario `select` uses the same browser-local, visible-entity contract and does not expand the
namespace's server authority.

Global help returns the namespace catalog. `lab --help`, `lab help <command>`, and
`lab <command> --help` return the Lab command catalog or a command's exact accepted shape and
variants, defaults, bounds, and one JSON example. All help forms work outside a Git checkout and
never inspect or start a daemon; descriptor coverage is checked against the public command
catalog. Bare commands are rejected so later namespaces cannot silently inherit Lab behavior.

## Application ownership

`scripts/interact/command_registry.ts` is the single public-command definition source. Each
entry contains its daemon/session scope, execution lane, ordinary or lifecycle/media timeout
class, runtime validator, service handler key, and help descriptor. CLI recognition and help,
daemon request deadlines, validation, service dispatch, and semantic ordering all project from
that registry; `command_inputs.ts` owns the exact bounded parsers used by its entries.

The application dependency direction is intentionally small:

```text
cli.mjs -> cli.ts / daemon.ts
        |
command registry + command service lifecycle/dispatch + session coordinator
        |
namespace handlers normalize policy and public input
        |
shared inspect / select / camera / media capabilities
        |
driver + private server + recording / fixed capture / Tailnet / runtime helpers
        |
process runner / filesystem / Puppeteer / FFmpeg / Rust server / Tailscale
```

`command_service.ts` owns session lifecycle and generic registry dispatch. Namespace handlers own
their public policy and normalize differences such as Lab aliases versus Game entity ids,
inspection ownership, and presentation defaults. After normalization, shared capabilities own the
common inspect, select, camera, screenshot, recording, and capture workflows. A capability does not
expand a namespace's authority: the namespace handler must admit the command before calling it.
Driver and infrastructure ownership remains unchanged.

`session_coordinator.ts` owns the only generic semantic FIFO. Commands use four explicit lanes:

| Lane | Commands | Ordering contract |
| --- | --- | --- |
| `serialized` | reset, catalog, mutations, order/time/camera, screenshot, artifact transfer, recording start/stop, fixed capture | Run in admission order for the session. |
| `observation` | status, record-wait | Observe safe current/resource-local state without waiting behind the FIFO. |
| `cancellation` | capture-cancel | Reach an active fixed or time-lapse capture promptly. |
| `lifecycle` | open, close, shutdown | Own application lifecycle; close rejects new session admission and drains work already admitted. |

Resource-local recording completion, encoder backpressure, capture finalization, and watchdogs
remain with their driver/media owner; they are not a second command queue. The daemon alone
installs `SIGINT`, `SIGTERM`, and `SIGHUP` handlers and drives service/driver teardown. The driver
owns browser/page operations, but no process-signal, private-server, finite-child, or generic
semantic queue policy.

## External process and dependency ownership

`process_runner.ts` owns finite request-path children. It invokes direct argv without a shell,
caps stdout and stderr, accepts a timeout and `AbortSignal`, sends TERM before a bounded KILL
fallback, and resolves or rejects only after the child is reaped. Cargo builds, FFmpeg/ffprobe
capability checks and finite post-processing/probes, and `tailscale status --json` use this runner.
The streaming H.264 encoder remains a direct `spawn` owned by `recording.ts` because it requires
stdin backpressure and explicit finalization.

`private_server.ts` owns loopback URL validation/reuse, ephemeral port selection, Cargo build,
the long-running Rust server child, health polling, bounded log ownership, build metadata, and
TERM/KILL teardown. The command service owns the `AbortController` for a cold `open`; shutdown
aborts it before awaiting the open promise, so held Cargo startup cannot block daemon teardown.
The private server remains loopback-only and retains the artifact-transfer capability boundary.

`puppeteer-core`, TypeScript 5.8 or newer, and Node 22 typings are repository-root development
dependencies in `package.json` and `package-lock.json`. Interact and the browser/performance
tests import declared dependencies directly; daemon requests never install or hydrate packages.
`lab open` checks for Puppeteer and Chrome before starting an expensive Rust build and returns the
corresponding remediation immediately when browser tooling is unavailable. Run `npm ci` at the repository root,
or use `tests/run-all.sh`, whose pre-suite cache setup installs the root lock into the shared
lockfile-keyed cache and links the ignored root `node_modules`.

Intentional synchronous exceptions are bounded filesystem work and Git checkout inspection in
`workspace_inspection.ts`, daemon checkout identity in `runtime.ts`, and CLI worktree inspection in
`cli.ts`. They run before long external request work; architecture-checked daemon
paths contain no `spawnSync` or `execSync`.

`open` returns the `sessionId` required by session commands. It is idempotent: repeated or
concurrent calls return the one active session instead of starting another browser. Run `close`
before `open` when a fresh session or different launch options are required. Optional aliases match
`[A-Za-z][A-Za-z0-9_-]{0,31}` and remain private to that session. Unknown, duplicate, stale, or
cross-session aliases are rejected rather than guessed. A session may retain up to 400 aliases.
Only one authoritative session may be open per worktree across all three namespaces. Opening another
kind while a session is active returns `sessionKindMismatch` and preserves the current session.
Server-rejected Lab launches, such as an unknown map or scenario, return `launchFailed` with the
server's bounded error text as soon as the browser receives it rather than consuming the full
startup timeout.

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

A cold first `open` may spend several minutes building the selected worktree's Rust server before
it writes its single JSON response. Cargo has a separate five-minute build deadline, and the CLI
remains attached beyond that bounded build plus server/browser readiness. A timed-out build returns
`serverBuildTimeout` with its exit/signal metadata and `cargo-build.log` path; a compiler failure
returns `serverBuild` with the same diagnostics. Keep that CLI process attached until it exits.
A concurrent `status` reports `opening: true` while startup is still in progress, and an idempotent `open` retry
recovers the same completed session if the original caller was interrupted. Daemon `shutdown`
aborts and reaps an in-progress Cargo/private-server startup instead of waiting for its normal
startup deadline. Global `--help`, `-h`, and `help` return the bounded namespace catalog without
requiring a Git worktree or starting a daemon; `lab --help` returns the Lab command catalog under
the same rule.

## Automatic daemon lifecycle

The first command starts a background daemon automatically. It is isolated by the real worktree
path and communicates over a mode-0600 Unix socket in a mode-0700 temporary runtime directory. A
versioned daemon identity and random capability in its mode-0600 state file must match every
request. This prevents a stale or unrelated local listener from being mistaken for the selected
worktree's daemon.

The daemon preserves its browser, private Rust server, aliases, and authoritative session across
CLI processes. Each accepted interaction resets a 30-minute idle deadline. An in-flight command
cannot expire. Idle expiry or `shutdown` closes the driver, browser, and Rust server, removes its
socket/state/runtime files, and exits. `RTS_INTERACT_IDLE_MS` is a bounded test-only override;
normal use should leave it unset. Visual preview delivery has a separate machine-level lifecycle:
issued URLs remain available after this per-worktree daemon exits.

The daemon state and compatible probe publish the checkout commit captured at daemon startup. The
CLI compares it with the selected worktree's current `HEAD` before dispatch. `status` remains
available and reports both commits and whether they match; `shutdown` also remains available. An
idle mismatched daemon with no session, opening/closing lifecycle, or other active request is
atomically shut down and refreshed. An active mismatch returns `daemonCheckoutMismatch`, preserves
the scene, and includes `node scripts/interact/cli.mjs lab shutdown` as the explicit recovery
command. A pre-feature daemon with no checkout field is treated the same as any other mismatch.
Because it may predate the atomic refresh handshake, it is preserved for explicit `status` and
`shutdown` rather than automatically restarted. Checkout metadata is optional IPC v1 probe/state
data and is not part of daemon authentication.

## Capture workflow

Query `catalog` before selecting owners, entity kinds, upgrades, abilities, or commands. Confirm
mutations with `inspect`, control authoritative time with `time`, and compose with `camera`.
Aliases, inspection, selection, camera focus, and screenshot subjects accept up to 400 entity references.
`screenshot` waits for fonts, relevant assets, two error-free render frames, and
authoritative state. The CLI returns an opaque Tailnet Preview URL plus bounded metadata; it
deliberately withholds local PNG and manifest paths so callers share the Tailnet URL rather than a
raw file. Readiness checks cover every requested subject, while
the response and manifest record the subject count, `truncated` state, and at most 24 detailed
subject rows. `presentation: "clean"` hides UI chrome; `presentation: "normal"` retains visible Lab
panels and game UI. Game screenshots and recordings default to `normal`; Lab screenshots and
recordings default to `clean`. Inspect the Tailnet preview once before delivery.

Successful visual responses include this delivery-shaped field (the URL is opaque and host-specific):

```json
{
  "preview": {
    "available": true,
    "url": "http://100.x.y.z:port/interact-preview/<opaque-token>",
    "instruction": "Share this Tailnet URL with the user to preview the Interact artifact. Do not share a local file path."
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

Repeat production uses the normal authoritative `adjustProductionRepeat` game command. Its
`buildings` field accepts 1–100 aliases or ids and resolves the entire producer set before enqueue.
`delta:1` adds the requested `unit` to one eligible producer; `delta:-1` removes it from one
producer. Producer ownership, compatibility, allocation policy, resources, supply, and retry
behavior remain ordinary simulation rules.

Artifacts are first confined to `target/interact/<lab|game|scenario>/<session-id>/` and ignored by Git.
On publication, Interact copies the PNG or MP4 into the machine-level `tailnet-preview` service
outside the worktree. That service binds the stable Tailnet port 8091, has no idle timeout, and
retains each copied artifact for at least 24 hours. The URL therefore survives Lab `close`,
`shutdown`, idle teardown, and removal of the originating worktree. A later publication restarts
the service if it is not running while retaining any unexpired copied artifacts.

Each URL has an unguessable per-artifact id, serves only its registered PNG or MP4 with no directory
route, and never exposes the private Lab game server or local filesystem paths. If Tailscale is
unavailable, the visual capture still completes and its response reports an actionable unavailable
preview instead of a raw path.
For a single-unit detail capture, camera `focus` defaults to close 32-world-pixel padding.
Multi-subject and non-unit focus defaults to 48 world pixels.

## Portable artifacts

`export` writes either a checkpoint-backed `setup` or an authoritative `replay` under
`target/interact/lab/artifacts/`. It returns an opaque `artifactId`, safe absolute paths, counts,
map/tick/build metadata, and optional concise reproduction text; it never prints the embedded
checkpoint or replay operation stream. An adjacent `.aliases.json` sidecar keeps aliases outside
protocol schemas. Setup imports reconcile ids through the server-returned `sourceEntityIdMap`;
replay imports restore aliases that still exist and report stale entries.
Import responses summarize restored and stale aliases independently with counts, truncation state,
and at most 12 rows each. `details:true` opts into every reconciliation row and the raw import result.

`import` destructively replaces only the current ephemeral Lab session. Select an artifact by its
opaque id or by a path already confined beneath this worktree's `target/interact/lab/`; URLs and
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
It keeps the selected presentation active and crops to the game viewport, so ordinary `order`, `time`,
mutation, inspection, and `camera` commands can continue through the same session while recording. Inputs
accept only a safe name, a 1–60 second maximum duration (10 seconds by default), an optional
viewport, `region:"viewport"`, `region:"minimap"`, or an in-viewport custom crop, and scale from
0.25 through 1. A second start returns
`recordingActive`; `status` with the current session id reports recorder state. Optional
`resumeSpeed` from 0.01 through 16 resumes authoritative time only after Chrome has delivered the
initial capture frame, within the same serialized command. This avoids paused dead air between
separate `record-start` and `time resume` calls.

Interact acknowledges raw Chrome DevTools screencast frames and streams them directly to a
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

Recordings live under `target/interact/<lab|game|scenario>/<session-id>/recordings/`, are capped at 64 MiB, and are
never printed through the CLI. The duration watchdog finalizes automatically. Session `close`,
daemon `shutdown`, and idle teardown attempt bounded finalization and remove a partial directory if
finalization fails; they do not leave FFmpeg owned by the session. Recording checks require
`ffmpeg`, `ffprobe`, and the `libx264` encoder on `PATH`, or explicit
`RTS_INTERACT_FFMPEG`/`RTS_INTERACT_FFPROBE` paths.

Real-time recordings are silent. Chrome's screencast API does not expose the page's WebAudio
graph, and Interact does not depend on macOS system-audio routing.

## AI-vs-AI time-lapse capture

`game capture-timelapse` requires a session opened with `spectate:[ai,ai]`. It temporarily selects
the AI-only live room's existing authoritative speed (8× by default), samples a PNG at a bounded
wall-clock interval (one second by default), and encodes the samples at 10–60 output FPS (30 by
default). It stops when the score screen appears or after 1–300 seconds. At most 1,800 samples and
64 MiB of H.264 MP4 are retained. The prior room speed, viewport, and presentation are restored on
success, cancellation, or failure.

`region:"viewport"` captures the full game screen; `region:"minimap"` resolves the live minimap DOM
bounds so responsive layouts do not require hard-coded coordinates. A custom
`region:{x,y,width,height}` is relative to the game viewport and must stay entirely inside it.
Minimap capture requires normal presentation because clean presentation hides the HUD. For a
stable whole-battlefield time-lapse, run `game camera` with `action:"overview"` first; this fits the
authoritative map bounds and disables the automatic spectator camera. Time-lapse results include
Tailnet video/contact-sheet previews, sampled ticks and hashes in the withheld manifest, the actual
stop reason, and the selected region. `status` reports progress and `capture-cancel` interrupts at
the next wait/frame boundary.

`dev-scenario capture-timelapse` uses the same sampling, region, encoder, progress, cancellation, and
preview contracts against an authored dev-scenario watcher room. It defaults to clean presentation,
records the scenario launch fields in the withheld manifest, and temporarily restores the prior
authoritative speed after capture. Run `dev-scenario camera` with `action:"overview"` first when a fixed
whole-map view is more useful than the normal watcher camera.

## Fixed-step capture

`capture-fixed` requires an open session whose authoritative room time is paused. Arrange the
scene, issue a normal `order` if movement or direct fire is wanted, and then request 1–1,800 frames
at an integer 10–60 FPS (one minute at 30 FPS). The command temporarily suspends the ordinary rAF loop, advances room
time only through the existing 30 Hz `time step` operation, advances the client render clock to
exact fractional milliseconds, and streams each PNG directly into FFmpeg instead of retaining the
full sequence. It keeps at most six representative PNGs plus an H.264 MP4, contact sheet, and
manifest under `target/interact/lab/<session-id>/fixed/`.

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
| `tailnetPreviewUnavailable` | Start Tailscale or restore the machine-level preview service, then capture again; share the returned Tailnet URL rather than a local path. |
| `recordingActive` / `recordingInactive` | Check session `status`, then stop/wait for the active recorder or start a new one. A wait before any start is inactive. |

## Focused verification

The fast contract set uses the fake driver and needs no Chrome or Rust server. FFmpeg and ffprobe
with H.264 support are required by the recording and fixed-capture contracts:

```bash
npm run check:interact-types
node scripts/check-interact-architecture.mjs
node scripts/check-source-file-sizes.mjs
node tests/interact_adapter_contracts.mjs
node tests/interact_cli_contracts.mjs
node tests/interact_artifact_contracts.mjs
node tests/interact_tailnet_preview_contracts.mjs
node tests/interact_driver_contracts.mjs
node tests/interact_bulk_contracts.mjs
node tests/interact_recording_contracts.mjs
node tests/interact_fixed_capture_contracts.mjs
node tests/interact_session_coordinator_contracts.mjs
```

The live browser canary needs Chrome/Chromium plus FFmpeg/ffprobe. Standalone mode starts and owns a
private Rust server; the browser smoke shard reuses its already-running loopback server. Both modes
run the same two-entity semantic workflow and clean their daemon runtime, UUID-owned session output,
and exact setup/alias-sidecar files on success or failure:

```bash
node tests/interact_cli_smoke.mjs
tests/run-all.sh --only-browser-scenarios=smoke
RTS_INTERACT_LAB_RECORDING_CANARY_MS=60000 node tests/interact_cli_smoke.mjs
node tests/interact_driver_smoke.mjs
```
