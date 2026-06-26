# Phase 1 - Ekat Stats and Ability Unlock Gate

Status: complete; user decisions recorded in [checklists.md](checklists.md) and
[requirements.md](requirements.md).

## Goal

Approve the exact stat changes and ability availability rule for the already-implemented Ekat
hero/body. Ekat, Dash, Line Shot, Magic Anchor, return markers, line projectiles, and Magic Anchor
runtime already exist; this phase must not redesign or reimplement those systems.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md) only for the stat and
  availability parts of the existing Ekat unit.
- Complete only the Ekat Hero/Body checklist items needed for stats and ability availability in
  [checklists.md](checklists.md), or mark items as deferred with named unknowns.
- Specify exact HP, armor/tags, sight, size, speed, supply, cost, regeneration, death/recovery, and
  no-default-attack policy.
- Specify that Dash, Line Shot, and Magic Anchor are not available at match start.
- Specify the unlock source for each ability. The current code can enforce ability requirements
  through completed building kinds, but the named unlock buildings still need their own serial
  building specs before implementation can depend on them.
- Update [requirements.md](requirements.md) only when a stat or unlock decision becomes approved
  product direction.

## Out of Scope

- Rebuilding Ekat's body, Dash, Line Shot, Magic Anchor, return marker, projectile, or anchor
  runtime.
- Zamok, Golem, Death Box, Vortex, or Dash building briefs/specs except for naming the unlock
  source needed by the existing ability gate.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules before exact stats and unlock sources are approved.
- Future implementation phase files.

## User Interview Focus

- What exact Ekat stat line replaces the current 300 HP, 2.0 px/tick speed, 9-tile sight, 10 px
  radius, 0 supply, no default attack, and no cost?
- Should natural regeneration be removed entirely, matching the requirements draft?
- What is the recovery rule before Golem consumption exists?
- Which ability unlocks first, if any: Dash, Line Shot, or Magic Anchor?
- Is each ability unlocked by a completed building, a consumed/transformed Golem, a research-like
  flag, or another source?
- If the unlock buildings are not implemented yet, should Ekat ship temporarily with all combat
  abilities hidden/locked, debug-only unlocks, or a smaller first ability gate?

## Rules To Specify

- Starting state, owner, selection behavior, command-card role, and whether the current Ekat entity
  is reused.
- Cost, supply impact, buildability, respawn or revival rules, and match-start loadout.
- Hit points, armor/tags, sight, collision size, selection size, render size, movement speed, and
  movement semantics.
- Natural regeneration policy, Golem-consumption healing dependency, death behavior, and comeback
  behavior.
- Combat policy before tech buildings unlock abilities: no attack, basic attack, or another rule.
- Ability access policy for Dash, Line Shot, and Magic Anchor before and after unlocks exist.
- AI availability and prediction policy as they apply to controlling this hero/body.

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Ekat Hero/Body stat and ability-availability
  items.
- [requirements.md](requirements.md) updated only for confirmed Ekat stat and unlock product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Ekat stat line, natural regeneration decision, ability unlock
sources, unresolved tuning questions, and exactly one next active entity. By default, the next
active entity is Zamok in [phase-2.md](phase-2.md). If stat numbers or unlock sources are not
approved, the handoff must say that implementation remains blocked.

## Handoff

Approved Ekat stat line:

- One unique Ekat hero/body starts at match start for each Ekat player.
- Cost 0, Supply 0, not normally produced.
- 150 starting HP.
- 1.6 px/tick movement speed, matching Rifleman speed.
- 9-tile sight.
- Existing body radius, selection feel, render presentation, armor/tags, visuals, and ability
  runtime are reused unless a later implementation pass identifies a specific mismatch.
- No default attack.

Approved health and death rules:

- Ekat has no natural health regeneration.
- Until Golem consumption exists, damaged Ekat has no recovery.
- Golem consumption heals Ekat to full HP once implemented.
- Ekat death causes immediate player loss for the first implementation target.

Approved ability availability:

- Dash, Line Shot, and Magic Anchor are not unlocked at match start.
- Locked abilities stay visible but disabled in the command card.
- Death Box unlocks Line Shot.
- Vortex unlocks Magic Anchor.
- The Dash building currently named `XYZ` unlocks Dash.

Unresolved tuning questions:

- Future HP scaling mechanism.
- Future cloning mechanics.
- Future revival or comeback mechanics.
- Exact Golem consumption command flow.
- Replay support and any future AI/prediction support.

Next active entity:

- Zamok home/core structure in [phase-2.md](phase-2.md).

No implementation files were edited.
