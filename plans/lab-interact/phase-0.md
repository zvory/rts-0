# Phase 0 - CLI Migration And Deep Rename

Status: done.

## Goal

Replace the unusable project-scoped MCP integration with `lab-interact`, a stateful command-line
client backed by an automatically managed per-worktree daemon. Remove the failed adapter and its
Codex configuration, deeply rename the existing agent-only tooling, and retain the authoritative
driver, browser, game session, aliases, and screenshot capability already delivered by Phases 1-3.

## Scope

- Remove the project-scoped MCP server entry point, protocol transport, SDK/schema dependencies
  that are unused after migration, MCP-only tests, and repository `.codex/config.toml`
  registration. Do not leave a compatibility server or advertise MCP as a supported path.
- Deeply rename the agent tool from Agent Lab/agentlab/agent-lab to Lab Interact/lab-interact in
  source modules, exported classes, bridge names, launch parameters, scripts, tests, documentation,
  skill paths, target artifact directories, manifests, and user-facing diagnostics. Preserve the
  game's ordinary human-facing Lab feature names and wire types where they describe the product
  rather than this agent tool.
- Rename `AgentLabDriver` and `AgentLabBridge` to their `LabInteract` equivalents and update every
  caller. Preserve the driver/bridge authority and dependency-injection boundaries proven in
  Phase 1.
- Add a repository-owned `lab-interact` CLI with discrete, scriptable commands for session
  lifecycle, catalog, spawn, update, remove, order, time, inspect, camera, and screenshot. Use
  structured JSON on stdout, actionable diagnostics on stderr, stable exit codes, strict schemas,
  and safe aliases; do not introduce a general evaluation or browser-control command.
- On the first command in a worktree, automatically start one background daemon for that Git top
  level, then connect the CLI to it over a private local IPC endpoint. `open` must create the
  authoritative Lab session and start its Rust server and Chrome without manual setup.
- Make the daemon own exactly one authoritative interaction surface per worktree: one driver, one
  private Rust server, one headless Chrome instance, one page, and one lab room/session. Sequential
  CLI invocations with the current opaque `sessionId` must observe and manipulate that same state
  until reset, close, shutdown, or idle teardown.
- Have `open` return an unguessable opaque `sessionId`, enforce `maxSessions: 1` per worktree, and
  require that id on every session-scoped command. Reject stale, missing, or foreign ids so delayed
  commands cannot act on a replacement session after close/reopen.
- Derive runtime identity from the canonical Git worktree root so different worktrees cannot
  cross-control one another. Use a private per-user runtime directory, restrictive permissions,
  atomic metadata/lock creation, a versioned IPC contract, and a capability or equivalent
  authentication check so unrelated local processes cannot issue commands accidentally.
- Define interaction idle as elapsed monotonic time since the daemon last completed or began
  handling a valid CLI request. After 30 minutes without interaction, the daemon must close the
  room/page/browser/server through the driver's idempotent teardown, remove its IPC socket, pid,
  lock, capability, and transient runtime files, and exit.
- Reset the idle deadline on every valid request, including read-only catalog/inspect/status calls,
  but not on internal browser frames, WebSocket traffic, health checks, or simulation activity.
  Do not tear down during an in-flight command; re-arm the deadline after it completes.
- Make startup race-safe and self-healing. Concurrent first commands must converge on one daemon;
  stale metadata, dead pids, incompatible daemon versions, half-created sockets, and startup
  failures must be detected, cleaned within the worktree-owned runtime namespace, and retried or
  reported with a corrective error.
- Keep explicit `open`, `status`, `reset`, `close`, and `shutdown` commands. `open` idempotently
  creates or returns the sole session; `close` idempotently releases that session's Lab room, page,
  browser, profile, private Rust server, aliases, and pending work while leaving the daemon/IPC
  ready. `shutdown` performs immediate full session and daemon/runtime-file teardown, and a later
  `open` auto-starts a fresh daemon if necessary.
- Move ignored outputs to `<worktree>/target/lab-interact/` and update path confinement,
  `.gitignore`, manifests, tests, the project skill, AGENTS guidance, and relevant design/context
  documentation. Existing ignored `target/agent-lab/` artifacts may be left as untracked historical
  output, but no current code or documentation should write there.
- Provide a short CLI workflow reference with copyable commands and JSON examples. The graphics
  workflow must call the CLI, inspect the returned PNG path once with the local image viewer, and
  share the path; it must not depend on Codex MCP discovery or image-content returns.

## Command Contract

The exact parser may be selected during implementation, but the supported shape should remain
small and composable:

```text
lab-interact open
lab-interact status
lab-interact reset
lab-interact close
lab-interact shutdown
lab-interact catalog [options]
lab-interact spawn [options]
lab-interact update [options]
lab-interact remove [options]
lab-interact order [options]
lab-interact time [options]
lab-interact inspect [options]
lab-interact camera [options]
lab-interact screenshot [options]
```

