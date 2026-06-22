# Phase 1 - Native Capture Harness

## Phase Status

- [ ] Planned.

## Plain-Language Summary

Build a tiny macOS-native cursor capture test before touching the actual game. The test should show
a cursor-like marker that moves immediately on each native mouse event, while the system cursor is
hidden or disconnected and then restored safely. This phase is a failure if cursor movement is
batched to animation frames, accumulated for later, or visibly waits on a busy WebView/JS frame.

## Objective

Prove or disprove the core macOS input idea in the smallest possible harness: native cursor capture,
native movement delivery, native visual cursor update, and reliable cleanup.

## Scope

- Add a macOS-only spike harness under a clearly marked desktop/spike path.
- Prefer Tauri for the harness unless a smaller direct Rust/AppKit harness is faster and easier to
  delete or fold into Tauri.
- Use macOS-native APIs to enter and exit capture mode.
  - Candidate APIs include CoreGraphics cursor association/disassociation, cursor hide/show, and
    AppKit/CoreGraphics event handling.
  - Use foreground-only behavior; do not try to capture while the app is not active.
- Render a native test cursor/marker outside the game render loop.
- Move that marker on each native mouse movement event.
- Include a visible or logged event counter and timestamp delta so input delivery can be inspected.
- Add cleanup on Escape, app/window deactivate, panic/error paths where practical, and normal app
  quit.
- Add an intentional WebView/JS load or stall test if the harness contains a WebView, so we can see
  whether the native marker keeps moving when JS is busy.

## Non-Negotiable Latency Rules

- Do not accumulate deltas for a later frame.
- Do not poll native deltas once per `requestAnimationFrame`.
- Do not make the visual cursor a DOM element updated by the game render loop.
- Do not use browser Pointer Lock as the success path.

## Expected Touch Points

- A new macOS-only spike directory, for example `desktop/maccursor-spike/` or
  `src-tauri-maccursor-spike/`.
- `package.json` or a spike-local package manifest only if needed to launch the harness.
- Minimal docs in this phase file or the plan handoff describing how to run the harness.

Avoid touching:

- `server/crates/*`
- `server/src/lobby/*`
- `client/src/input/*`
- `client/src/match.js`
- production packaging scripts

## Verification

- Run the harness on the available Mac.
- Confirm entering capture hides or disconnects the system cursor and displays the native test
  marker.
- Confirm mouse movement updates the native marker immediately while the app is active.
- Confirm Escape exits capture and restores ordinary cursor behavior.
- Confirm Command-Tab, app deactivate, and window close restore ordinary cursor behavior.
- If a WebView is present, run an artificial JS stall and confirm whether native marker movement
  remains responsive.
- Record the observed event-to-marker latency if the harness can measure it; otherwise record a
  plain-language playtest observation.

## Manual Testing Focus

Move the mouse quickly in circles and short back-and-forth flicks while capture is active. The marker
should feel glued to the mouse, and the normal cursor must come back reliably after Escape,
Command-Tab, and closing the window.

## Handoff Expectations

The handoff must state which macOS API path was used, whether Tauri was used, whether JS/WebView load
affected native marker movement, how cleanup behaved, and whether Phase 2 should proceed. Include
the exact run command and any permission prompts observed on macOS.
