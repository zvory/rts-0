# Agent Lab Plan

## Purpose

Give repository agents a small, stateful tool API for arranging and inspecting authoritative game
scenes without Computer Use, Browser Use, or a growing command-line flag surface. The shipped
surface should be a project-scoped local MCP server backed by a reusable `AgentLabDriver`; the
driver owns a private Rust server, headless Chrome, one or more lab sessions, the normal Pixi
client, stable entity aliases, and bounded capture artifacts. The existing lab remains the product
and authority boundary: this plan adds an agent adapter around typed lab operations, normal
commands, room time, checkpoint setups, and lab replay artifacts rather than introducing a second
simulation or a raw state mutation API.

The first implementation milestone ends after Phase 3. At that point an agent must be able to open
the active task worktree, discover available entities, build a tiny scene through typed calls,
issue orders, pause or step authoritative time, position the camera, take a screenshot, inspect the
returned image, and share the artifact path with the user. Phases 4-6 are deliberate follow-up work
for portable scene artifacts, real-time video, and deterministic frame capture; do not start them
until the Phase 3 workflow has been manually reviewed and the tool vocabulary has proved usable.

This plan refines the product direction in [the lab architecture hypotheses](../lab/architecture.md).
That document remains the lab product north star; this plan owns only the local agent-control and
media-capture adapter.

## Chosen Architecture

```text
Codex or another local MCP client
  -> project-scoped Agent Lab MCP server (stdio)
    -> AgentLabDriver (session/process/alias/artifact ownership)
      -> private loopback Rust server and authoritative lab room
      -> headless normal client with a narrow in-page agent bridge
      -> normal Pixi renderer
    -> structured tool results, image content, and bounded local artifacts
```

The MCP server is the agent interaction surface. A shell entry point may remain for setup,
contract tests, and human diagnostics, but new behavior must appear as typed driver methods and MCP
tools rather than accumulating unrelated CLI flags. Keep MCP transport and schemas out of client,
simulation, and room code so the driver can also support focused tests or a future non-MCP adapter.

The first stable tool vocabulary should stay small and composable:

- session lifecycle: `lab_open`, `lab_close`, `lab_reset`;
- discovery and observation: `lab_catalog`, `lab_inspect`;
- setup mutation: `lab_spawn`, `lab_update`, `lab_remove`;
- gameplay and time: `lab_order`, `lab_time`;
- presentation: `lab_camera`, then `lab_screenshot` in Phase 3;
- later artifact/media tools: `lab_export`, `lab_import`, `lab_record_start`, and
  `lab_record_stop`.

Tool results may return authoritative numeric ids, but callers should normally use session-local
aliases such as `shooter`, `target`, or `subject`. Alias resolution belongs to the driver and must
survive known checkpoint entity-id remaps; aliases are not wire fields, simulation state, or a new
persistence contract.

## Overall Constraints

- Run a real `Game` in a real lab room and render through the normal client. Do not add a
  renderer-only fake entity model, a parallel simulation, or a second game client.
- Enter authoritative state only through existing typed lab operations, normal
  `issueCommandAs`, room-time controls, or deliberately reviewed extensions to those seams. Never
  expose arbitrary object mutation, code evaluation, private `GameState` serialization, or raw
  checkpoint editing as an MCP tool.
- Preserve the `Game` API seam, the room task's sole ownership of `Game`, the mirrored protocol,
  panic-free tick path, and authoritative fog/event projection. Any protocol addition must update
  both mirrors and `docs/design/protocol.md` in the same phase.
- Keep the agent integration local. The MCP server should use stdio and private loopback child
  processes; do not add a production remote-control endpoint or make normal deployed rooms
  discoverable through the agent tool.
- Render the explicitly selected task worktree. Resolve and validate its Git top level, start the
  server from that worktree, record its branch/head in artifacts, and never silently serve the
  original checkout when the graphics change lives in another worktree.
- Keep session and response sizes bounded. Inspection tools need filters, limits, concise entity
  projections, and truncation metadata instead of returning a full snapshot or checkpoint payload
  to the model.
- Treat checkpoint setups and `LabReplayArtifactV1` as opaque, versioned artifacts. A static scene
  may export a checkpoint setup; a scene with timed mutations or commands should export a lab
  replay. The agent should build both through tools rather than hand-authoring their large JSON
  internals.
- Restrict all default outputs to `<worktree>/target/agent-lab/`, use safe generated names, and
  ignore that directory in Git. A capture or export tool must not accept an arbitrary output path,
  write source assets, submit a PR, or add a bundled scenario.
- Own child processes, ports, temporary browser profiles, timeouts, and teardown explicitly. A
  failed or interrupted tool call must not leave a private server, Chrome process, recorder, or
  stale session holding resources indefinitely.
- Pin browser viewport and device-pixel ratio for repeatability and record browser/build details in
  manifests. Initial screenshots are visual-review evidence, not cross-platform pixel-golden tests.
- Avoid a network-time dependency for tool startup. Pin Node dependencies in a lockfile and reuse
  or extract the repository's existing browser-dependency hydration; do not launch the MCP server
  through an unpinned `npx` download.
- Return actionable tool errors that the model can correct: unknown alias, invalid kind, occupied
  location, command rejection, missing asset, timed-out snapshot, unavailable Chrome/FFmpeg, or
  stale session. Do not turn expected tool errors into protocol crashes.
