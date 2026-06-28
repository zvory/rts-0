# Drain Artifacts Plan

## Purpose

Guarantee that deploy drain preserves replay-backed match history for live matches that are still in
progress when the server must exit. Matches ended this way should be recorded with no winner and an
explicit `aborted` outcome, so Recent Matches can show "Aborted" instead of treating the row as a
draw. The implementation should keep normal match resolution non-blocking while giving shutdown a
bounded path to finalize active games and wait for replay/match-history writes.

## Phase Summaries

### [Phase 1 - Aborted Outcome Contract](phase-1.md)

Add `aborted` as an explicit match-history outcome across the database, server API, client display,
and design docs. Replace the current derived `winner_name -> win/draw` write behavior with an
explicit server-side outcome so `winner_name = null` can represent either draw or aborted depending
on `outcome`. This phase should make persisted and API-returned aborted rows display as "Aborted"
with no winner, but it should not yet change deploy drain behavior.

### [Phase 2 - Bounded Match-History Write Tracking](phase-2.md)

Introduce a bounded tracker for match-history write tasks so normal rooms can still detach slow
Supabase writes, while shutdown can wait for writes that were started during the drain window. Route
normal `end_match` persistence through this tracker and expose a lobby/drain-level wait primitive
with timeout-friendly semantics. This phase should not force-end active games yet; it only makes
already-started history/replay writes observable and awaitable during shutdown.

### [Phase 3 - Shutdown Abort Finalization](phase-3.md)

Add a deploy-shutdown finalization event that asks active live rooms to capture their current `Game`
state as an aborted, replay-backed match before WebSocket connections are closed. Reserve part of
Fly's 300 second stop window for this forced finalization and write wait: first let matches end
naturally, then abort any remaining eligible live matches, then wait for final writes within the
reserved budget. This phase is the core behavior change that prevents empty-room cleanup from
drawing down the active-match counter without recording the replay artifact.

### [Phase 4 - End-to-End Coverage and Deploy Docs](phase-4.md)

Add regression coverage, operational logging, and docs for the new drain-abort lifecycle. Verify the
full path from shutdown signal through aborted match record, replay row, Recent Matches display, and
replay launch. This phase should leave the workflow ready for beta validation, including manual
testing notes for an in-progress match interrupted by deploy drain.

## Overall Constraints

- Server remains the only match-history writer. Clients may display `aborted`, but must not create
  or mutate match-history rows.
- Preserve the `RTS_RECORD_MATCHES` gate. Local runs with the gate off must not upload match rows or
  replay artifacts, including drain-aborted matches.
- Use a versioned SQL migration for the `matches.outcome` CHECK constraint. Do not hand-apply DDL
  or depend on uncommitted schema state.
- Keep `winner_name` and replay `winnerId`/`winnerTeamId` winner-only. Do not overload
  `winner_id = None` or `winner_name = null` to mean aborted; `outcome` is the source of truth for
  draw versus aborted.
- Keep normal match completion non-blocking. Slow Supabase writes must not stall room transitions
  outside deploy shutdown; shutdown may wait only through an explicit bounded drain/write wait.
- Capture aborted replay artifacts before shutdown closes WebSockets and before `Leave` events can
  empty/reset rooms. Empty-room reset must not be the mechanism that "finishes" a drain-aborted
  live match.
- Preserve existing match-history eligibility filters. Normal live matches with recording enabled
  may be aborted and recorded; dev, replay, replay-artifact, replay-branch, lab, and automated test
  fingerprints should not suddenly become public match-history rows.
- Keep replay artifact schema unchanged unless implementation proves an explicit replay-level
  aborted marker is required. The match-history row can say `aborted` while the replay artifact
  plays to the captured shutdown tick with no winner.
- Stay within Fly's stop window. `kill_timeout = 300` remains the external cap; the application
  should reserve time for forced abort, write completion, WebSocket close, and Axum graceful exit.
- Update design docs in the same phase that changes their source-of-truth contract, especially
  `docs/design/match-history.md`, `docs/design/hardening.md`, and `docs/fly.md`.
- Each implementation phase must land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite PR merge with the phase head reachable from
  `origin/main` before the next phase starts.
- When a phase is complete, mark that phase document done in the implementation commit and provide a
  handoff message describing what changed, what the next agent should do, and the core manual
  testing focus.

## Phase Index

1. [Phase 1 - Aborted Outcome Contract](phase-1.md)
2. [Phase 2 - Bounded Match-History Write Tracking](phase-2.md)
3. [Phase 3 - Shutdown Abort Finalization](phase-3.md)
4. [Phase 4 - End-to-End Coverage and Deploy Docs](phase-4.md)

## Non-Goals

- Do not add accounts, player identity, ratings, or leaderboard semantics.
- Do not persist partial local developer matches when `RTS_RECORD_MATCHES` is off.
- Do not expose replay artifact JSON through `/api/matches` or `/api/lobbies`.
- Do not change normal combat victory, give-up, draw, post-match replay, or replay-branch behavior
  except where shared persistence helpers are factored.
- Do not attempt a crash/outbox recovery system in this plan. The goal is graceful deploy drain, not
  process-crash durability.
- Do not add a new client command to trigger aborted records. Only server-controlled deploy
  shutdown can produce this outcome.

## Required Verification Themes

Each phase should run the smallest relevant subset of:

- Focused Rust tests for `Db` outcome mapping, room lifecycle, deploy drain timing, and write
  tracking.
- Focused JS/client contract tests for Recent Matches rendering when `outcome === "aborted"`.
- `node tests/protocol_parity.mjs` only if protocol DTOs or mirrored wire shape change.
- `node scripts/check-client-architecture.mjs` if client module wiring changes.
- `cargo fmt --manifest-path server/Cargo.toml --check` or equivalent formatting check for Rust
  touches.
- `git diff --check`.

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait gate
and confirm the phase head is reachable from `origin/main`. For unattended executor passes after the
plan is approved, use:

```bash
scripts/phase-runner.sh --plan drainartifacts phase-1 --pr --wait
scripts/phase-runner.sh --plan drainartifacts phase-2 --pr --wait
scripts/phase-runner.sh --plan drainartifacts phase-3 --pr --wait
scripts/phase-runner.sh --plan drainartifacts phase-4 --pr --wait
```

## Handoff Requirements

After every phase, the implementing agent must provide a handoff message for the next agent. The
handoff must summarize shipped behavior, focused verification, known blockers, and any contract or
docs changes that later phases must honor. Manual testing notes should cover the core interrupted
deploy-drain flow and Recent Matches replay launch, not an exhaustive test matrix.
