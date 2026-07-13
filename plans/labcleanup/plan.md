# Lab Interact CLI Cleanup Plan

## Purpose

Turn the Lab Interact CLI from a successful proof of concept into a small, understandable developer
tool without treating it like production software. The current Node/JavaScript implementation is the
right runtime shape because it coordinates Chrome, page RPC, media tools, Tailnet previews, and a
private Rust game server; Rust remains authoritative for game state, but moving the orchestrator to
Rust would add integration cost without fixing its architectural problems. This plan takes the
high-value 80/20 path: establish a reliable behavioral canary, fix the main ownership and blocking
boundaries, then migrate the settled Node-side code to TypeScript.

The deep product/command rename is intentionally deferred. Until that separate rename pass, the
operator entry point remains `node scripts/lab-interact/cli.mjs` even after the implementation behind
it becomes TypeScript.

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
metadata, one owner of semantic command ordering, responsive/cancellable long process work, explicit
layer constraints, and strict type checking for the Node-side implementation. It should still be a
plain repository tool: no packaging, generated client, service framework, or deployment surface. The
existing authority, validation, artifact confinement, and loopback/capability boundaries must remain
intact.

## Overall Constraints

- Keep this to three executable phases before a measured checkpoint. Do not extend the chain during
  implementation; record newly discovered lower-value work in the deferred backlog.
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
- Keep the browser client in native JavaScript with no build step. Phase 3 covers the Node-side CLI
  implementation only.
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

### [Phase 2 - Explicit Boundaries and Responsive Operations](phase-2.md)

Create one static command definition source, one application-layer owner for session ordering, and
small asynchronous adapters for long-running child processes and private-server startup. Remove the
driver's duplicate semantic queue and signal ownership, then add a narrow architecture check that
prevents those responsibilities from collapsing back into the two hotspot files. Keep capture,
artifact, and browser implementations mostly intact unless extraction is directly needed for
responsiveness or dependency direction.

### [Phase 3 - Directly Executable TypeScript](phase-3.md)

Migrate the now-separated Node-side implementation to strict TypeScript and execute it directly with
Node's built-in type stripping, retaining only a tiny `cli.mjs` compatibility/bootstrap entry point
until the later rename. Add a no-emit typecheck and type the high-value command, IPC, session, error,
and adapter seams while preserving runtime validation at every untrusted boundary. Do not add a
transpiler, bundler, generated output, client build pipeline, or wholesale test conversion.

## Checkpoint

After Phase 3, stop and review evidence before planning more work. The checkpoint should consider:

- whether the live canary is reliable and fast enough to run for relevant changes;
- whether cold open, status, cancellation, and shutdown remain responsive during slow subprocesses;
- whether the command registry and coordinator make ordinary changes local and obvious;
- whether source-size/import ratchets are helping without creating busywork; and
- whether native TypeScript execution is pleasant enough to keep before starting the separate rename.

The rename should be a fresh task based on this checkpoint, not an implicit fourth phase.

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
```
