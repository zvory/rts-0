# Lab Interact Plan

## Purpose

Give repository agents a small, stateful command-line interface for arranging and inspecting
authoritative game scenes without Computer Use, Browser Use, or Codex MCP connectivity. The public
surface is the repository-owned `lab-interact` CLI backed by one automatically started per-worktree
daemon, a reusable `LabInteractDriver`, a private Rust server, one headless normal client, stable
entity aliases, and bounded local artifacts. The existing human-facing Lab remains the product and
authority boundary: Lab Interact composes typed Lab operations, normal commands, room time,
checkpoint setups, and Lab replay artifacts instead of introducing a second simulation or raw
state mutation API.

Phases 1-3 already delivered the reusable driver, bounded interaction vocabulary, and screenshot
MVP, but their project-scoped MCP adapter cannot be reached reliably from Codex Desktop and is no
longer a supported architecture. Phase 0 is therefore the next implementation phase: it deletes
that adapter and configuration, deeply renames the agent tool to Lab Interact, and migrates the
existing capability to the CLI/daemon lifecycle. Phases 4-6 remain follow-up work for portable
scene artifacts, real-time video, and deterministic frame capture after the migrated MVP is
manually reviewed.

This plan refines the product direction in [the Lab architecture hypotheses](../lab/architecture.md).
That document remains the Lab product north star; this plan owns only the local agent-control and
media-capture adapter.

## Chosen Architecture

```text
Codex or another local shell caller
  -> lab-interact CLI (one bounded command, JSON result)
    -> private local IPC for the canonical Git worktree
      -> auto-started lab-interact daemon (30-minute interaction-idle lifetime)
        -> LabInteractDriver (process/session/alias/artifact ownership)
          -> one private loopback Rust server and authoritative Lab room
          -> one headless normal client with a narrow in-page bridge
          -> normal Pixi renderer
        -> bounded files under target/lab-interact/
```

The CLI is the only supported agent interaction surface. Any command can start the worktree daemon
automatically; `open` creates its sole authoritative browser/game session and returns an opaque
`sessionId`. Later session commands echo that id and reconnect to the same state so aliases and
scene state persist. `close` idempotently releases the session, browser, and private Rust server
while the daemon remains ready; `shutdown` or 30 minutes without a valid CLI interaction also
removes IPC and runtime metadata before the daemon exits.

The stable command vocabulary should remain small and composable:

- lifecycle: `open`, `status`, `reset`, `close`, `shutdown`;
- discovery and observation: `catalog`, `inspect`;
- setup mutation: `spawn`, `update`, `remove`;
- gameplay and time: `order`, `time`;
- presentation: `camera`, `screenshot`;
- later artifact/media commands: `export`, `import`, `record-start`, `record-stop`, and a clearly
  named deterministic-capture command.

The daemon permits at most one live session, but every session-scoped command must carry the opaque
`sessionId` returned by `open` so a stale caller cannot act on a replacement session. Commands may
return authoritative numeric entity ids, but callers should normally use daemon-local aliases
such as `shooter`, `target`, or `subject`. Alias resolution belongs to the daemon/driver and must
survive known checkpoint entity-id remaps; aliases are not wire fields, simulation state, or a new
persistence contract.

## Overall Constraints

- Run a real `Game` in a real Lab room and render through the normal client. Do not add a
  renderer-only fake entity model, parallel simulation, or second game client.
- Enter authoritative state only through existing typed Lab operations, normal `issueCommandAs`,
  room-time controls, or deliberately reviewed extensions to those seams. Never expose arbitrary
  object mutation, code evaluation, private `GameState` serialization, raw checkpoint editing,
  shell execution, or browser navigation through the CLI.
- Preserve the `Game` API seam, room task ownership, mirrored protocol, panic-free tick path, and
  authoritative fog/event projection. Any protocol addition must update both mirrors and
  `docs/design/protocol.md` in the same phase.
- Keep integration local. Use private local IPC plus loopback child services; do not add a
  production remote-control endpoint or make deployed rooms discoverable through Lab Interact.
