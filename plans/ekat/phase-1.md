# Phase 1 - Ekat Hero/Body Brief and Spec

Status: planned.

## Goal

Complete the new-unit checklist Phase 0 brief and Phase 1 rules/balance spec for the Ekat
hero/body only. This phase should answer what the player does with the central controlled entity
before Zamok, Golem, or tech-building details are designed.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md) for this entity.
- Complete only the Ekat Hero/Body sections in [checklists.md](checklists.md), or mark items as
  deferred with named unknowns.
- Specify direct mining, regeneration, recovery, death, command-card exposure, and ability-lockout
  policy only as far as they affect the hero/body.
- Update [requirements.md](requirements.md) only when a hero/body decision becomes approved product
  direction.

## Out of Scope

- Zamok, Golem, Death Box, Vortex, or Dash building briefs/specs except for dependency questions
  needed to keep the Ekat hero/body coherent.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- What is the hero/body's battlefield role: harvester, duelist, raider, support caster, map-control
  piece, or something else?
- What should the UI description say in player-facing language?
- What does direct mining ask the player to do moment-to-moment?
- What should threaten Ekat while mining, fighting, or retreating?
- Should the current implemented Dash, Line Shot, Magic Anchor, and regeneration be preserved,
  replaced, hidden, or treated as debug-only?
- What should be in the first playable version, and what should wait for later entities?

## Rules To Specify

- Starting state, owner, selection behavior, command-card role, and whether the current Ekat entity
  is reused.
- Cost, supply impact, buildability, respawn or revival rules, and match-start loadout.
- Hit points, armor/tags, sight, collision size, selection size, render size, movement speed, and
  movement semantics.
- Direct mining command, valid resource targets, Zamok proximity requirement if any, deposit
  cadence, income rate, interruption behavior, and UI feedback.
- Natural regeneration policy, Golem-consumption healing dependency, death behavior, and comeback
  behavior.
- Combat policy before tech buildings unlock abilities: no attack, basic attack, or another rule.
- Ability access policy for Dash, Line Shot, and Magic Anchor before and after buildings exist.
- AI availability and prediction policy as they apply to controlling this hero/body.

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Ekat Hero/Body Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed hero/body product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Ekat hero/body brief and rules, unresolved tuning questions, and
exactly one next active entity. By default, the next active entity is Zamok in
[phase-2.md](phase-2.md). If the hero/body is not approved, the handoff must say that later entity
work remains blocked.
