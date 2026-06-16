# Phase 3 - Command Execution Context

## Phase Status

- [x] Done.

## Objective

Reduce command helper coupling by grouping command-time dependencies into a private execution
context.

## Work

- Introduce a context local to command application that carries the mutable stores and common facts
  currently threaded through most command helpers.
- Keep the context private to `commands.rs` or an area-local command module; do not expose it as a
  broad simulation API.
- Use the context to centralize repeated lookups such as team relationships, player faction id,
  AI budget exemption, notices, and planner facts.
- Avoid changing when `systems.rs` calls command application unless the call signature is narrowed
  without behavior changes.

## Expected Touch Points

- `server/crates/sim/src/game/services/commands.rs`
- Optional area-local command submodule if needed for readability
- `server/crates/sim/src/game/systems.rs` only if the outer call signature changes

## Implementation Checklist

- [x] Identify repeated parameter groups and shared lookup logic in command helpers.
- [x] Add a private command execution context.
- [x] Convert helpers incrementally while preserving existing validation order.
- [x] Keep or reduce archcheck broad-signature pressure.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim command`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim ability`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Test Focus

Smoke-test basic player commands, train/research/cancel, rally, build placement, and abilities in a
normal local match.

## Handoff Expectations

State whether the `apply_commands` signature changed, which dependencies remain intentionally
explicit, and whether any new archcheck allowance was required.
