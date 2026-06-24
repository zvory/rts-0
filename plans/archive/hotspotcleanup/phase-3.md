# Phase 3 - Command Service Guards and Tests

Status: done.

## Goal

Reduce context in `server/crates/sim/src/game/services/commands.rs` by splitting tests and extracting
pure command input guard helpers while preserving command semantics exactly.

## Scope

- Read `docs/context/server-sim.md`, `docs/context/testing.md`, and the command-service section of
  `plans/archive/hotspots/responsibility-map.md`.
- Split command-service tests by behavior family where they currently live inside
  `commands.rs`.
- Extract pure helper code for input shaping, id dedupe/capping, command-budget validation,
  non-finite target rejection, and authority checks when those helpers can be named narrowly.
- Keep `pub(crate) fn apply_commands` as the orchestration entry point.
- Keep issue-time mutation, receipt ordering, resource mutation order, replay determinism, and queued
  order behavior stable.
- Update hotspot grouping only if new command-service paths are not already matched by
  `server/crates/sim/src/game/services/commands/`.

## Touch Points

- `server/crates/sim/src/game/services/commands.rs`
- possible new files under `server/crates/sim/src/game/services/commands/`
- possible module declarations in `server/crates/sim/src/game/services/mod.rs`
- focused command-service tests
- `plans/hotspotcleanup/phase-3.md`

## Constraints

- Do not change command acceptance semantics, client-visible command budget constants, protocol
  command payloads, or replay command behavior.
- Do not split ordering-sensitive orchestration until a later design pass proves it safe.
- Do not add service-to-service import edges without running the sim architecture check and justifying
  the edge.
- Keep `Game::tick()` panic-free.

## Verification

- Focused Rust tests for moved command-service behavior
- Command replay and command-budget tests if touched or moved
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `git diff --check`

## Manual Testing Focus

Use code review as the main manual check: compare moved guards and tests against the original
ordering. If any runtime command path moved beyond pure validation, manually smoke-test issuing move,
build, train, attack, and command-card actions in a local match.

## Handoff

After implementation, mark this phase done and summarize the new command-service layout, which
helpers are pure guards, which orchestration remains in `apply_commands`, commands run, and any
ordering-sensitive code deliberately left in place.
