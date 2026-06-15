# Phase 2 - Report-Backed Replay Durability

Status: planned.

## Goal

Make replay evidence durable for reportable matches independently of ordinary match-history
eligibility. A submitted bug report must force final replay upload for the match's server-generated
`replay_key`, even for AI-vs-AI, human-vs-AI, and solo matches, and even though the final artifact
is normally written at match end.

## Scope

- Audit current replay capture and match-history write paths.
- Ensure normal match start allocates and retains a stable `replay_key` for the match.
- Add a server-side helper that marks a room/match `replay_key` as report-backed and therefore
  force-persisted at match end.
- Relax replay persistence for report-backed AI-vs-AI, human-vs-AI, and solo cases without making
  all such matches appear as ordinary public match-history rows unless explicitly intended.
- Ensure the replay artifact includes enough deterministic context for AI-seat replay playback and
  later review.
- Make report submission block until the report row is saved and the force-persist replay flag is
  registered for that `replay_key`. The final replay artifact may remain pending until match end.
- Write the final replay artifact under the same `replay_key` at match end. If a matching report
  already exists, the review dashboard should transition from pending to available once this write
  succeeds.
- Keep ordinary match history writes detached/non-critical.
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

- Prefer the `replay_key` association over placeholder replay rows. The bug report can store
  `replay_key` before a `match_replays` row exists, and the final replay row can be resolved by the
  same key later.
- If the implementation still introduces placeholder rows, make their lifecycle explicit: what
  fields are known at report time, what updates at match end, and what the review UI should display
  before resolution.
- If the existing room task cannot safely create an in-progress replay artifact, stop and document
  the blocker instead of faking replay durability.

## Constraints

- Do not add player-facing report UI in this phase.
- Do not block normal match-end transitions on ordinary match-history writes.
- Do not silently drop report-backed replay persistence failures. The eventual report API must show
  whether the force-persist flag was registered, and the dashboard must show pending or missing
  replay state instead of a false available replay.
- Keep replay compatibility validation aligned with the existing match-history replay launch rules.

## Verification

- Add focused server/Rust tests showing report-backed replay persistence works for:
  - a normal two-human match context
  - a human-vs-AI match context
  - an AI-vs-AI or solo context if those paths can be exercised without broad harness work
  - a match still in progress where the report stores `replay_key` before the final replay row exists
- Add or update tests around deploy drain/write tracking if drain behavior changes.
- Run the smallest relevant Rust test target plus formatting.

## Manual Testing Focus

- Start a local server with database configured.
- Exercise a human-vs-AI or solo match, submit/mark a report-backed `replay_key`, and confirm the
  final replay artifact is persisted under that same key even if it would not appear in normal
  public match history.
- Confirm ordinary match-history behavior did not start showing every AI/debug/test match unless the
  phase explicitly chose that policy.

## Handoff

After implementation, mark this phase done and summarize exactly how a later report API obtains the
match `replay_key`, how it marks that key for forced replay upload, and how the dashboard should
distinguish pending, available, and missing replay states.
