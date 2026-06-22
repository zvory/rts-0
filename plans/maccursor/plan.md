# macOS Native Cursor Capture Spike Plan

## Phase Summaries

### Phase 1 - Native Capture Harness

Build the smallest macOS-native capture harness before touching the game. It must prove that the
native side can hide/disconnect the system cursor, receive mouse movement immediately, move a native
test cursor on each OS mouse event, and restore the cursor cleanly on Escape, blur, or app
deactivation. This phase must not route cursor movement through `requestAnimationFrame`, accumulate
deltas for later playback, or send one mouse update per rendered frame.

### Phase 2 - Desktop Shell Thin Slice

Create a macOS-first Tauri desktop shell that launches the existing local Rust server and loads the
served client, with no gameplay behavior changes yet. This proves that Tauri can host the current
HTML/CSS/JS client, keep the existing `/ws` same-origin model, and package the current server/client
asset shape enough for local spike testing. The shell must keep browser Pointer Lock disabled for
the native-capture path, so any cursor success or failure comes from the native backend instead of
Chromium or WKWebView.

### Phase 3 - Native Cursor Backend Integration

Wire the native macOS cursor backend into the existing client cursor-lock seam and in-match input
state. Native movement must be delivered as an event stream as quickly as the WebView can accept it,
and the visible cursor must move immediately per native event through a native overlay or direct
event-time visual update, not through frame-batched JS. If JS event delivery cannot stay below the
latency target during realistic game load, this phase must preserve the native visual cursor path
and document exactly which gameplay interactions still wait for JS.

### Phase 4 - In-Game Plausibility Gate

Run the spike in an actual local match and measure whether it feels and behaves better than browser
Pointer Lock on the Mac hardware available to us. This phase should produce the go/no-go evidence:
cursor latency observations, failure modes, cleanup reliability, control-group behavior, edge-pan
behavior, and whether Tauri remains the right shell. The output is either a recommendation to
continue toward a shippable macOS desktop app or a clear stop reason with the next fallback.

## Purpose

Find out quickly whether a macOS-native cursor capture path can make Bewegungskrieg feel acceptable
as a desktop app. The spike targets visible and interactive mouse latency, not simulation latency,
snapshot latency, or render throughput. The fastest useful answer is a local Mac playtest that uses
native OS cursor handling instead of browser Pointer Lock.

## Shell Choice

Use Tauri first for this macOS spike. The native side is Rust, the repo already has a Rust server,
and the spike needs direct macOS API calls more than it needs Electron's mature Chromium packaging.
Electron remains a fallback only if Tauri blocks the native macOS event handling or packaging shape
needed for the test.

This plan must not repeat the previous failed Tauri approach. The previous failure was a shell plus
laggy cursor handling and Pointer Lock fallback; this spike is specifically a native macOS cursor
backend with Pointer Lock removed from the tested path.

## Latency Contract

- Do not intentionally accumulate mouse deltas for later consumption.
- Do not update the visible cursor once per animation frame.
- Do not make native cursor movement wait for Pixi rendering, server snapshots, `camera.update`, or
  the match render loop.
- Do not flood synthetic DOM `mousemove` events as the main design. If JS receives native input, use
  an explicit native-input seam and tests that can inspect event counts and ordering.
- Move the visible cursor on each native event or via an OS/native overlay that is independent of JS
  frame cadence.
- If a platform API itself coalesces hardware samples before delivery, document that as observed OS
  behavior, not as a design choice.
- If JS/WebView delivery is the bottleneck, keep the native cursor visual responsive and record the
  exact gameplay actions still delayed by JS.

## Cross-Phase Constraints

- Keep the browser client and normal browser play working. The desktop path must be feature-gated
  and must not remove the current Pointer Lock fallback for ordinary web clients.
- Keep the server authoritative. This plan changes local input/cursor plumbing only; it must not
  change sim rules, fog authority, protocol command authority, balance, or match outcome logic.
- Keep the existing `Input` ownership model. `Match` should continue composing input dependencies;
  avoid non-shell cross-area imports unless the client architecture checker is intentionally updated
  with a reason.
- Keep client teardown strict. Any native listeners, app-window listeners, overlay resources, timers,
  Tauri event subscriptions, or WebView bridges must have explicit release paths on match destroy,
  window blur, Escape, app deactivate, and process shutdown.
- Keep the first spike macOS-only. Do not design a Windows or Linux abstraction until the Mac path
  proves the idea.
- Keep scope local. Do not add auto-update, signing/notarization, cross-platform installers, release
  workflows, or broad packaging polish in this spike.
- Prefer hard evidence over architecture optimism. Each phase should leave enough logs, measurements,
  or manual notes for the next agent to decide whether to continue.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite merge with the phase head reachable from `origin/main`
  before the next phase starts.
- After implementing each phase, the implementing agent must provide a handoff message describing
  what changed, what the next agent should do, and what should be manually tested. Manual testing
  notes should cover core behavior, not an exhaustive matrix.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.

## Key Risks

- macOS may require accessibility/input-monitoring permission or foreground-only behavior for the
  exact API path that feels good.
- WKWebView or Tauri IPC may add too much latency for JS-side hover/gameplay input even if native
  cursor drawing is instant.
- A native cursor overlay can make the cursor feel good while gameplay actions still wait for a busy
  JS thread; the spike must distinguish visual cursor latency from command/click latency.
- Tauri may be lightweight but less mature for unusual game input. If it blocks the native event
  path, stop or switch shell rather than bending the input design around Tauri.
- Any cursor capture path that fails to restore the cursor on blur/deactivate is unacceptable for a
  desktop app, even if it feels good while active.

## Suggested Phase Runner Usage

After this plan is approved, run phases one at a time from a clean checkout:

```bash
scripts/phase-runner.sh --plan maccursor 1 --pr --wait
scripts/phase-runner.sh --plan maccursor 2 --pr --wait
scripts/phase-runner.sh --plan maccursor 3 --pr --wait
scripts/phase-runner.sh --plan maccursor 4 --pr --wait
```

Stop after any phase whose handoff says the native path failed the latency or cleanup gate.
