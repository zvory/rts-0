# Phase 8 - AI Self-Play Test Split

Status: planned.

## Goal

Split `server/crates/ai/src/selfplay/tests.rs` and harness helpers by domain while preserving quick
test defaults, full-AI gates, and replay artifact behavior.

## Scope

- Read `docs/context/server-sim.md`, `docs/context/testing.md`, and the AI self-play section of
  `plans/hotspots/responsibility-map.md`.
- Split harness/artifact helpers from profile matchup tests.
- Split resource regression, pending-build tracker, live-AI team behavior, and full real-AI tests
  into domain files.
- Keep `RTS_FULL_AI_TESTS=1` behavior explicit and avoid silently expanding default runtime.
- Preserve replay artifact schema, writing behavior, and failure diagnostics.
- Confirm the existing `ai` architectural group still covers all new files under `server/crates/ai/`.

## Touch Points

- `server/crates/ai/src/selfplay/tests.rs`
- new self-play test modules under the AI crate
- possible shared test fixture modules inside AI test ownership
- `plans/hotspotcleanup/phase-8.md`

## Constraints

- Do not change AI decision behavior, profile behavior, resource rules, live-AI team semantics, or
  replay artifact schema.
- Do not turn long full-AI checks into default quick tests.
- Do not hide failure artifact output behind helper indirection that makes failed self-play hard to
  inspect.

## Verification

- Focused `rts-ai` self-play tests that moved
- `RTS_FULL_AI_TESTS=1` focused self-play coverage only if long-test paths or full-AI gates move
- Replay artifact schema tests after helper extraction
- `git diff --check`

## Manual Testing Focus

No gameplay manual test is expected for a pure test split. If a self-play failure appears and the
reason is not obvious, follow the repo self-play failure protocol and open the replay artifact with
macOS `open`.

## Handoff

After implementation, mark this phase done and summarize the test module tree, unchanged quick/full
gates, commands run, any long tests skipped, and how failure artifacts should be inspected next.
