# Phase 1: AI 1.1 profile fork

## Status

Not started.

## Goal

Add a selectable `ai_1_1_tank_mg` profile that is an AI 1.0 fork with Scout Cars removed, ordinary Barracks capped at two, and a future-facing production shape that can make a bounded Machine Gunner defensive group while preserving Tank priority.

## Scope

- Add stable AI 1.1 constants and profile data in `server/crates/ai/src/ai_core/profiles.rs`.
- Register AI 1.1 in `required_profiles()` / `profile_by_id()` so `ai-matchup --list-profiles` and self-play matchup code can select it.
- Keep AI 1.0 registered for direct comparison.
- Add canonical aliases only if they are unambiguous. Suggested aliases:
  - `ai_1_1` -> `ai_1_1_tank_mg`
  - `ai11` -> `ai_1_1_tank_mg`
  - Keep `ai`, `ai1`, `ai_1_0`, and `default` pointing to AI 1.0 until replay evidence justifies promotion.
- Update `server/crates/ai/src/tools/matchup.rs` help text for the new profile/aliases.
- Update `docs/design/ai.md` to describe AI 1.1 as available in developer tooling but not the live lobby default.

## Behavioral requirements

- AI 1.1 ordinary Barracks target must never exceed two.
- AI 1.1 must have `harassment: None`.
- AI 1.1 tech-transition unit priorities must not include `EntityKind::ScoutCar`.
- AI 1.1 must still save for first Tank and require Methamphetamines before Tank production via the existing shared path.
- AI 1.1 should continue to require a Tank in frontal-wave attacks.
- Machine Gunner production should be bounded by profile data or a small new policy field, not by ad hoc checks in command emission.
- Phase 1 must not change the live lobby default.

## Expected touch points

- `server/crates/ai/src/ai_core/profiles.rs`
- `server/crates/ai/src/selfplay/replay.rs`
- `server/crates/ai/src/tools/matchup.rs`
- `docs/design/ai.md`
- Focused tests in `server/crates/ai/src/ai_core/profiles.rs` and/or `server/crates/ai/src/selfplay/replay.rs`

## Verification

- `cargo test -p rts-ai ai_1_1`
- `cargo run --bin ai-matchup -- --list-profiles`
- `cargo run --bin ai-matchup -- ai_1_1_tank_mg ai_1_0_tech --ticks 3000 --seed 7 --json`

## Manual testing focus

- Confirm `ai-matchup --list-profiles` shows both AI 1.0 and AI 1.1.
- Confirm the short JSON matchup runs without unknown-profile errors.
- Inspect the JSON scorecard for AI 1.1 and verify no Scout Car count appears in the early run.

## Handoff

After implementation, mark this phase done and summarize the exact AI 1.1 profile id, aliases, tests run, and any observed early-match scorecard differences. Tell the next agent where the bounded MG target is represented so Phase 2 can connect it to perimeter staging, and confirm the live lobby default is still AI 1.0.
