# Phase 3 - One-Minute Recording And Wait Workflow

Status: done.

## Objective

Support one-minute real-time recordings as a normal bounded Lab operation and let callers await the
watchdog/finalization result without writing sleeps or polling loops.

## Work

- Raise the accepted real-time recording duration ceiling from 30,000 to 60,000 milliseconds.
- Add `record-wait {sessionId}` backed by a per-recording completion promise/deferred. Active and
  finalizing recordings await the same completion; an already completed current recording returns
  its last result; a session with no recording returns `recordingInactive`.
- Treat `record-wait` as an observational wait outside `session.operationTail` and the mutation
  queue. Close/shutdown must settle recorder completion before awaiting queued work, so a waiter
  cannot prevent the lifecycle operation that needs to resolve it.
- Preserve nonblocking `record-start` so callers may issue orders, time changes, inspection, and
  camera changes before waiting. Add a documented `record --wait` convenience only if it composes
  these primitives without blocking other accepted session commands.
- Make the watchdog resolve/reject the completion primitive rather than discarding finalization
  results, and ensure close/shutdown/idle cleanup settle waiters exactly once.
- Give 60-second waits and MP4 finalization a command-specific IPC timeout with bounded headroom.
  Derive/cap media-stage timeouts from target duration where the current fixed 15-second subprocess
  limit is insufficient; do not globally weaken unrelated commands.
- Keep H.264 `avc1`, `yuv420p`, fast-start metadata, 30 FPS wall-time normalization, artifact
  confinement, representative frames/contact sheet, and silent-recording disclosure.
- Retain the 64 MiB limit unless the required dense 1200x800 canary exceeds it; any adjustment must
  remain bounded and be justified by measured output.

## Expected Touch Points

- recording limits/finalization helpers and driver recorder lifecycle
- command catalog, validation, dispatch, CLI timeout selection, daemon request plumbing, and fake
  driver
- recording, CLI, driver, close/shutdown, and live smoke tests
- Interact CLI documentation, testing context, and skill workflow

## Verification

- Contract tests for 60,000 acceptance, 60,001 rejection, active/finalizing/completed/inactive wait,
  concurrent accepted interaction before wait, and identical stop/wait result shape.
- Lifecycle tests for watchdog, explicit stop, close, shutdown, cancellation/failure, exactly-once
  completion, waiter rejection, and partial artifact cleanup.
- Timeout tests prove one-minute wait/finalization has bounded headroom while ordinary commands keep
  their existing deadline.
- Live canary records a dense 1200x800 scene for 60 seconds, awaits completion without polling, and
  probes exactly 60 seconds/1800 frames/30 FPS/H.264/`avc1`/`yuv420p`/fast-start beneath the size cap.
- Run recording contracts/smokes, docs health, suite selection, and the owned-PR workflow.

## Manual Testing Focus

Start a one-minute recording, manipulate authoritative time or camera during capture, invoke the
wait command, and open the returned MP4 on a mobile-compatible player. Confirm a second wait returns
the completed artifact, a never-started wait fails clearly, and shutdown does not leave FFmpeg,
Chrome, server, socket, or partial media behind.

## Handoff

Report the final duration/wait/timeout contract, probed media facts and size, exact tests, cleanup
behavior, and any duplicated-frame diagnostics from the stress canary. Name the remaining manual
review risks and whether the Lab operations initiative is ready for final review.
