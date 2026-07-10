# Phase 2 - Project-Scoped Interactive MCP

Status: done.

## Goal

Expose the Phase 1 driver to Codex as a local, stateful, project-scoped MCP server. Agents should be
able to construct and puppet a bounded lab scene through typed tool calls and stable aliases without
shell flags, page internals, or manual browser interaction.

## Scope

- Add a stdio MCP server that owns a bounded set of `AgentLabDriver` sessions. Keep MCP transport,
  validation schemas, tool annotations, and server instructions in the adapter; do not move them
  into client or simulation code.
- Pin the current stable MCP TypeScript SDK generation and schema dependency in a checked-in
  lockfile. Reuse the repository's local Node dependency hydration strategy and avoid runtime
  package downloads.
- Add trusted project-scoped `.codex/config.toml` registration for the stdio server, with a
  deliberate command, arguments, working directory behavior, startup/tool timeouts, enabled tool
  allowlist, and approval defaults. Document that a fresh thread/reload may be required after this
  phase first lands.
- Supply concise MCP server instructions that tell the agent to open a session, inspect the
  catalog, use aliases, keep scenes small, confirm authoritative results, and close sessions. The
  first 512 characters should contain the essential workflow and safety boundary.
- Support explicit `sessionId` values even if the first implementation normally uses one session.
  Bound concurrent sessions and idle lifetime so different threads or failed tasks do not silently
  control the same room forever.
- Maintain a per-session alias table. `lab_spawn` may assign an optional safe alias; later tools
  accept either aliases or numeric ids and return both resolved forms. Reject duplicate aliases and
  never guess when a reference is ambiguous or stale.
- Expose these initial tools with input and structured output schemas:
  - `lab_open`: workspace root, map, seed, optional bundled scenario, viewport; returns session,
    worktree/build facts, players, tick, and capabilities.
  - `lab_close`: idempotently closes one session and its owned processes when no session reuses
    them.
  - `lab_reset`: restores the current setup baseline and reconciles aliases/remaps deliberately.
  - `lab_catalog`: returns requested bounded categories such as maps, factions, units, buildings,
    upgrades, players, commands, and abilities.
  - `lab_spawn`: spawns one or a small bounded batch, supports optional aliases, and returns
    authoritative ids/outcomes.
  - `lab_update`: a closed union for move, owner, resources, research, and god-mode operations;
    do not accept arbitrary key/value patches.
  - `lab_remove`: removes a bounded list of aliases/ids and clears their aliases.
  - `lab_order`: resolves aliases into an existing protocol command and sends it through
    `issueCommandAs`; return command acceptance and observed order/tick evidence.
  - `lab_time`: pause/resume/speed/step/seek with bounded values and authoritative room-time state.
  - `lab_inspect`: filters by aliases/ids, kind, owner, or camera viewport and returns concise
    entity/player/room summaries with `truncated` metadata.
  - `lab_camera`: sets center/zoom or focuses a bounded list of entities with padding; returns the
    applied camera/world bounds.
- Return expected domain failures as MCP tool errors with concise corrective details. Reserve MCP
  server failure for initialization, transport, or unrecoverable process corruption.
- Annotate read-only tools (`lab_catalog`, `lab_inspect`) separately from ephemeral mutating tools.
  Make descriptions explicit that mutations affect only the private lab session and do not edit
  repository files.
- Add bounded structured logging to stderr/MCP logging; never write ordinary logs to stdout because
  stdio is reserved for MCP messages.
- Close all sessions when the MCP transport shuts down or receives termination signals.

## Expected Touch Points

- a focused MCP entry point and schemas under the Phase 1 agent-lab tooling area
- `.codex/config.toml`
- `tests/package.json` and lockfile, or the dedicated agent-lab package/lockfile chosen in Phase 1
- MCP client contract tests and live driver/MCP smoke coverage
- a short setup/troubleshooting document under `docs/`
- `docs/context/testing.md` if it gains a stable verification command

## Constraints

- Do not add screenshots, video, setup/replay export, checked-in scenario authoring, or a reusable
  Codex skill yet.
- Do not expose a generic `execute`, `evaluate`, `sendMessage`, `setState`, filesystem path, shell,
  or browser-navigation tool.
- Do not let MCP aliases enter the game protocol or checkpoint format. They are adapter state.
- Do not return full checkpoint payloads, full snapshots, terrain arrays, unbounded event logs, or
  base64 assets from inspection tools.
- Do not accept arbitrary command JSON without validating it against the mirrored command schema
  and the catalog/capabilities visible to the session. The server remains the final authority.
- Do not mark ephemeral setup operations as repository writes in user-facing text, but do preserve
  truthful MCP side-effect annotations and configurable approval behavior.
- Keep tool names stable after this phase unless the Phase 3 manual review finds a concrete
  usability problem.

## Verification

- Add schema tests covering accepted/rejected inputs, unknown fields, numeric bounds, alias syntax,
  duplicate aliases, stale aliases, batch caps, and structured output shapes.
- Launch the MCP server through an SDK client harness, list tools, inspect server instructions, and
  call every read-only tool without using Codex itself.
- Add an end-to-end MCP smoke that opens a private session, catalogs tanks, spawns aliased
  `shooter` and `target`, focuses them, issues an attack or move command, steps time, inspects the
  resulting state, resets, and closes.
- Confirm stdout contains only valid MCP protocol traffic and child-process logs are routed
  elsewhere.
- Verify an idle/aborted client connection tears down sessions within the documented bound.
- Run `node scripts/check-docs-health.mjs` and `node tests/select-suites.mjs --verify` if mapped
  files change.

## Manual Testing Focus

- In a fresh Codex task after MCP configuration reload, ask the agent to list Agent Lab tools,
  create a two-unit scene using aliases, order one unit, inspect the outcome, and close it.
- Confirm no Browser Use or Computer Use approval/surface appears; all interaction should be MCP
  calls and local artifacts/logs.
- Start two bounded sessions or simulate a stale session id and confirm calls cannot cross-control
  the wrong lab.

## Handoff

After implementation, mark this phase done and report the final MCP tool names/schemas,
configuration/reload steps, alias rules, session limits, approval annotations, SDK version, and
focused verification. Identify any vocabulary or lifecycle rough edges that Phase 3 should address
before documenting the workflow as the graphics-review default.
