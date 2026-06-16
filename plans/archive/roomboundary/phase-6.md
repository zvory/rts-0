# Phase 6 - Lifecycle Cleanup And Docs

## Phase Status

- [x] Done.

## Objective

Consolidate room lifecycle bookkeeping after replay, fanout, live tick, and branch boundaries exist.

## Work

- Consolidate start, end, reset, branch-live, dev-session, post-match replay, match-history, and
  drain bookkeeping behind explicit room-owned helpers.
- Preserve detached match-history writes and environment gating.
- Update server simulation design/context docs to describe the final lobby module map.
- Add a lightweight guardrail only if earlier phases reveal a repeatable boundary failure.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- Extracted lobby modules
- `docs/design/server-sim.md`
- `docs/context/server-sim.md`

## Implementation Checklist

- [x] Extract or rename lifecycle helpers after earlier modules are stable.
- [x] Preserve match-history and drain-start behavior.
- [x] Update docs with final lobby module boundaries.
- [x] Run verification and record exact results in the handoff.

## Verification

- Focused `cargo test --manifest-path server/Cargo.toml -p rts-server` lifecycle tests
- Match-history persistence tests if match-history code is touched
- `cargo clippy --manifest-path server/Cargo.toml -p rts-server -- -D warnings`
- `git diff --check`

## Manual Test Focus

Normal match from lobby to post-match replay to empty-room reset, replay room return behavior, and
drain-start behavior allowing active match completion.

## Handoff Expectations

Provide the final module map, remaining risks, and any follow-up plan candidates.
