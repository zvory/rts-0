# Lab Interact CLI Cleanup Plan

## Purpose

Turn the Lab Interact CLI from a successful proof of concept into a small, understandable developer
tool without treating it like production software. The current Node/JavaScript implementation is the
right runtime shape because it coordinates Chrome, page RPC, media tools, Tailnet previews, and a
private Rust game server; Rust remains authoritative for game state, but moving the orchestrator to
Rust would add integration cost without fixing its architectural problems. This plan takes the
high-value 80/20 path: establish a reliable behavioral canary, separate application ownership, then
make external-process and private-server lifecycle responsive.

The TypeScript migration remains the intended final cleanup operation, followed later by the deep
product/command rename. It is not bundled into these architecture phases: after the three-phase
checkpoint, create a fresh one-phase TypeScript plan if the new boundaries have held up.

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
explicit layer constraints. It should still be a plain repository tool: no packaging, generated
client, service framework, or deployment surface. The existing authority, validation, artifact
confinement, and loopback/capability boundaries must remain intact.

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
- Keep the browser client and Node tool in JavaScript throughout these three phases. TypeScript is a
  post-checkpoint follow-up so it translates settled seams instead of obscuring architecture changes
  with file/type churn.
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

## Checkpoint

After Phase 3, stop and review evidence before planning more work. The checkpoint should consider:

- whether the live canary is reliable and fast enough to run for relevant changes;
- whether cold open, status, cancellation, and shutdown remain responsive during slow subprocesses;
- whether the command registry and coordinator make ordinary changes local and obvious;
- whether source-size/import ratchets are helping without creating busywork; and
- whether the application and adapter seams are stable enough to translate mechanically to
  TypeScript.

If the result is sound, create a fresh one-phase TypeScript plan. Keep the previously chosen 80/20
end state: Node 22.18+ direct type stripping, strict `tsc --noEmit`, a tiny `cli.mjs` version/bootstrap
entry point until the rename, TypeScript only under `scripts/lab-interact/`, existing runtime
validation at untrusted boundaries, and no transpiler/bundler/emitted output/client conversion. The
rename should remain a later fresh task, not be mixed into either architecture or TypeScript work.

## Deferred Backlog

- Deep Lab Interact product, executable, command, socket, documentation, and artifact rename.
- The post-checkpoint Node-side TypeScript migration described above; it requires a fresh plan rather
  than automatic execution as a fourth phase.
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