- Add server-wide MCP instructions describing the normal workflow and the distinction between
  ephemeral lab changes and repository writes. Tool descriptions must state side effects and use
  appropriate read-only/destructive/idempotent annotations.
- Do not make the agent play a full match. This plan is for bounded inspection scenes such as one
  stationary unit, two units moving, or one unit firing at a target.

## Phase Summaries

### [Phase 1 - Reusable Agent Lab Driver](phase-1.md)

Build a transport-independent driver that starts the selected worktree's private server and
headless normal client, opens an authoritative lab, exposes typed lab/camera methods, and tears the
session down reliably. Add a narrow in-page bridge so automation does not depend on every internal
field of `window.__rts`, while keeping all authority in the existing lab protocol and room. Prove
the driver can discover a catalog, spawn and inspect entities, issue a real command, and pause/step
time without adding MCP or media capture yet.

### [Phase 2 - Project-Scoped Interactive MCP](phase-2.md)

Wrap the driver in a local stdio MCP server configured by trusted project-scoped Codex config.
Expose the bounded session, catalog, mutation, inspection, command, time, and camera tools with
session-local aliases and structured errors. Verify the tool contract through an MCP client harness
and a fresh-client manual smoke, but do not add screenshots or artifact persistence in this phase.

### [Phase 3 - Screenshot MVP And Agent Workflow](phase-3.md)

Add renderer asset-readiness, a clean shared presentation mode, deterministic viewport/camera
setup, and a `lab_screenshot` tool that returns both MCP image content and a bounded local artifact
manifest. Document the graphics-review workflow in repository guidance and a focused skill so an
agent can capture, inspect, and share a scene without Browser Use. Stop after this phase for manual
review of a stationary unit and a tiny two-entity scene before approving later phases.

### [Phase 4 - Portable Setup And Replay Artifacts](phase-4.md)

Make successful interactive sessions reusable by exposing bounded setup checkpoint and lab replay
export/import through local agent tooling. Reuse `LabCheckpointScenarioV1` for static state and
`LabReplayArtifactV1` for timestamped operations and commands, preserving server-side validation
and entity-id remaps instead of inventing an agent-authored checkpoint schema. Keep large artifact
bytes out of model context and out of the normal WebSocket control-frame path.

### [Phase 5 - Real-Time Video And Contact Sheets](phase-5.md)

Add bounded start/stop recording tools around the live headless page so the agent can issue normal
tool calls while a 30 FPS WebM records movement or combat. Generate a representative PNG contact
sheet and manifest alongside every video, because the model should inspect a few frames rather than
consume a long clip blindly. Keep this real-time and review-oriented; do not claim deterministic
animation capture yet.

### [Phase 6 - Deterministic Frame-Time Capture](phase-6.md)

Introduce an injected render/visual clock and a fixed-step capture path that advances authoritative
room time and client visual time explicitly. Capture a bounded PNG sequence and encode it with
FFmpeg so animation review can be repeated at known ticks and frame times. Preserve normal runtime
timing and gameplay behavior, and avoid cross-GPU golden-image promises until evidence supports
them.

## MVP Review Gate

Phases 1-3 form the initial approved implementation sequence. After Phase 3 merges, manually test
the tool from a fresh Codex task against a graphics change in a non-main worktree and review the
tool vocabulary, session lifecycle, image quality, artifact sharing, and failure messages. Record
the decision to continue, revise the API, or stop before executing Phase 4; do not automatically
run Phases 4-6 just because their documents exist.

## Non-Goals

- Full autonomous gameplay, opponent strategy, or a general game-playing benchmark.
- A public or production remote-control service for arbitrary lab rooms.
- Raw JavaScript evaluation, arbitrary WebSocket messages, direct `GameState` field mutation, or
  general filesystem access exposed as tools.
- Scratch asset upload, hot-reload asset URLs, balance hot reload, or an alternate renderer.
- Automatic visual approval or pixel-diff merge gates in the MVP.
- Writing bundled lab scenarios, source media, commits, PRs, or external messages from MCP tools.
- Replacing the human-facing lab panel, dev scenarios, browser smoke suite, checkpoint format, or
  replay format.

## Phase Delivery And Handoffs

Each phase must be implemented and committed on its own `zvorygin/` branch in a clean task-specific
worktree, pushed as an owned PR, and opened with auto-merge armed. After opening the PR, the
implementing agent must run `scripts/wait-pr.sh`, wait for a definite merge, fetch `origin/main`,
and verify the phase head is reachable from `origin/main` before reporting the phase complete or
starting the next phase. Mark the phase document `Status: done.` in the implementation commit.

Every phase handoff must include:

- the final public/internal APIs and tool names added or changed;
- exact verification commands and results;
- whether client, protocol, `Game`, room, checkpoint, or replay contracts changed;
- process/session cleanup behavior and remaining failure cases;
- what the next agent should implement;
- the core manual test focus for the next phase, not an exhaustive test matrix;
- whether the next phase can proceed or must wait at the Phase 3 review gate.

After this plan is approved, execute the MVP sequence explicitly and stop for review:

```bash
scripts/phase-runner.sh --plan agentlab phase-1 --pr --wait
scripts/phase-runner.sh --plan agentlab --from 1 --to 3 --pr --wait
```

Only after the MVP review gate is cleared should the later sequence run:

```bash
scripts/phase-runner.sh --plan agentlab --from 3 --to 6 --pr --wait
```
