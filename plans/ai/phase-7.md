# Phase 7 - Promotion, Live Selection, and Rollback

Status: Not implemented.

## Objective

Promote the new AI 1.0 profile only after it satisfies the promotion bar. This phase should make
the profile available in live lobby AI selection while preserving `rifle_flood_full_saturation` as
a named rollback baseline.

## Scope

- Run the promotion evidence suite against the new profile and `rifle_flood_full_saturation`.
- Add the new profile to the live profile pool only after scenario and matchup gates pass.
- Decide whether the live default should become the new profile or whether the profile pool should
  include both behaviors temporarily.
- Preserve deterministic test coverage for default profile id, profile pool contents, and unknown
  profile fallback behavior.
- Update design docs and patch notes with the actual player-facing behavior and known limits.

## Expected Touch Points

- `server/crates/ai/src/live.rs`
- `server/crates/ai/src/ai_core/profiles.rs`
- `server/crates/ai/src/selfplay/tests.rs`
- `server/crates/ai/src/tools/`
- `docs/design/ai.md`
- this plan's completed phase status markers

## Verification

- Run focused AI tests:

```bash
cd server && cargo test -p rts-ai
```

- Run full AI coverage because this phase changes live strategy selection:

```bash
cd server && RTS_FULL_AI_TESTS=1 cargo test
```

- Run representative bounded matchups or the AI balance matrix for the final profile set.
- If lobby behavior changes, run the live AI lobby integration suite with a running server:

```bash
node tests/ai_integration.mjs
```

## Manual Testing Focus

Start a 1-human + 1-AI lobby match and confirm the live AI uses the promoted behavior: Rifleman
opening, expansion, Scout Car harassment, Tank conversion, and frontal waves. Also run or inspect a
baseline profile replay to confirm `rifle_flood_full_saturation` still works as rollback.

## Handoff Expectations

The handoff must include the promotion evidence, final live profile/default decision, rollback
instructions, and factual patch-note bullets. It must list known AI 1.0 limits, especially any
deferred smoke, retreat, split attack, support-unit, or micro behavior.

## Player-Facing Outcome

Live lobby AI becomes stronger and more varied, with readable Rifleman pressure, expansion, Scout
Car harassment, and Tank conversion. The previous saturation AI remains available as a rollback
baseline.
