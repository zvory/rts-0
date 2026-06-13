# Phase 4 - Tech and Production Managers

Status: Implemented.

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

## Implementation Notes

- Added selectable profile id `ai_1_0_tech`; `rifle_flood_full_saturation` remains unchanged and
  the only live random-lobby profile.
- `ai_1_0_tech` opens on Riflemen, expands off Training Centre, techs through Research Complex and
  Factory, trains Scout Cars while Tank research is blocked or pending, then prioritizes Tanks once
  Tank research completes.
- Fixed AI observation so Research Complexes are available to the shared production/research action
  path; without that, profile-backed live/self-play observations could build the tech structure but
  never issue Tank research.
- Added baseline scenario metadata for AI 1.0 early production, tech-blocked production, Scout Car
  unlock, and Tank unlock.

## Verification Results

- `cd server && cargo test -p rts-ai` passed: 126 passed, 1 ignored.
- Bounded matchup sample:
  `cargo run -p rts-ai --bin ai-matchup -- ai_1_0_tech rifle_flood_full_saturation --ticks 14000 --seed 1090519044 --json`
  passed replay verification and reached the required normal-start milestones before tick cap.
- `ai_1_0_tech` milestone timing in that sample: first Rifleman attack command tick 1703, expansion
  planned tick 7571, expansion completed tick 8432, first Scout Car tick 9179, first Tank tick 10409.

## Handoff to Phase 5

Phase 5 should build frontal-wave planning around Riflemen, Scout Cars, and Tanks for
`ai_1_0_tech`. Machine Gunners, Anti-Tank Guns, Artillery, and Command Cars are still intentionally
outside this profile's normal production arc. No remaining tech blocker is known after the Research
Complex observation fix; watch playtests for late Tank count and whether Scout Car spending delays
Tank mass too much.
