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
node scripts/lab-interact/cli.mjs close '{"sessionId":"<id>"}'
node scripts/lab-interact/cli.mjs shutdown
```

The complete surface is `open`, `close`, `reset`, `catalog`, `spawn`, `update`, `remove`, `order`,
`time`, `inspect`, `camera`, `screenshot`, `status`, and `shutdown`. Success writes exactly one JSON
envelope to stdout. Failure writes a concise JSON error to stderr and exits nonzero. Every command
has an exact, bounded input shape; arbitrary state patches, protocol messages, browser evaluation,
and caller-selected artifact paths are not accepted.

`open` returns the `sessionId` required by session commands. Optional aliases match
`[A-Za-z][A-Za-z0-9_-]{0,31}` and remain private to that session. Unknown, duplicate, stale, or
cross-session aliases are rejected rather than guessed. Only one authoritative session may be open
per worktree.

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

Artifacts are confined to `target/lab-interact/<session-id>/captures/` and ignored by Git. For a
single-unit detail capture, camera `focus` defaults to close 32-world-pixel padding. Multi-subject
and non-unit focus defaults to 48 world pixels.

## Recovery

| Error | Correction |
| --- | --- |
| `unknownSession` | Run `open` and use its current session id. |
| `sessionLimit` | Close the current worktree session before opening another. |
| `unknownAlias` / `staleAlias` | Inspect current state or create a new alias. |
| `invalidKind`, `invalidUpgrade`, or `invalidAbility` | Query `catalog` and use an exposed id. |
| `chromeUnavailable` | Install Chrome/Chromium or set `CHROME` before `open`. |
| `daemonIncompatible` | Let the prior daemon exit or stop its recorded pid, then retry. |
| `assetLoadFailed`, `captureRenderError`, or `captureTimeout` | Fix the reported source/render problem; do not accept a fallback capture. |

## Focused verification

```bash
node tests/lab_interact_cli_contracts.mjs
node tests/lab_interact_driver_contracts.mjs
node tests/lab_interact_cli_smoke.mjs
node tests/lab_interact_driver_smoke.mjs
```
