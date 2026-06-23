# Phase 3 - Basic Logs And Failure Surfaces

## Phase Status

- [ ] Planned.

## Plain-Language Summary

The shell needs enough logging to support playtesters without a terminal. It should persist shell
startup events, selected server profile, local server output, and native cursor failures. Startup
failures should show a simple in-app error with a way to find or copy the relevant logs.

## Objective

Add basic app-local diagnostics for startup, server selection, local server launch, connection, and
native cursor capture failures.

## Scope

- Create an app log directory under the user's macOS app data or logs directory.
- Write a shell log with timestamped startup events, selected server profile id/URL, app version,
  build id if available, packaged/dev mode, and local server lifecycle events.
- Capture local server stdout/stderr to a persistent log file in packaged and dev local modes.
- Add a small in-app startup failure surface for invalid server URLs, failed local server launch,
  server-ready timeout, remote navigation rejection, and native cursor capture start failure.
- Add a way to reveal/copy the log path from the startup screen or error surface.
- Optionally bridge a small set of desktop-runtime JS errors into the shell log, especially native
  cursor bridge failures and autostart/autolock failures.
- Keep logs local. Do not add upload, telemetry, or external crash reporting.
- Avoid logging secrets or full custom URL query strings if future URLs may carry tokens.

## Expected Touch Points

- `desktop/maccursor-shell/src-tauri/src/main.rs`
- `desktop/maccursor-shell/src-tauri/src/native_cursor.rs`
- startup UI files from Phase 1
- `desktop/maccursor-shell/README.md`
- Tauri permissions if a reveal-log command is added

Avoid touching:

- server structured log semantics
- Fly log tooling
- match-history persistence

## Verification

- Run `cargo test --manifest-path desktop/maccursor-shell/src-tauri/Cargo.toml`.
- Add focused tests for log path selection, URL redaction if implemented, and startup failure state
  formatting.
- Manually force at least one bad URL and one local-server startup failure to confirm an in-app
  error and log file are produced.

## Manual Testing Focus

Trigger a bad custom URL and a failed local launch, then confirm the app explains the failure and
points to logs.

## Handoff Expectations

The handoff must state the log directory, file names, what events are logged, what is intentionally
not logged, and how a tester can expose logs for support.
