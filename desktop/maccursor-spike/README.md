# macOS Native Cursor Capture Spike

This is a disposable macOS-only harness for `plans/archive/maccursor/phase-1.md`. It
does not use the game client, browser Pointer Lock, Tauri, a WebView, or the
RTS render loop.

Run it from this directory:

```bash
./run.sh
```

The runner builds `.build/MacCursorSpike.app` locally and launches it with
`open -W` so AppKit treats the harness as a foreground app.

Run the targeted smoke check without opening the interactive window:

```bash
./run.sh --self-test
```

`--self-test` compiles the same app binary and checks event movement/cleanup
logic without taking over the desktop cursor. Use the interactive command above
for the real foreground-only CoreGraphics capture test.

The harness uses AppKit local mouse events plus CoreGraphics cursor APIs:

- `CGDisplayHideCursor(CGMainDisplayID())` hides the system cursor while capture
  is active.
- `CGAssociateMouseAndMouseCursorPosition(0)` disconnects mouse movement from
  the system cursor position.
- `NSEvent.addLocalMonitorForEvents` receives foreground mouse movement and
  drag events.
- The marker is drawn by an `NSView` and forced to redraw synchronously on each
  native mouse event with `display()`, so it is not tied to
  `requestAnimationFrame`, Pixi, or a WebView frame cadence.
- `CGAssociateMouseAndMouseCursorPosition(1)` and
  `CGDisplayShowCursor(CGMainDisplayID())` restore normal cursor behavior.

Cleanup paths are wired for Escape, window blur, app deactivation, window close,
normal app termination, and best-effort `SIGINT`/`SIGTERM` restoration. This
harness intentionally has no WebView, so there is no JS stall test in Phase 1;
Phase 3 should add that only if it routes native input through the desktop
shell/client seam.

Manual checks:

1. Launch `./run.sh`.
2. Confirm the system cursor disappears and the yellow marker is visible.
3. Move the mouse in fast circles and short flicks; the marker should redraw on
   each native mouse event and the event counter should climb.
4. Press Escape and confirm the normal cursor returns.
5. Relaunch or press Space to capture again, then Command-Tab away and confirm
   the normal cursor returns.
6. Capture again, close the window, and confirm the normal cursor returns.
