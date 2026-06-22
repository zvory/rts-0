# Phase 4 - Broad Sim Game Test Split

Status: done.

## Goal

Split `server/crates/sim/src/game/tests.rs` into behavior-focused test modules without changing the
public `Game` API, setup helpers, or assertion meaning.

## Scope

- Read `docs/context/server-sim.md`, `docs/context/testing.md`, and the broad sim test section of
  `plans/hotspots/responsibility-map.md`.
- Convert `server/crates/sim/src/game/tests.rs` into a test module root or otherwise split it into
  domain files under `server/crates/sim/src/game/tests/`.
- Move shared fixtures first so domain files stay small.
- Split by behavior families such as ability/Ekat, artillery, fog/projection, mortar/smoke,
  movement replay/determinism, scoring/observer analysis, mining/resources, and tank traps.
- Preserve existing test names where practical, or leave aliases/comments when names must move.
- Confirm hotspot grouping still treats `server/crates/sim/src/game/tests/**` as `sim-tests`.

## Touch Points

- `server/crates/sim/src/game/tests.rs`
- new files under `server/crates/sim/src/game/tests/`
- module declarations needed for the Rust test layout
- `plans/hotspotcleanup/phase-4.md`

## Constraints

- Do not rewrite assertions while moving them.
- Do not change `Game` public methods, fixtures used by runtime code, fog visibility semantics, replay
  determinism, or observer analysis expectations.
- Do not split by arbitrary line chunks; use behavior families.
- Keep shared fixtures smaller than the original hotspot.

## Verification

- Focused `rts-sim` tests for the moved modules
- Any named domain tests that moved
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected for a pure test split. Manually inspect moved fog/projection,
replay determinism, and command replay assertions because those are the easiest places to accidentally
lose context during test movement.

## Handoff

After implementation, mark this phase done and summarize the test module tree, fixture placement,
commands run, any names changed, and the highest-risk moved assertion groups for future reviewers.