- Maintain exactly one daemon, driver, browser, page, and authoritative Lab session per canonical
  worktree. Commands for different worktrees must never share state or cross-control rooms.
- Require the current opaque `sessionId` on session-scoped command JSON even with the one-session
  cap. Reject stale, missing, or foreign ids instead of routing them to whichever session is live.
- Start the daemon on first use without a separate daemon setup command. Concurrent starts must
  converge, and stale runtime state must be recoverable.
- `open` creates the sole authoritative session; `close` releases its room, page, browser, private
  server, profile, aliases, and pending operations but may leave the daemon and IPC ready.
  `shutdown` and 30 minutes without valid CLI interaction perform full session plus daemon/runtime
  teardown.
- Define interaction idle from valid CLI requests, not game ticks, browser frames, WebSocket
  traffic, or health probes. Never interrupt an in-flight request solely because its deadline
  elapsed.
- Render the explicitly selected task worktree. Resolve and validate its Git top level, start the
  server there, record its branch/head in artifacts, and never silently serve another checkout.
- Keep command and response sizes bounded. Inspection needs filters, limits, concise entity
  projections, and truncation metadata instead of a full snapshot or checkpoint payload.
- Treat checkpoint setups and `LabReplayArtifactV1` as opaque versioned artifacts. A static scene
  may export a checkpoint setup; timed mutations or commands should export a Lab replay.
- Restrict default outputs to `<worktree>/target/lab-interact/`, use safe generated names, and
  ignore that directory in Git. A capture/export command must not accept arbitrary output paths,
  write source assets, submit a PR, or add a bundled scenario.
- Own ports, browser profiles, runtime files, locks, child processes, timeouts, and teardown
  explicitly. Failed commands, startup races, stale metadata, client interruption, and idle expiry
  must not leak resources.
- Pin viewport and device-pixel ratio for repeatability and record browser/build details in
  manifests. Screenshots are visual-review evidence, not cross-platform pixel-golden tests.
- Avoid startup network dependencies. Pin Node dependencies and reuse repository browser
  dependency hydration rather than launching through an unpinned download.
- Keep stdout machine-readable and return actionable correctable errors: unknown alias, invalid
  kind, occupied location, rejected command, missing asset, timed-out snapshot, unavailable
  Chrome/FFmpeg, incompatible daemon, or stale runtime state.
- Preserve the ordinary human-facing Lab vocabulary for the product and wire protocol. Deep rename
  only agent-tool identities, paths, modules, exported classes, launch gates, diagnostics, and
  artifacts from Agent Lab/agentlab/agent-lab to Lab Interact/lab-interact.
- Do not make the agent play a full match. Use bounded inspection scenes such as one stationary
  unit, two moving units, or one unit firing at a target.

## Phase Summaries

### [Phase 0 - CLI Migration And Deep Rename](phase-0.md)

Delete the failed MCP adapter and project registration, deeply rename the agent tool to Lab
Interact, and expose its existing bounded commands through a repository-owned CLI. Add one
auto-started daemon per canonical worktree so `open` returns the opaque id for one authoritative
session that later commands share, with race-safe startup and private local IPC. Let `close` release
that session while the daemon stays ready, and tear everything down on `shutdown` or 30 minutes
without interaction.

### [Phase 1 - Reusable Lab Interact Driver](phase-1.md)

Build the transport-independent driver that starts the selected worktree's private server and
headless normal client, opens an authoritative Lab, and exposes typed Lab/camera methods. Add a
narrow in-page bridge so automation does not depend on every internal field while all authority
remains in the existing Lab protocol and room. This underlying work is done; Phase 0 renames it and
makes it the persistent daemon core behind the CLI.

### [Phase 2 - Stateful Interaction Contract](phase-2.md)

Define the bounded lifecycle, catalog, mutation, inspection, command, time, camera, alias, and
structured-error contract over the driver. Preserve its strict schemas and end-to-end smoke while
moving transport ownership to one worktree daemon and command mapping to the CLI. The reusable
contract is done, but its former MCP adapter is superseded and must be deleted in Phase 0.

### [Phase 3 - Screenshot MVP And Agent Workflow](phase-3.md)

