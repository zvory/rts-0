# Phase 1 - Name The Shared Late Game

## Goal

Introduce an explicit shared tank late-game policy in the AI profile data without changing the
strategic behavior meaningfully. The end of this phase should make the intended destination easy to
see in code before any profile is forced to use it.

## Plain-Language Intent

Right now several profiles have their own version of "eventually make tanks and attack." They are
similar, but not identical. This phase gives that idea a name and a single home.

The point is to make later changes mechanical: once there is a named shared policy, profiles can
opt into it without copying tank wave numbers everywhere.

## Current Anchors

Relevant existing data lives in `server/src/game/ai_core/profiles.rs`:

- `ProductionPolicy`
- `AttackPolicy`
- `TechTransitionPolicy`
- `RIFLE_FLOOD_FAST.tech_transition`
- `RIFLE_FLOOD_FULL_SATURATION.tech_transition`
- `TECH_TO_TANKS.production`
- `TECH_TO_TANKS.attack`
- `STEEL_EXPANSION_TANKS.tech_transition`

Relevant selection logic lives in `server/src/game/ai_core/decision.rs`:

- `active_tech_transition`
- `active_production_policy`
- `active_attack_policy`
- `active_required_tech_path`

## Proposed Shape

Add shared late-game constants or helpers near the existing profile constants:

- shared tank tech path: Barracks -> Training Centre -> Tank Factory;
- shared tank late-game production policy;
- shared tank late-game attack policy.

The first version can use the current tank+rifle shape that the live profiles already mostly share:

- production priorities: Tank, Rifleman;
- save for first tank: yes;
- production queue depth: common value;
- attack unit kinds: Tank, Rifleman;
- required unit: Tank;
- first attack size: common value;
- wave growth: common value;
- regroup reset: common value;
- reissue cadence: common value;
- stage distance: common value.

Do not over-design the type model yet. If constants are enough, use constants. If a small
`LateGamePolicy` struct makes the next phases cleaner, add it only if it removes real duplication.

## Implementation Notes

- Keep the shared policy inside `ai_core::profiles` unless another module genuinely needs it.
- Prefer names that describe behavior, not the current unit list only. For example,
  `SHARED_TANK_LATE_GAME_*` is clearer than `TANK_AND_RIFLE_*` if future support units are added.
- Keep profile ids unchanged. Replay and tests rely on stable ids.
- Avoid changing `AiDecisionMemory` in this phase unless tests show that shared policy selection
  needs explicit memory separation.
- Do not touch `LIVE_PROFILE_IDS` in this phase.

## Expected Behavior At End

Ideally no player-visible behavior changes. If a tiny behavior change is unavoidable because
existing duplicated constants disagree, call it out explicitly in the commit and patch notes.

At the end:

- shared late-game production and attack policy is visible in one place;
- existing profiles still have their current openings;
- existing profile ids and live profile pool are unchanged;
- no profile switching has been introduced;
- `rifle_flood_fast` recovery remains out of scope.

## Tests

Add or update narrow unit tests in `profiles.rs` if useful:

- shared late-game attack requires a tank;
- shared late-game attack includes tanks and riflemen;
- shared late-game production saves for the first tank;
- required live profile ids remain stable.

This phase does not need broad self-play coverage unless behavior changes.

## Done When

- The shared late-game policy has a clear name.
- Duplication is reduced or clearly ready to be reduced in Phase 2.
- Existing tests pass.
- No docs claim that live behavior has converged yet.
