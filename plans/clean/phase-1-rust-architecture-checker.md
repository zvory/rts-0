# Phase 1 - Rust Architecture Checker

## Objective

Create a Rust-owned architecture check for `rts-sim::game` internals. This replaces the earlier
placeholder idea of `scripts/check-sim-architecture.mjs`: that name meant "a script that checks sim
architecture", but this phase should implement the checker in Rust.

## Work

- Add a small workspace tool crate, for example `server/crates/archcheck`, with a binary such as
  `check-sim-architecture`.
- The binary should scan `server/crates/sim/src/game` and report structural violations with stable,
  human-readable messages.
- Use simple source scanning first where it is reliable:
  - file line counts
  - `use crate::game::services::...` edges
  - direct `PlayerState` argument usage
  - direct `Entity` field writes outside approved modules
  - broad `pub` / `pub(crate)` exports
- Use Rust parser support only where regex becomes brittle. `syn` is reasonable for function
  signatures and item visibility if the simple scanner produces false positives.
- Keep the checker deterministic and fast enough to run in commit hooks.

## Initial Checks

- Detect service-to-service imports and compare them to an allowlist.
- Detect pure-policy modules and forbid imports of:
  - `EntityStore`
  - `PlayerState`
  - `Fog`
  - `MoveCoordinator`
  - `SmokeCloudStore`
  - `protocol::Event`
- Detect functions with very broad mutable world signatures, especially functions accepting both
  `&mut EntityStore` and `&mut [PlayerState]`.
- Detect large files, but only as budget data until Phase 2 adds ratcheting.

## Verification

- `cargo run -p rts-archcheck -- check-sim-architecture` prints `sim architecture check passed` on
  the current baseline.
- Add unit tests for the checker's rule parsing using tiny fixture strings.
- Add at least one fixture that proves a pure-policy module importing `EntityStore` fails.

## Outcome

No gameplay change. The repo gains a Rust enforcement point that future agents can run before and
after sim work.
