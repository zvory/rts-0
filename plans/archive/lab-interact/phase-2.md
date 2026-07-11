# Phase 2 - Stateful Interaction Contract

Status: done.

Migration note: the bounded command schemas, aliases, session behavior, and integration smoke are
completed reusable work. Their former project-scoped MCP adapter is superseded and unsupported;
Phase 0 deletes it and maps the retained contract onto the `lab-interact` CLI and daemon IPC.

## Goal

Provide a typed, stateful interaction contract over the Phase 1 driver so agents can construct and
puppet bounded scenes through stable aliases without page internals or manual browser interaction.
The current supported architecture exposes this contract as CLI subcommands handled by one
per-worktree daemon.

## Delivered Contract

- Lifecycle: `open`, `status`, `reset`, `close`, and `shutdown` with the distinct session/daemon
  semantics established by Phase 0.
- `open` returns an unguessable opaque `sessionId`; every session-scoped command requires it, and
  the daemon rejects missing, stale, or foreign ids despite permitting only one live session.
- Discovery and observation: `catalog` and `inspect` with bounded category/filter/limit inputs and
  `truncated` metadata.
- Setup mutation: `spawn`, `update`, and `remove`, including a closed update union for move, owner,
  resources, research, and god mode rather than arbitrary patches.
- Gameplay and time: `order` through validated existing protocol commands, plus bounded
  pause/resume/speed/step/seek control.
- Presentation: `camera` center/zoom/focus with bounded entity lists and returned world bounds.
- A daemon-local alias table accepts safe aliases or numeric ids, returns resolved forms, rejects
  duplicates, and never guesses for stale or ambiguous references.
- Expected domain failures return concise correctable structured errors; initialization,
  corruption, or IPC failure remain distinct infrastructure errors.
- Inspection never returns full checkpoints, snapshots, terrain arrays, unbounded logs, or base64
  assets.
- Schemas validate unknown fields, numeric bounds, batches, aliases, command shapes, and structured
  result envelopes before the server remains final authority.

## Phase 0 Transport Mapping

The migration must preserve command semantics while changing only the local agent transport:

```text
lab-interact <command> [bounded options or validated JSON]
  -> private versioned local IPC
    -> one worktree daemon and alias table
      -> LabInteractDriver method
```

Commands between `open` and `close` reuse the same daemon-owned browser/game session. Callers copy
the `open` result's `sessionId` into later command JSON; canonical worktree identity selects the
daemon while the opaque id selects its current session generation.

## Delivered Touch Points

- bounded operation schemas and alias/session adapter logic, renamed/reused by the CLI daemon
- focused schema and driver integration tests
- end-to-end catalog/spawn/order/time/inspect/reset/close smoke coverage
- setup and troubleshooting documentation, rewritten by Phase 0 for the CLI

The superseded transport entry point, Codex project registration, transport-only dependencies,
annotations/instructions, and client-harness tests are deletion targets in Phase 0, not supported
compatibility surfaces.

## Constraints

- Do not expose generic execute, evaluate, message, state-patch, filesystem, shell, or browser
  navigation commands.
- Do not let aliases enter the game protocol, checkpoint, or replay schema.
- Do not accept arbitrary command JSON without validating it against the mirrored command schema
  and the daemon session's visible catalog/capabilities.
- Keep ephemeral scene changes distinct from repository writes in diagnostics.
- Preserve strict stdout JSON discipline in the CLI; logs and child-process output go to stderr or
  bounded ignored files.
- The daemon must release its sole session on `close`; signals, unrecoverable failure, `shutdown`,
  or the Phase 0 interaction-idle deadline must additionally remove daemon runtime state.

## Verification To Preserve During Migration

- Schema tests for valid/invalid inputs, unknown fields, numeric bounds, alias syntax, duplicate
  and stale aliases, missing/stale/foreign session ids, batch caps, and structured output shapes.
- A direct CLI contract smoke that invokes every read-only command without Codex-specific client
  discovery.
- An end-to-end smoke starting from no daemon, opening, cataloging tanks, spawning aliased
  `shooter` and `target`, focusing, ordering, stepping, inspecting, resetting, closing, and shutting
  down.
- stdout purity, bounded logs, cross-worktree isolation, stale daemon recovery, and idle teardown
  verification.
- `node scripts/check-docs-health.mjs` and `node tests/select-suites.mjs --verify` when mapped files
  change.

## Manual Testing Focus

- From a fresh shell, use only CLI commands to create, command, and inspect a two-unit scene while
  confirming aliases persist across invocations.
- Confirm no Browser Use, Computer Use, or Codex MCP discovery/configuration is needed.
- Start commands in two worktrees and confirm each reaches only its own single Lab session.

## Handoff Record

The bounded interaction vocabulary and aliases are complete underlying MVP work. Phase 0 must
remove the obsolete transport/configuration, preserve schemas and authoritative evidence, decide
the final CLI flag/JSON grammar and result envelope, and add daemon startup, IPC, runtime cleanup,
and 30-minute interaction-idle coverage before the screenshot workflow is re-reviewed.
