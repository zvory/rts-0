# Phase 4 - Attack and Harassment Managers

Status: not started

## Goal

Implement the launch AI's army behavior: staged frontal attacks, Scout Car steel-line harassment,
and tank waves. This phase should make the AI feel more varied and dangerous than the current
saturation rifle-wave profile without adding advanced micro.

## Scope

- Army manager:
  - stage Riflemen until wave thresholds are met
  - launch frontal attack-move waves on a clear cadence
  - avoid trickling individual units unless a phase explicitly asks for harassment
  - launch tank waves once tank production is online and enough value is ready
- Scout Car harassment:
  - route Scout Cars toward the back side of the enemy steel line rather than the shortest path to
    the public enemy start
  - use ordinary movement/attack commands and public/fog-respecting information
  - keep behavior simple enough to be robust on current 1v1 maps
- Local defense:
  - preserve simple base-defense behavior from existing AI where it does not conflict with staged
    wave behavior
  - do not implement retreat/regroup, mortar dodging, split attacks, or focused target selection in
    this phase
- Optional if cheap and isolated:
  - first pass at Scout Car smoke against visible enemy combat units, especially Machine Gunners

## Expected Touch Points

- `server/crates/ai/src/ai_core/decision/raids.rs`
- New or existing army/harassment manager modules under `server/crates/ai/src/ai_core/`
- `server/crates/ai/src/ai_core/actions.rs`
- Scenario tests for attack and harassment

## Verification

- Fast tests for wave readiness, staging, and no-trickle behavior.
- Fast tests for Scout Car rear-steel-line route target selection on supported map layouts.
- Fast tests proving harassment commands use ordinary legal command shapes.
- Short scenario tests for:
  - Rifleman frontal wave
  - Scout Car harassment from an already-teched state
  - tank wave from an already-teched state
- Bounded self-play smoke checking that attacks are launched at least as early as the current
  saturation AI's first meaningful attack window.

## Manual Testing Focus

- Watch a normal 1v1 AI run and confirm the first attack arrives in the expected broad timing
  window.
- Watch a Scout Car harassment scenario and confirm cars path toward the enemy steel-line rear
  rather than only the enemy start tile.
- Watch a tank scenario and confirm tanks attack in groups rather than as a trickle.

## Handoff

The handoff should report wave thresholds, first-attack timing, Scout Car route assumptions, and
whether smoke usage was included or deferred.
