# Agent Lab MCP

Agent Lab MCP is a project-local stdio server for arranging and inspecting small authoritative lab
scenes without Browser Use or Computer Use. It starts the selected worktree's normal Rust server and
headless Pixi client; all mutations remain private to that lab session and never edit repository
files, source assets, scenarios, checkpoints, commits, or PRs.

## Setup

The checked-in [`.codex/config.toml`](../.codex/config.toml) registers `agent_lab` with the pinned
local Node dependencies. After this configuration first lands, reload Codex or start a fresh task so
the project-scoped MCP server is discovered. The project must be trusted for Codex to load a local
`.codex/config.toml`.

The server launches from the active worktree (`cwd = "."`) and `lab_open` accepts only that same Git
top level. This ensures a graphics change is rendered from the intended worktree rather than another
checkout. It uses `@modelcontextprotocol/sdk` 1.29.0 with `zod` 4.4.3, locked in the root
`tests/package-lock.json`; the adapter reuses the lockfile-keyed test dependency hydration used by
the browser driver. Run `cd tests && npm ci --ignore-scripts --no-audit --fund=false` once if that
cache has not been hydrated. Chrome/Chromium remains a local driver requirement; set
`CHROME=/path/to/browser` when automatic discovery cannot find it.

## Normal flow

1. Call `lab_open`, then save its `sessionId`.
2. Call `lab_catalog` before selecting player ids, kinds, upgrades, abilities, or command kinds.
3. Use optional short aliases such as `shooter` and `target` in `lab_spawn`; later entity inputs
   accept either those aliases or numeric ids.
4. Keep the scene to a few entities. Use `lab_order`, `lab_time`, and `lab_inspect` to confirm the
   authoritative result, then use `lab_camera` to set up a later capture.
5. Call `lab_close` when finished. It is idempotent and stops the owned browser/server processes.

The initial tool surface is `lab_open`, `lab_close`, `lab_reset`, `lab_catalog`, `lab_spawn`,
`lab_update`, `lab_remove`, `lab_order`, `lab_time`, `lab_inspect`, and `lab_camera`. It deliberately
does not provide screenshots, video, filesystem access, generic evaluation, arbitrary WebSocket
messages, raw checkpoint editing, or arbitrary command JSON. Screenshot/artifact tooling belongs to
the next phase.

## Bounds and alias rules

- At most two sessions may be open at once; an unused session is closed after five minutes.
- Aliases match `[A-Za-z][A-Za-z0-9_-]{0,31}`, are unique within one session, and never cross into
  the game protocol or checkpoint format.
- An unknown, duplicate, stale, or cross-session alias is a tool error; aliases are never guessed.
- `lab_reset` reconciles aliases only to one exact authoritative post-reset match (kind, owner, and
  position). Ambiguous or missing aliases are cleared and returned in `clearedAliases`.
- `lab_inspect` is capped at 100 concise entities. `lab_spawn` is capped at 10 entities per call;
  entity lists and commands are similarly bounded by the tool schemas.

## Troubleshooting

| Error | Correction |
| --- | --- |
| `workspaceNotAllowed` | Start the project-local server from the desired worktree; it cannot control another checkout. |
| `sessionLimit` or `unknownSession` | Close a completed session or call `lab_open` for a new one. |
| `unknownAlias` / `staleAlias` | Use `lab_inspect`, then use a current numeric id or create a new alias. |
| `invalidKind`, `invalidUpgrade`, or `invalidAbility` | Query `lab_catalog` and use an id exposed by that session. |
| `chromeUnavailable` | Install Chrome/Chromium locally or set `CHROME` to its executable. |
| `snapshotTimeout` / `labRejected` | Inspect the concise error, correct the request, and retry; the server remains authoritative. |

## Focused verification

```bash
cd tests && npm ci --ignore-scripts --no-audit --fund=false
node tests/agent_lab_mcp_contracts.mjs
node tests/agent_lab_mcp_smoke.mjs
```

The contract harness starts the real stdio entry point with a deterministic driver fixture and checks
schemas, tool annotations, stdout framing, aliases, session bounds, and connection cleanup. The smoke
starts a private Rust server and normal headless client through MCP, builds a two-rifleman scene,
issues a normal order, steps time, inspects authoritative state, resets, and closes.
