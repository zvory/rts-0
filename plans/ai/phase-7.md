# Phase 7 - Promotion

Status: Implemented.

## Objective

Promote the new AI 1.0 profile after it satisfies the promotion bar. This phase makes
`ai_1_0_tech` the live lobby AI behavior. Per implementation direction, the old AI profiles are
removed from selection/tooling, there is no live fallback, and AI profiles are not selectable from
the lobby.

## Scope

- Run the promotion evidence suite against the new profile.
- Make `ai_1_0_tech` the live default and sole live profile id.
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
opening, expansion, Scout Car harassment, Tank conversion, and frontal waves.

## Handoff Expectations

The handoff must include the promotion evidence, final live profile/default decision, and factual
patch-note bullets. It must list known AI 1.0 limits, especially any deferred smoke, retreat, split
attack, support-unit, or micro behavior.

## Player-Facing Outcome

Live lobby AI becomes stronger and more varied, with readable Rifleman pressure, expansion, Scout
Car harassment, and Tank conversion. Previous experimental profiles are no longer selectable by
developer tooling or live code.

## Completion Notes

- `DEFAULT_LIVE_PROFILE_ID` is now `ai_1_0_tech`.
- The live profile pool contains only `ai_1_0_tech`.
- Unknown live profile ids fall back to `ai_1_0_tech`.
- Older experimental profile ids are no longer registered for self-play or matchup tooling.
- Player-facing patch notes:
  - Lobby AI now opens with four-Rifleman frontal waves instead of the previous saturation rifle
    behavior.
  - Lobby AI can expand from a completed Training Centre, send Scout Cars on harassment moves, and
    pivot into Tanks after Tank research and Methamphetamines.
  - Known limits remain: no Scout Car smoke usage, no harassment retreat/regroup micro, no split
    attack planning beyond the Scout Car reservation, and no Machine Gunner, Anti-Tank Gun,
    Artillery, or Command Car branch in AI 1.0.
