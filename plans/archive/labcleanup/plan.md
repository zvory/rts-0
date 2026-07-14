# Lab Interact CLI Cleanup Plan

## Purpose

Turn the Lab Interact CLI from a successful proof of concept into a small, understandable developer
tool without treating it like production software. The current Node/JavaScript implementation is the
right runtime shape because it coordinates Chrome, page RPC, media tools, Tailnet previews, and a
private Rust game server; Rust remains authoritative for game state, but moving the orchestrator to
Rust would add integration cost without fixing its architectural problems. This plan takes the
high-value 80/20 path: establish a reliable behavioral canary, separate application ownership, make
external-process and private-server lifecycle responsive, then translate the settled Node-side tool
to TypeScript.

The TypeScript migration is the fourth and final phase of this plan, after both architecture splits
have landed independently. The deep product/command rename remains a separate later task.

## Current Evidence

- `tests/lab_interact_cli_smoke.mjs` already exercises a real daemon, browser, authoritative Lab
  scene, PNG screenshot, Tailnet preview, short H.264 recording, session reuse, and teardown. Phase 1
  should improve and promote this test rather than create a competing harness.
- `command_service.mjs` currently combines input validation, command routing/help metadata, session
  state, aliases, artifact work, and semantic ordering. `driver.mjs` combines browser RPC, private
  server startup, dependency/tool processes, capture orchestration, signal handling, and a second
  operation queue.
- Long synchronous Cargo, dependency, FFmpeg, and probe operations can block the daemon while its
  status model implies that work can be observed or cancelled concurrently.
- Public commands, help, validation, execution lanes, and timeout choices are maintained in several
  places, so adding or changing a command creates avoidable drift risk.
- The browser bridge is narrow, launch-gated, and already protected by focused contracts. A deeper
  client facade or client-wide build system would cost more than it returns in this cleanup.

## Outcome

At the checkpoint, Lab Interact should have one meaningful live smoke workflow, one source of command
metadata, one owner of semantic command ordering, responsive/cancellable long process work, and
explicit layer constraints, with the Node-side implementation checked as strict TypeScript. It should
still be a plain repository tool: no packaging, generated client, service framework, or deployment
surface. The existing authority, validation, artifact confinement, and loopback/capability boundaries
must remain intact.

## Overall Constraints

- Execute these four phases in order, each as its own merged PR. Let the actual application, adapter,
  and language boundaries determine phase scope rather than imposing an arbitrary phase-count cap.
- Preserve the Rust server as the sole game authority and the browser page bridge as a narrow,
  launch-gated automation surface. The Node tool may arrange and observe a Lab but must not duplicate
  simulation rules or bypass server validation.
- Preserve exact and bounded command validation, loopback-only private-server access, one
  authoritative session per worktree, per-session artifact capability, replay transfer checks,
  artifact confinement under `target/lab-interact/`, and opaque Tailnet preview registration.
- Optimize for one developer on a pre-alpha game. Prefer a few explicit modules and static tables to
  frameworks, generalized plugin systems, generated schemas, or speculative abstractions.
- Use semantic smoke assertions rather than freezing full JSON envelopes, generated ids/ticks, exact
  pixels, or every error permutation. Breaking internal changes are welcome; public behavior should
  stay recognizable through the cleanup so the canary remains useful.
- Keep the browser client in native JavaScript with no build step. Keep the Node tool in JavaScript
  through Phases 1-3, then convert only that settled Node-side implementation in Phase 4.
- Every phase must update `docs/lab-interact-cli.md` and relevant testing/context documentation when
  it changes an operator command, dependency, architecture rule, or verification path.
- Each phase is implemented on its own `zvorygin/` branch, committed, pushed as an owned PR with
  auto-merge armed, and followed through a definite merge. Before reporting completion or starting
  the next phase, fetch `origin/main` and verify that the phase head is reachable from it.
- Mark the phase document done in that phase's implementation commit. After each phase, provide a
  handoff describing what changed, what the next agent should do, and the core features that should
  be manually tested.

## Phase Summaries

### [Phase 1 - Behavioral Canary and Reliable Gate](phase-1.md)

Strengthen the existing live CLI smoke into one small, semantic scene workflow that proves commands,
state mutation, PNG output, preview serving, media, and teardown still work. Repair the current test
runner and artifact-isolation gaps so Lab-specific failures cannot be lost or caused by concurrent
tests deleting shared output. Make Lab Interact changes select the focused contracts and browser
smoke explicitly, while avoiding golden images, exhaustive compatibility snapshots, and slow media
matrices.

### [Phase 2 - Application Ownership](phase-2.md)

Create one static command definition source and one application-layer owner for session ordering.
Remove the driver's duplicate semantic queue and signal ownership, then add a narrow architecture
check for registry completeness, dependency direction, queue ownership, and size. Leave subprocess,
private-server, media, Tailnet, and dependency-install behavior unchanged so failures in this phase
are attributable to command/session semantics.

### [Phase 3 - Responsive External Adapters](phase-3.md)

Add small asynchronous process and private-server adapters, make cold open abortable, and move long
finite request-path subprocess work off synchronous calls. Establish explicit repository ownership
for the browser runtime dependency and prove status, cancellation, and shutdown remain responsive
while fake Cargo or media children are held open. Extend the Phase 2 architecture ratchet for adapter
direction and blocking-process rules without redesigning browser, capture, or artifact behavior.

### [Phase 4 - Directly Executable TypeScript](phase-4.md)

Migrate the separated Node-side implementation to strict TypeScript and execute it directly with
Node's built-in type stripping, retaining only a tiny `cli.mjs` version/bootstrap entry point until
the later rename. Add a no-emit typecheck and type the high-value command, IPC, session, error, and
adapter seams while preserving runtime validation at every untrusted boundary. Do not add a
transpiler, bundler, generated output, client build pipeline, or wholesale test conversion.

## Checkpoint

After Phase 4, stop and review evidence before planning more work. The checkpoint should consider:

- whether the live canary is reliable and fast enough to run for relevant changes;
- whether cold open, status, cancellation, and shutdown remain responsive during slow subprocesses;
- whether the command registry and coordinator make ordinary changes local and obvious;
- whether source-size/import ratchets are helping without creating busywork; and
- whether native TypeScript execution and strict no-emit checking improve maintenance without adding
  an unwanted build lifecycle.

The rename should remain a fresh later task, not be mixed into architecture or TypeScript work.

## Deferred Backlog

- Deep Lab Interact product, executable, command, socket, documentation, and artifact rename.
- Client-wide TypeScript, a browser bundler, or conversion of the JavaScript page bridge and tests.
- Generated runtime schemas/types, a command plugin framework, DI container, or full error hierarchy.
- A new client `LabAutomationFacade` unless future bridge growth demonstrates the need.
- Further splitting screenshot/recording/fixed-capture or introducing a generalized artifact store.
- Golden-image comparison, deterministic visual certification, long media soaks, performance budgets,
  renderer/GPU/OS matrices, crash injection, and Windows/cross-platform IPC.
- Packaging, publishing, a single executable, public service/API exposure, or a Rust rewrite.
- Exhaustive command compatibility snapshots and migration support for old pre-alpha behavior.

## Implementation Commands

After this plan is approved, run one merged phase at a time:

```bash
scripts/phase-runner.sh --plan labcleanup phase-1 --pr --wait
scripts/phase-runner.sh --plan labcleanup phase-2 --pr --wait
scripts/phase-runner.sh --plan labcleanup phase-3 --pr --wait
scripts/phase-runner.sh --plan labcleanup phase-4 --pr --wait
```
