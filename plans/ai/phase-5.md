# Phase 5 - Frontal Wave Attack Manager

Status: Not implemented.

## Objective

Add the AI 1.0 frontal staged-wave attack manager. The manager should preserve the current broad
first-attack timing while making wave readiness, staging, visible-target reactions, and reissue
cadence explicit and testable.

## Scope

- Move frontal attack planning into a manager that receives available combat groups and returns
  stage or attack intents.
- Define wave readiness using unit composition, minimum force, required-unit gates, and time/supply
  fallback signals.
- Add a tech gate for Tank frontal waves: the AI must research Methamphetamines at the Training
  Centre before attacking with Tanks. Prefer making this a prerequisite before producing the first
  Tank if that is cleaner for the tech/production manager, so Tank production and Tank-wave readiness
  cannot race ahead of the upgrade.
- Preserve local defense priority so home threats can interrupt outgoing waves without breaking
  unrelated attack groups.
- Add blocker and trace output for "waiting for units", "waiting for tank", "staging", "attacking",
  "waiting for Methamphetamines", and "reissuing".
- Keep retreat/regroup micro, focused unit targeting, mortar dodging, and split attacks out of AI
  1.0 unless later evidence requires them.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/raids.rs`
- `server/crates/ai/src/ai_core/decision/defense.rs`
- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/actions.rs`
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/selfplay/`
- `docs/design/ai.md`

## Verification

- Add pure tests for wave readiness, staging positions, reissue cadence, required tank gates,
  Methamphetamines-before-Tank-attack gating, and visible combat target selection.
- Add scenario tests for early Rifleman waves and later tank-supported waves.
- Run:

```bash
cd server && cargo test -p rts-ai
```

- Run bounded matchup samples and compare first attack timing, attack command count, damage timing,
  army value, and deaths against the Phase 1 baseline.

## Manual Testing Focus

Watch a replay where the new AI stages before attacking and confirm the wave looks readable:
Riflemen pressure early, later Tank groups do not dribble one unit at a time unless the profile says
they should, and local defense still responds to visible pressure.

## Handoff Expectations

The handoff must state the frontal-wave gates, expected first-attack timing window, and any matchup
metrics that improved or regressed. It should tell Phase 6 how harassment groups can coexist with
frontal-wave reservations. It must also state whether Methamphetamines is enforced before first
Tank production or only before Tank attack launch, and include the observed timing impact.

## Player-Facing Outcome

The new AI profile applies clearer frontal pressure and should feel less passive while still using
fair, ordinary commands.
