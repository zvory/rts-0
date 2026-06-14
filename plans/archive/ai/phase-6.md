# Phase 6 - Scout Car Harassment Manager

Status: Not implemented.

## Objective

Add Scout Car harassment  that avoids pathing through the frontal approach and attempts to attack the enemy base and steel line from behind. This should be the AI 1.0
second attack style while staying fog-respecting, deterministic, and simpler than full split-attack
micro.

## Scope

- Add a harassment manager that reserves a small Scout Car group separately from frontal-wave units.
- Choose harassment destinations from public enemy start/resource information and visible,
  fog-respecting observations.
- Route toward the enemy steel/oil patches from behind the enemy base, using a circuitious route to avoid being spotted. 
- React to visible combat units with ordinary attack or move intents, but do not require retreat,
  regroup, focused worker targeting, or hidden building ignore logic for AI 1.0.
- Add smoke usage only if the current action layer and unit ability contracts make it small and
  testable; otherwise leave smoke as an explicit follow-up.

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/raids.rs`
- new harassment-focused decision module if needed
- `server/crates/ai/src/ai_core/actions.rs`
- `server/crates/ai/src/ai_core/observation.rs` only if a fog-respecting observation field is
  needed
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/selfplay/`
- `docs/design/ai.md`

## Verification

- Add pure tests for harassment target selection, Scout Car reservation, coexistence with frontal
  waves, and visible-threat reaction.
- Add a scenario test that starts with Scout Cars available and verifies a harassment command routes
  toward the enemy steel-line back side.
- Run:

```bash
cd server && cargo test -p rts-ai
```

- Run bounded matchup samples and record harassment command timing, damage timing, and whether
  frontal attack metrics remain stable.

## Manual Testing Focus

Watch a replay with the new profile and confirm Scout Cars leave the main army, drive toward the
enemy steel-line back side, and do not use hidden information. Confirm the main frontal wave still
forms and attacks.

## Handoff Expectations

The handoff must state how harassment units are reserved, how targets are selected, and whether
smoke was implemented or deferred. It should include replay or matchup evidence that harassment did
not starve the frontal attack plan.

## Player-Facing Outcome

The new AI gains a readable harassment pattern that pressures resource lines without cheating or
requiring advanced micro.
