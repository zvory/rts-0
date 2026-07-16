# Pointer Lock Lab

An intentionally isolated Tauri app for testing browser Pointer Lock in a native WebView. It has
no game code, network access, remote URLs, Tauri commands, or native cursor implementation.

The lab exposes two explicit paths:

- **Lock raw (goal)** calls `requestPointerLock({ unadjustedMovement: true })`.
- **Lock standard (control)** calls `requestPointerLock()`.

Keeping both paths visible makes automation failures legible. If standard lock receives synthetic
movement but raw lock does not, Pointer Lock itself works and the automation input is not reaching
WebView2's raw-input path.

## Run on Windows

From the repository root in PowerShell:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'rts-0\tauri-target-windows'
$env:CARGO_BUILD_JOBS = '2'
cargo run --manifest-path .\desktop\pointer-lock-lab\src-tauri\Cargo.toml
```

The built executable is
`$env:LOCALAPPDATA\rts-0\tauri-target-windows\debug\pointer-lock-lab.exe`.

## Agent test recipe

1. Launch the executable and target the window titled **Pointer Lock Lab**.
2. Click the large striped capture surface. A raw lock request happens directly in that click.
3. Confirm the headline says **LOCKED — RAW**.
4. Use a drag from the middle of the capture surface toward its right edge. A coordinate jump or a
   second click at the right edge can also work if the automation runtime only supports jumps.
5. Confirm **MOVEMENT DETECTED**, a positive last X delta, and **RIGHT** appear.
6. Press Escape, click **Lock standard (control)**, and repeat when raw movement produced no events.

The standard mode is a diagnostic control, not a fallback. The app never silently changes modes.

## Checks

```powershell
node --test .\desktop\pointer-lock-lab\tests\pointer_lock_lab.mjs
cargo check --manifest-path .\desktop\pointer-lock-lab\src-tauri\Cargo.toml
```
