# Phase 6: Gated gameplay adoption

Status: Pending

## Goal

Enable route-aware AI behavior in small, reviewable slices after the analysis and diagnostics have
been validated.

## Scope

- Start with the lowest-risk behavior: route-aware staging or attack-move waypoints for frontal
  waves.
- Then enable defensive staging near route/choke candidates if overlays and traces show candidate
  quality is reliable.
- Only enable tank-trap route blocking after placement legality, pathing interaction, and bypass
  behavior are covered by focused tests.
- Keep feature flags or profile gates where useful so behavior can be rolled back without removing
  the analyzer.
- Update AI traces so every route-aware command explains the route/choke/candidate id that caused it.
- Collect factual patch-note bullets for each behavior change.

## Non-goals

- Do not enable all decision families in one PR.
- Do not bypass existing command validation, build placement validation, fog rules, or pathing.
- Do not introduce hidden-information route scoring.

## Expected touch points

- AI decision modules enabled one at a time
- AI profiles if feature gates or promoted defaults change
- Tank-trap placement/command helpers if route blocking is enabled
- Self-play matchup tests and focused decision tests
- `docs/design/ai.md`

## Verification

- Run focused decision tests for the enabled decision family.
- Run targeted self-play or matchup coverage comparing pre/post behavior for the affected profile.
- For tank-trap behavior, run focused pathing and placement tests covering infantry pass-through,
  vehicle blocking, and enemy breachability.

## Manual testing focus

Watch AI-vs-AI spectator games with map routes and AI traces visible. Confirm route-aware commands
match the overlay, attacks remain readable, defenses appear near sensible approaches, and tank traps
do not accidentally wall the AI into its own base.

## Handoff

The handoff must include player-facing gameplay impact, patch-note bullets, known tuning risks, and
the next behavior slice that should or should not be enabled.
