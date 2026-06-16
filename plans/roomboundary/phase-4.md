# Phase 4 - Live Tick Driver And AI Adapter

## Phase Status

- [ ] Not implemented.

## Objective

Extract live match tick orchestration while keeping `Game` AI-free and transport-free.

## Work

- Split the live `Phase::InGame` tick body into a room-local driver.
- Sequence AI command generation, `Game` ticking, snapshot fanout, observer analysis, defeat/outcome
  checks, and panic replay capture through explicit methods.
- Keep AI controllers in lobby/server code and enqueue ordinary `SimCommand`s through `Game`.
- Avoid changing `Game` signatures unless the design doc and all callers are updated in the same
  phase.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- New `server/src/lobby/live_tick.rs`
- `server/crates/ai/src/live.rs` only if adapter imports need cleanup
- `docs/design/ai.md` only if wording is stale

## Implementation Checklist

- [ ] Extract live tick sequencing into a room-local driver.
- [ ] Keep AI command generation outside `Game`.
- [ ] Preserve panic replay capture and outcome timing.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server ai`
- `cargo test --manifest-path server/Cargo.toml -p rts-server outcome`
- `cargo test --manifest-path server/Cargo.toml -p rts-ai live` if AI adapter behavior changes

## Manual Test Focus

Human versus AI match start, AI command activity, give-up/surrender, defeat outcome, and post-match
replay capture.

## Handoff Expectations

State whether `Game` signatures changed. If they changed, list docs and callers updated.
