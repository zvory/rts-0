# Phase 2 - Report-Backed Replay Durability

Status: planned.

## Goal

Make replay evidence durable for reportable matches independently of ordinary match-history
eligibility. A submitted bug report must be able to force or reuse a persisted replay artifact even
for human-vs-AI matches and even before the match naturally reaches its normal end-of-match write.

## Scope

- Audit current replay capture and match-history write paths.
- Add a server-side helper that can persist or retrieve a replay row for a reportable room/match.
- Relax replay persistence for report-backed human-vs-AI cases without making all AI matches appear
  as ordinary public match-history rows unless explicitly intended.
- Ensure the replay artifact includes enough deterministic context for AI-seat replay playback and
  later review.
- Make report-backed replay persistence block the report submission path, while ordinary match
  history writes remain detached/non-critical.
- Ensure deploy drain accounts for report/replay writes that are in progress or required by
  submitted reports.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/mod.rs`
- `server/src/db.rs`
- `server/src/main.rs` drain helpers if write tracking needs to participate in shutdown
- `server/crates/sim/src/game/replay.rs` only if artifact capture lacks required metadata
- `docs/design/match-history.md`
- `docs/context/match-history.md`

## Important Design Decisions

- Prefer creating/reusing a normal `matches` row plus `match_replays` row when a report needs replay
  evidence. If a live match has no resolved match row yet, define a clear placeholder or report-only
  match-record policy before implementing.
- If placeholder match rows are introduced, make their lifecycle explicit: what fields are known at
  report time, what updates at match end, and what the review UI should display before resolution.
- If the existing room task cannot safely create an in-progress replay artifact, stop and document
  the blocker instead of faking replay durability.

## Constraints

- Do not add player-facing report UI in this phase.
- Do not block normal match-end transitions on ordinary match-history writes.
- Do not silently drop report-backed replay persistence failures. The eventual report API must be
  able to return a failure instead of a false success.
- Keep replay compatibility validation aligned with the existing match-history replay launch rules.

## Verification

- Add focused server/Rust tests showing report-backed replay persistence works for:
  - a normal two-human match context
  - a human-vs-AI match context
  - a match still in progress, if the implementation supports live artifact capture in this phase
- Add or update tests around deploy drain/write tracking if drain behavior changes.
- Run the smallest relevant Rust test target plus formatting.

## Manual Testing Focus

- Start a local server with database configured.
- Exercise a human-vs-AI match and confirm a report-backed replay artifact can be persisted even if
  it would not appear in normal public match history.
- Confirm ordinary match-history behavior did not start showing every AI/debug/test match unless the
  phase explicitly chose that policy.

## Handoff

After implementation, mark this phase done and summarize exactly how a later report API obtains or
creates a durable replay id. Call out whether live in-progress replay capture is fully implemented
or whether Phase 3 must constrain reporting to contexts where a replay artifact already exists.
