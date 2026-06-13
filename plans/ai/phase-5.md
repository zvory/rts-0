# Phase 5 - Matchup Gates and Debug Visibility

Status: not started

## Goal

Prove the new AI is not worse than the old saturation AI and make failures inspectable. This phase
adds bounded matchup gates, debug traces, and rollout controls before the new AI becomes the normal
live-lobby default.

## Scope

- Add a bounded matchup harness for new AI versus `rifle_flood_full_saturation`.
- Run multiple seeds/configurations within an initial under-one-minute target for the normal AI
  regression command.
- Compare scorecard metrics when full elimination is too slow:
  - army value
  - economy value
  - tech milestones
  - expansion timing
  - attacks launched
  - damage dealt
  - buildings killed
- Add high-value server logs for AI phase changes, major tech transitions, expansions, attack wave
  launches, and repeated blockers. Avoid per-tick spam.
- Expose AI decision traces to self-play/watch or debug mode so a developer can see current phase,
  active targets, blockers, and recent commands in game.
- Add rollout wiring so the new AI can be promoted to default while retaining the old baseline as
  an explicit fallback.

## Expected Touch Points

- `server/crates/ai/src/selfplay/`
- `server/crates/ai/src/tools/`
- `server/src/tools/ai_perf_harness.rs` if useful
- `server/crates/ai/src/live.rs`
- `docs/design/ai.md`
- Debug/self-play UI only if a minimal trace display is cheaper than logs/artifacts

## Verification

- Focused matchup tests for new AI versus old saturation AI.
- Focused tests for scorecard calculation and deterministic seed handling.
- Tests or smoke checks proving trace/log output is bounded and meaningful.
- Existing AI integration lobby tests still pass with the old and new profile wiring.

## Manual Testing Focus

- Watch at least one new-vs-old self-play replay and inspect the trace output.
- Start a normal lobby match against the new profile only if rollout wiring exposes it safely.
- Confirm the old saturation AI remains available as a fallback.

## Handoff

The handoff should include matchup results, the recommended default/fallback setting, how to view
AI traces, and which metrics still need tuning before launch.