Add renderer readiness, reversible clean presentation, deterministic viewport/camera setup, and a
bounded screenshot operation that writes a PNG plus manifest under the ignored target root. Make
the agent workflow capture through the CLI, read the returned path, inspect the PNG once, and share
the artifact without Browser Use. The underlying capture MVP is done; Phase 0 renames its paths and
skill and replaces its inaccessible transport before the manual review gate is repeated.

### [Phase 4 - Portable Setup And Replay Artifacts](phase-4.md)

Make successful sessions reusable through bounded CLI export/import commands for setup checkpoints
and Lab replay artifacts. Reuse `LabCheckpointScenarioV1` for static state and
`LabReplayArtifactV1` for timestamped operations, preserving server validation and entity-id
remaps. Keep large artifact bytes out of CLI output and normal WebSocket control frames.

### [Phase 5 - Real-Time Video And Contact Sheets](phase-5.md)

Add bounded CLI recording commands around the persistent headless page so normal commands can run
while a 30 FPS WebM records movement or combat. Generate representative PNG frames, a contact
sheet, and a manifest because agents should inspect stills rather than consume a long clip blindly.
Keep recording real-time and review-oriented without claiming deterministic animation capture.

### [Phase 6 - Deterministic Frame-Time Capture](phase-6.md)

Introduce an injected render clock and fixed-step CLI capture path that advances authoritative room
time and client visual time explicitly. Capture a bounded PNG sequence and encode it so animation
review can be repeated at known ticks and frame times. Preserve normal runtime timing and gameplay
behavior, and defer cross-GPU golden-image promises until evidence supports them.

## Migration And MVP Review Gate

Phases 1-3 record completed underlying MVP work, but the old integration failed its real Codex
Desktop usability gate and is unsupported. Implement Phase 0 next, then manually test the renamed
CLI from a fresh Codex task against a graphics change in a non-main worktree. Review command
vocabulary, auto-start, open/close/shared-session behavior, idle/shutdown teardown, image quality,
artifact sharing, and failure messages before approving Phase 4.

## Non-Goals

- Full autonomous gameplay, opponent strategy, or a general game-playing benchmark.
- A public or production remote-control service for arbitrary Lab rooms.
- MCP compatibility, project-scoped Codex server registration, or a second supported adapter.
- Raw JavaScript evaluation, arbitrary WebSocket messages, direct `GameState` mutation, or general
  filesystem access through CLI commands.
- Scratch asset upload, hot-reload asset URLs, balance hot reload, or an alternate renderer.
- Automatic visual approval or pixel-diff merge gates in the MVP.
- Writing bundled Lab scenarios, source media, commits, PRs, or external messages from the CLI.
- Replacing the human Lab panel, dev scenarios, browser smoke suite, checkpoint format, or replay
  format.

## Phase Delivery And Handoffs

Each planned phase must be implemented and committed on its own `zvorygin/` branch in a clean
task-specific worktree, pushed as an owned PR, and opened with auto-merge armed. After opening the
PR, the implementing agent must run `scripts/wait-pr.sh`, wait for a definite merge, fetch
`origin/main`, and verify the phase head is reachable from `origin/main` before reporting the phase
complete or starting the next phase. Mark that phase document `Status: done.` in its implementation
commit.

Every phase handoff must include:

- final public/internal APIs and CLI command names added or changed;
- exact verification commands and results;
- whether client, protocol, `Game`, room, checkpoint, or replay contracts changed;
- daemon/process/session cleanup behavior for close, shutdown, and idle expiry, plus remaining
  failure cases;
- what the next agent should implement;
- the core manual test focus for the next phase, not an exhaustive matrix;
- whether the next phase may proceed or must wait at the migrated MVP review gate.

Execute the corrective migration explicitly:

```bash
scripts/phase-runner.sh --plan lab-interact phase-0 --pr --wait
```

Phases 1-3 are already complete as underlying implementation history and must not be rerun. Only
after Phase 0 merges and the fresh CLI review gate is cleared should the later sequence run:

```bash
scripts/phase-runner.sh --plan lab-interact --from 3 --to 6 --pr --wait
```
