# Lab Operations Plan

## Purpose

Make Interact fast and predictable for general scene authoring without adding a scene-specific
composition language. The existing `spawn`, `update`, `remove`, inspection, camera, and recording
concepts remain, but the primitive operations become genuinely bulk, placement failures become
actionable, command help becomes discoverable, daemon freshness becomes visible, and one-minute
recordings become a first-class workflow.

## Intended Result

- `spawn`, `update`, and `remove` accept one through 400 inputs and reach authority as one atomic
  operation with one post-batch observation rather than per-item confirmation waits.
- Bulk movement validates the destinations simultaneously so moved entities do not block their own
  former positions and legal translations or swaps do not require staging.
- Placement failures identify the failed input and blocking entity, terrain, feature, or boundary,
  and return a bounded deterministic list of nearby positions accepted by the same authoritative
  placement predicate.
- The CLI provides daemon-free per-command help with exact shapes, variants, defaults, limits, and
  examples.
- Aliases, inspection, camera focus, screenshot subjects, and bulk mutations support up to 400
  references while returned manifests and diagnostics stay explicitly bounded.
- The daemon publishes the checkout commit it started under. The CLI reports checkout mismatch and
  never silently destroys an active scene to refresh a stale daemon.
- Real-time recording supports 60 seconds and a wait operation that returns the finalized MP4
  without caller-managed sleeps or status polling.

## Overall Constraints

- Preserve the server-authoritative Lab boundary. Do not add generic state patches, browser
  evaluation, arbitrary protocol messages, or an impossible-placement bypass.
- Keep the existing primitive vocabulary; do not add a declarative battle/formation/scene DSL.
- Bulk mutations are atomic: validate the entire request against a scratch state, commit only when
  every input succeeds, preserve input ordering, and identify the failed input index on rejection.
- Use plural wire and replay operations for Interact, including one-item requests, while keeping
  old singular artifact operations readable when compatibility requires it.
- Run repair and authoritative commit once per accepted batch rather than once per item. The CLI
  waits for one post-batch observation; ordinary 30 Hz live snapshot publication remains free to
  continue, and paused authoring must not turn one batch into hundreds of simulation advances.
- Legal-position suggestions must be deterministic, bounded in count and search work, and use the
  authoritative placement predicate. A screenshot-on-error overlay is deferred; structured
  blockers and legal alternatives are the initial general-purpose diagnostic.
- Operational limits may rise to 400, but CLI output, screenshot manifests, alias sidecars, and
  recording manifests must retain explicit summary caps with total/truncated metadata.
- Checkout mismatch must leave `status` and `shutdown` usable. Automatic restart is allowed only
  when no active session or request can be lost; otherwise return a clear warning/error with both
  commit ids and the recovery command.
- Preserve mobile-compatible H.264 MP4 output, artifact confinement under `target/interact/lab/`,
  strict stdout JSON, per-worktree daemon isolation, and existing teardown guarantees.
- Update `docs/design/protocol.md` and any changed `Game` API documentation with the owning contract
  in the same phase.

## Phase Summaries

### [Phase 1 - Atomic Bulk Mutation And Placement Diagnostics](phase-1.md)

Add authoritative plural spawn, update, and remove operations with a 400-item bound, atomic
validation, simultaneous group movement, ordered results, and one post-batch observation.
Carry structured placement blockers and deterministic nearby legal suggestions from the simulation
through the protocol, replay/timeline, browser bridge, driver, daemon, and CLI error envelope.
Prove the result with focused simulation, protocol, room, replay, client, CLI, and live Lab tests.

### [Phase 2 - Discoverability, Large-Scene Bounds, And Daemon Freshness](phase-2.md)

Add daemon-free per-command help and raise the operational alias, inspection, focus, screenshot,
and related reference limits to 400 while keeping artifact summaries bounded. Publish the daemon's
startup checkout commit and compare it with the current checkout before dispatch, automatically
refreshing only an idle daemon and protecting active scenes with an actionable mismatch response.
Exercise a large-scene CLI canary that uses bulk operations, focuses the full scene, and captures a
bounded clean screenshot.

### [Phase 3 - One-Minute Recording And Wait Workflow](phase-3.md)

Raise the real-time recording ceiling to 60 seconds and add a wait command backed by the recorder's
completion promise so callers receive the finalized artifact without polling. Give recording and
media finalization appropriately bounded command-specific timeouts, retaining the 64 MiB cap unless
real dense-scene evidence requires a documented adjustment. Verify a dense 1200x800 60-second MP4
and the start, manipulate, wait, close, shutdown, timeout, and failure lifecycles.

## Delivery And Handoffs

Each phase is implemented on its own `zvorygin/*` branch in a clean task worktree, committed, pushed
through `scripts/agent-pr.sh`, and opened as an owned PR with auto-merge armed. The implementing
agent must run `scripts/wait-pr.sh`, wait for a definite merge, fetch `origin/main`, and verify the
phase head is reachable from `origin/main` before reporting completion or beginning the next phase.
The phase implementation commit marks its phase file `Status: done.`

After every phase, provide a self-contained handoff naming the merged contract, focused validation,
remaining risks, what the next phase should do, and the core features that should be manually
tested. Manual testing should cover the ordinary workflow and the principal failure/recovery path,
not an exhaustive matrix.

## Executor Commands

After this plan is merged, execute and merge one phase at a time:

```bash
scripts/phase-runner.sh --plan labops phase-1 --pr --wait
scripts/phase-runner.sh --plan labops phase-2 --pr --wait
scripts/phase-runner.sh --plan labops phase-3 --pr --wait
```