Commands should accept bounded flags or JSON from a deliberate argument/file/stdin contract. They
must never accept arbitrary JavaScript, shell, browser navigation, unrestricted filesystem paths,
or unvalidated protocol messages.

## Expected Touch Points

- the existing driver, bridge, adapter, schemas, launch gate, and screenshot code under their new
  `lab-interact` names
- the new CLI entry point, daemon process, local IPC schemas, runtime registry, lifecycle helpers,
  and focused contract/integration tests
- deletion of the MCP entry point, MCP-only dependencies/tests, and `.codex/config.toml`
  registration
- `client/src/app.js`, `client/src/bootstrap.js`, and renamed bridge imports/launch parameters
- `.gitignore`, `AGENTS.md`, `.agents/skills/lab-interact/`, and deletion of the old skill path
- `docs/design/client-ui.md`, relevant `docs/context/` capsules, and CLI troubleshooting guidance
- test selection/package lockfiles only where dependency or routing changes require them

## Constraints

- Do not change the authoritative Lab wire protocol merely to replace the local agent transport.
  Keep the Rust server, room task, and normal client as the authority boundary.
- Do not create one daemon per command or multiple browser/game surfaces per worktree. Persistence
  between `open` and `close` is the reason the daemon exists.
- Do not bind a network control port, expose a production endpoint, or make the daemon reachable
  outside the local user's machine. Prefer local IPC with private filesystem permissions.
- Do not treat daemon uptime, simulation ticks, WebSocket activity, or browser rendering as user
  interaction for the 30-minute idle deadline.
- Do not silently attach to a daemon for a different canonical worktree, repository head contract,
  or incompatible CLI/IPC version.
- Do not delete source artifacts or unrelated processes during stale-state recovery. Cleanup must
  be restricted to resources proven to belong to the current worktree's Lab Interact runtime.
- Keep stdout machine-readable. Logs, startup progress, and remediation guidance belong on stderr
  or in bounded files under the ignored target root.
- Preserve all existing input bounds, alias ambiguity checks, snapshot confirmation, capture
  readiness, output confinement, and panic/fog/client-trust invariants.

## Verification

- Add CLI parser/schema tests for every command, required/unknown fields, numeric and batch bounds,
  aliases, required session ids, structured JSON success/error envelopes, stdout purity, and exit
  codes.
- Add daemon registry/IPC tests for canonical worktree identity, private permissions, capability
  checks, version mismatch, concurrent first use, stale pid/socket/lock recovery, and cross-worktree
  isolation.
- Add lifecycle tests with an injectable short idle duration proving valid requests reset the
  deadline, internal game activity does not, and in-flight commands are not interrupted. Prove
  `close` releases all session children but keeps a responsive daemon, while `shutdown` and idle
  expiry remove every runtime file and owned child process.
- Add an end-to-end CLI smoke that starts from no daemon, opens, copies its `sessionId` into later
  command JSON, catalogs units, spawns aliased `shooter` and `target`, focuses the camera, issues an
  order, steps time, inspects authoritative state,
  captures a PNG under `target/lab-interact/`, resets, closes, verifies session children are gone,
  then shuts down and verifies the daemon/runtime files are gone. Include a stale-id rejection
  after close/reopen.
- Run the existing driver, client architecture, client contract, and screenshot smokes under their
  renamed paths and verify no behavior regression.
- Search tracked source and active documentation for stale Agent Lab tool names, `agentlab`,
  `agent-lab`, MCP server/config/SDK references, and `target/agent-lab`; review exceptions so only
  human Lab terminology or historical evidence remains.
- Run `node scripts/check-docs-health.mjs` and `node tests/select-suites.mjs --verify`.

## Manual Testing Focus

- From a clean shell in a non-main worktree, run `open` and confirm the daemon, server, Chrome, page,
  and room appear automatically. Copy its `sessionId` into later commands and confirm they retain
  the same aliases and authoritative scene.
- Create and capture a two-unit scene using only CLI commands, inspect the PNG once, and confirm its
  manifest identifies the selected worktree and new `target/lab-interact/` path.
- Exercise concurrent first use, close/reopen, shutdown/restart, and a test-configured short idle
  timeout. Confirm close leaves only the ready daemon, while shutdown and idle leave no socket,
  metadata, Chrome, Rust server, or temporary browser profile.

## Handoff

After implementation, mark this phase done and report the final command grammar and JSON envelope,
opaque session-id rules and one-session cap, IPC/runtime locations and permissions, worktree
identity, startup concurrency behavior, 30-minute idle semantics, close/shutdown/idle teardown
evidence, deep-rename exceptions, deleted MCP surfaces, and focused verification. Confirm that
Phases 1-3 still pass under the CLI/daemon surface and identify whether the Phase 3 review gate is
ready for a fresh CLI-based manual review before Phase 4.
