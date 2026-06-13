# Phase 4 - Tech and Production Managers

Status: Not implemented.

## Objective

Implement the AI 1.0 tech and production spine as a selectable profile. The profile should reliably
move from Riflemen to Scout Cars to Tanks from normal starts before it is promoted to the live
default.

## Scope

- Add the new profile id for AI 1.0 while keeping `rifle_flood_full_saturation` unchanged and
  selectable.
- Add a tech manager that tracks prerequisites, saving behavior, blocked tech goals, and completed
  milestone transitions.
- Add a production manager that chooses Riflemen first, then Scout Cars for harassment availability,
  then Tanks once the economy and tech path can support them.
- Keep Machine Gunners, Anti-Tank Guns, Artillery, and Command Cars out of the first implementation unless
  a focused defensive support case is explicitly required.
- Add authored scenarios for early production, tech-blocked production, Scout Car unlock, and Tank
  unlock without requiring 10,000 setup ticks.

## Expected Touch Points

- `server/crates/ai/src/ai_core/profiles.rs`
- `server/crates/ai/src/ai_core/decision/production.rs`
- `server/crates/ai/src/ai_core/decision/policies.rs`
- `server/crates/ai/src/ai_core/decision/mod.rs`
- `server/crates/ai/src/ai_core/decision/tests.rs`
- `server/crates/ai/src/selfplay/`
- `docs/design/ai.md`

## Verification

- Add pure decision tests for tech blockers, save-for-tech decisions, production priority order,
  and first-unit milestone transitions.
- Add scenario tests that prove the new profile reaches Riflemen, Scout Cars, expansion, and Tanks.
- Run:

```bash
cd server && cargo test -p rts-ai
```

- Run bounded matchup samples against `rifle_flood_full_saturation` and record whether the new
  profile reaches the required milestones before elimination or tick cap.

## Manual Testing Focus

Open a profile-vs-baseline replay and confirm the new AI opens with readable Rifleman pressure,
builds the Scout Car tech path, and eventually produces Tanks without getting stuck behind supply,
oil, or prerequisite blockers.

## Handoff Expectations

The handoff must name the new profile id, summarize milestone timing from normal starts, list any
remaining tech blockers, and tell Phase 5 which combat units are ready for frontal-wave planning.

## Player-Facing Outcome

A new selectable AI profile can tech and produce the required AI 1.0 unit arc. It should not become
the live default yet.
