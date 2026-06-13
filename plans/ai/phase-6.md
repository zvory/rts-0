# Phase 6 - Launch Tuning and Tactical Polish

Status: not started

## Goal

Tune and harden the new AI into the 1.0 launch opponent. This phase should use matchup results,
scenario failures, and playtest observations to improve strength while preserving fairness and
debuggability.

## Scope

- Tune phase unlocks and target values:
  - first attack timing
  - expansion timing
  - oil worker timing
  - Scout Car timing and count
  - tank tech timing
  - wave sizes and cadence
- Add or defer tactical polish based on risk:
  - Scout Car smoke against visible combat units
  - simple defensive Machine Gunner or AT Team production after tanks begin
  - route variation for harassment paths
  - additional 2v2 smoke coverage if cheap
- Promote the new AI to the normal live-lobby default only after the promotion bar in `plan.md` is
  satisfied.
- Keep the old saturation profile selectable and documented as the baseline.
- Refresh AI docs and patch-note bullets with player-facing behavior:
  - expands and techs more consistently
  - uses Riflemen, Scout Cars, and Tanks
  - sends staged waves and harassment

## Expected Touch Points

- `server/crates/ai/src/ai_core/profiles.rs`
- `server/crates/ai/src/live.rs`
- AI manager modules from earlier phases
- `docs/design/ai.md`
- Self-play/matchup tests

## Verification

- Fast scenario suite remains green.
- Bounded matchup gate passes against the old saturation AI.
- Focused live-lobby AI integration remains green.
- Optional longer self-play runs can be used for confidence, but should not replace fast gates.

## Manual Testing Focus

- Play or watch several 1v1 games against the launch AI from normal lobby flow.
- Confirm the AI feels fair: no impossible information, no hidden economy, no command behavior a
  human could not produce.
- Confirm a new player sees a clear progression from Riflemen to Scout Cars to Tanks.
- Confirm fallback profile selection still works.

## Handoff

The final handoff should include launch readiness, matchup metrics, known weaknesses, how to run the
fast AI regression target, and the exact fallback profile to use if live play reveals a blocker.
