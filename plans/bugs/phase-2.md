# Phase 2 - Report Replay-Key Binding

Status: planned.

## Goal

Bind bug reports to the deployed replay-upload path that now already records resolved normal
matches with at least one active participant. A submitted bug report should store the match's
server-generated `replay_key` immediately, mark that key as expecting final replay evidence, and
resolve to the single final replay artifact written at match end.

## Scope

- Audit current replay capture and match-history write paths.
- Ensure normal match start allocates and retains a stable `replay_key` for the match.
- Add `replay_key` to the existing final `matches`/`match_replays` write so reports can resolve to
  the one canonical replay artifact after match end.
- Add a server-side helper that marks a room/match `replay_key` as report-backed and expecting final
  replay evidence.
- Build on the current replay-history behavior: deployed normal solo, player-vs-AI, and AI-only
  matches already persist replay-backed rows when `RTS_RECORD_MATCHES` is enabled. This phase should
  not add a second replay artifact for those matches.
- Ensure the replay artifact includes enough deterministic context for AI-seat replay playback and
  later review.
- Make report submission block until the report row is saved and the report-backed replay
  expectation is registered for that `replay_key`. The final replay artifact may remain pending
  until match end.
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
- `docs/design/match-history.md`
- `docs/context/match-history.md`

## Important Design Decisions

- Prefer the `replay_key` association over placeholder replay rows. The bug report can store
  `replay_key` before a `match_replays` row exists, and the final replay row can be resolved by the
  same key later.
- Preserve one canonical replay artifact per match. Do not upload a mid-match partial replay and a
  second final replay for the same match unless a later product decision explicitly adds report
  snapshot artifacts.
- If the implementation still introduces placeholder rows, make their lifecycle explicit: what
  fields are known at report time, what updates at match end, and what the review UI should display
  before resolution.
- If the existing room task cannot safely create an in-progress replay artifact, stop and document
  the blocker instead of faking replay durability.

## Constraints

- Do not add player-facing report UI in this phase.
- Do not block normal match-end transitions on ordinary match-history writes.
- Do not silently drop report-backed replay persistence failures. The eventual report API must show
  whether the replay expectation was registered, and the dashboard must show pending or missing
  replay state instead of a false available replay.
- Keep replay compatibility validation aligned with the existing match-history replay launch rules.
- Do not re-broaden replay eligibility that the latest replay-history change already broadened.
  Instead, test that bug-report lookup can see persisted rows even when they are hidden from Recent
  Matches.

## Verification

- Add focused server/Rust tests showing report-backed replay persistence works for:
  - a normal two-human match context
  - a human-vs-AI or solo match that already qualifies for deployed replay persistence
  - an AI-only row that may be hidden from `/api/matches` but still has a replay row
  - a match still in progress where the report stores `replay_key` before the final replay row exists
- Add or update tests around deploy drain/write tracking if drain behavior changes.
- Run the smallest relevant Rust test target plus formatting.

## Manual Testing Focus

- Start a local server with database configured.
- Exercise a human-vs-AI or solo match, submit/mark a report-backed `replay_key`, and confirm the
  final replay artifact is persisted under that same key through the existing match-end upload path.
- Confirm AI-only and Debug/quickstart visibility remains consistent with match-history policy:
  hidden from Recent Matches when appropriate, but still resolvable by bug-report review if a replay
  row was persisted.

## Handoff

After implementation, mark this phase done and summarize exactly how a later report API obtains the
match `replay_key`, how it marks that key as expecting final replay evidence, and how the dashboard should
distinguish pending, available, and missing replay states.
