# Tauri Desktop Shell

This is the Tauri shell for the live Bewegungskrieg web client. It opens a
shell-owned startup selector first, then loads one of the built-in release
channels in the same window:

- Beta: `https://rts-0-zvorygin-beta.fly.dev/`
- Mainline: `https://rts-0-zvorygin.fly.dev/`

The MVP startup UI intentionally has no local server profile, loopback option,
or custom URL entry. The shell does not start `rts-server` and does not serve or
bundle game assets; the selected release website provides the client, maps, and
WebSocket endpoint.

Run it from the repo root on macOS:

```bash
scripts/open-tauri-game.sh
```

Or run it from this directory:

```bash
./run.sh
```

The WebView injects `window.__RTS_DESKTOP_RUNTIME` before client scripts run.
Remote beta/mainline pages receive the same native cursor bridge as the earlier
spike:

```js
{
  shell: "tauri",
  platform: "macos",
  nativeCursorBackend: true,
  nativeCursorCapture: true,
  pointerLockDisabled: true,
  aggressiveCursorLock: true,
  serverMode: "startup" | "release" | "developer",
  serverUrl: "https://rts-0-zvorygin-beta.fly.dev/" | "https://rts-0-zvorygin.fly.dev/" | null,
  releaseChannel: "beta" | "mainline" | null
}
```

Pointer Lock is deliberately disabled inside the macOS shell. Cursor-lock requests
use `window.__RTS_NATIVE_CURSOR`, a Tauri-injected native bridge that hides and
disconnects the macOS cursor, forwards native mouse movement/down/up/wheel
events to the client, and exposes `diagnostics()` with the active backend,
native event count, JS processed count, dropped event count, delivery latency,
and whether movement is batched. The current visible cursor is a DOM cursor
painted directly in the native event handler (`visual: "dom-event-time"`), not
a native overlay.

That native bridge is macOS-only. Windows injects the same installed-app/runtime and release-channel
metadata with `platform: "windows"`, but reports no native cursor backend or capture requirement and
does not replace the Pointer Lock API. Windows uses WebView2 Pointer Lock with raw
`unadjustedMovement`; if raw input is unavailable, cursor lock fails instead of falling back to
lower-quality adjusted movement.

Once a non-replay match starts in the Tauri shell, the web client aggressively
requests native cursor capture, retries on focused unlocks, and grabs the cursor
again after the window regains focus. Alt-Tab releases capture through the shell
window blur handler; focusing the game window again re-captures it.

Developer-only shortcuts for local debug runs:

- `RTS_DESKTOP_SERVER_URL=http://127.0.0.1:<port>/ ./run.sh` skips the startup
  selector and opens a loopback server started outside the shell. Non-loopback
  URLs are rejected. This override is ignored by release builds so the packaged
  MVP path always starts at the Beta/Mainline selector.
- `RTS_DESKTOP_AUTOSTART=1` and `RTS_DESKTOP_AUTOLOCK=1` are still available as
  engineering aids after a release channel or developer loopback server loads.
  They do not run on the startup selector page.

## Unsigned macOS artifact

Build a local unsigned playtest artifact from this directory:

```bash
./build-unsigned.mjs
```

Prerequisites:

- macOS with Xcode command line tools installed.
- Rust/Cargo and `cargo tauri` 2.x available on `PATH`.
- Node.js 18 or newer for the build wrapper.
- A git checkout so the artifact manifest can record the exact commit SHA.

By default the output is written under:

```text
src-tauri/target/unsigned-playtest/
```

The artifact directory and zip are named
`bewegungskrieg-v<version>-<short-sha>-<arch>`. Each artifact directory
contains `Bewegungskrieg.app`, `manifest.json`, `README.md`, and
`contents.txt`. The manifest records the git SHA, build date, target
architecture, shell version, Tauri product metadata, release-channel URLs, and
the thin-shell asset check. `contents.txt` lists the payload files with SHA-256
hashes and byte sizes.

The command invokes `cargo tauri build --bundles app --no-sign --ci` with a
temporary config override that enables the app bundle target. The artifact is
not Developer ID signed, notarized, or stapled. The command copies only the
Tauri app bundle into the artifact directory, creates a zip, and fails if the
bundle contains obvious game runtime assets such as `rts-server`, `client/`,
`maps/`, `lab-scenarios/`, or match-history data. The app still loads all game
content from the selected beta or mainline website.

Use `./build-unsigned.mjs --output <dir>` to write the artifact somewhere else.

## Local logs

The shell writes local JSONL diagnostics to Tauri's app log directory:

```text
~/Library/Logs/dev.bewegungskrieg.Bewegungskrieg/shell.log
```

The startup screen has **Copy log path** and **Reveal logs** actions. Log-path
commands are guarded so loaded beta/mainline pages can record bounded runtime
events but cannot read or reveal local filesystem paths.

Logged events include shell startup, app version, optional build id,
dev/packaged mode, configured release profiles, selected profile id/URL,
navigation start/finish/rejection/timeout, native cursor capture start/failure,
and desktop autostart/autolock failures. URLs are logged without query strings
or fragments. The shell does not upload logs, record gameplay commands, collect
telemetry, or include secrets intentionally.

Manual check:

1. Run `./run.sh`.
2. Confirm only Beta and Mainline choices appear.
3. Choose Beta and confirm the loaded page uses
   `https://rts-0-zvorygin-beta.fly.dev/`.
4. Restart, choose Mainline, and confirm the loaded page uses
   `https://rts-0-zvorygin.fly.dev/`.
5. From the lobby, use **Open Lab** and confirm the lab opens in the same
   shell window and starts the lab room.
6. Start a one-player sandbox or AI match from either release channel.
7. Confirm the shell locks the cursor automatically in-match, then Alt-Tab away
   and back to confirm it re-locks. Move over terrain/HUD/minimap, right-click
   move units, box-select, and wheel zoom. Inspect
   `window.__RTS_NATIVE_CURSOR.diagnostics()` if movement feels delayed.
8. Use **Copy log path** or **Reveal logs** from the startup screen and confirm
   `shell.log` contains startup and selected-profile events.
