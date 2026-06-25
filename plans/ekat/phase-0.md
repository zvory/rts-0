# Phase 0 - Entity Briefs and User Interviews

Status: planned.

## Goal

Create user-reviewed briefs for the Ekat-controlled hero/body, the Zamok/home structure, Golem,
Death Box, Vortex, and the Dash building currently named `XYZ`. This phase should resolve the
player-facing identity and strategic intent of each entity before any rules numbers or code changes
are approved.

## Scope

- Review the existing Ekat requirements draft with the user as hypotheses, not final decisions.
- Reconcile the draft with the current playable Ekat hero/Zamok slice documented in the design
  docs.
- Complete the Phase 0 sections in [checklists.md](checklists.md), or mark items as deferred with
  named unknowns.
- Update [requirements.md](requirements.md) only when the user confirms a product decision or when
  an ambiguity must be recorded.
- Start draft patch-note bullets for any player-facing direction that would matter later.

## Out of Scope

- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Exact stat, cost, cooldown, timing, radius, footprint, or mining-rate implementation.
- Future implementation phase files.
- Recreating purged RTS-style Ekat content without explicit user approval.

## User Interview Flow

Work through one entity at a time. Keep the questions concrete enough that the user can answer from
game feel instead of implementation detail.

1. Read back the current draft for the entity in one or two sentences.
2. Ask what the player should do with it in a normal match.
3. Ask what strategic problem it solves and what should punish or counter it.
4. Ask what choosing it means giving up compared with the other Ekat options.
5. Ask what should be visible in the command card, map, fog, or selection UI.
6. Ask what must be in the first implementation, what can be debug-only, and what should be delayed.
7. Record the answer as approved, deferred, or rejected in [checklists.md](checklists.md).

## Required User Decisions

### Global Identity

- Is the controlled hero/body called Ekat, Zamok, or something else?
- Is Zamok a home structure, a hero title, a starting base, or another concept?
- Does the new requirements draft replace the current playable Ekat slice, layer on top of it, or
  stay hidden until it is complete?
- Should Ekat start with no combat abilities until tech buildings exist, even though the current
  implementation exposes Dash, Line Shot, and Magic Anchor?
- Should Ekat have no natural health regeneration, replacing the current implemented regeneration?
- Are Steel, Oil, and Supply still the only resources for this slice?
- Should AI and prediction remain blocked for Ekat in the first implementation pass?

### Ekat Hero/Body

- What is the battlefield role: harvester, duelist, raider, support caster, map-control piece, or
  something else?
- What should the UI description say in player-facing language?
- What does direct mining ask the player to do moment-to-moment?
- What should threaten Ekat while mining, fighting, or retreating?
- What unusual interactions are expected with fog, resource patches, Zamok proximity, command
  queueing, ability lockouts, and Golem consumption?
- Is Ekat playable in the initial implementation, debug-only, or hidden?

### Golem

- Is a Golem a unit the player controls, a worker-like economic body, a tech currency, a temporary
  summon, or a hybrid?
- What should a Golem feel like compared with four Kriegsia engineers?
- What does the player give up when transforming a Golem into a building?
- Should Golems be vulnerable while mining, transforming, or being consumed for healing?
- Can multiple Golems exist at once, and is there a desired cap?
- Is Golem production playable in the initial implementation, debug-only, or hidden?

### Zamok/Home Structure

- Is Zamok required for mining deposits, Golem production, Ekat revival, supply, victory, or some
  combination?
- What should happen if Zamok is destroyed?
- Should Zamok be buildable, fixed at match start, transformable, repairable, movable, or unique?
- Should Zamok provide +10 supply as the current implementation does, or is that a compatibility
  detail to revisit?
- What should the opponent learn from scouting or damaging Zamok?

### Death Box

- Is Death Box the final name?
- Why does the player choose Death Box over Vortex or the Dash building?
- What should Line Shot and its upgrades do before numbers are chosen?
- Should Death Box be fragile, durable, hidden, obvious, attackable, or mainly a tech commitment?
- What should happen to Line Shot access if Death Box is destroyed or transformed away?

### Vortex

- Is Vortex the final name?
- Why does the player choose Vortex over Death Box or the Dash building?
- What should Magic Anchor and its upgrades do before numbers are chosen?
- Should Vortex change battlefield space, defense, pursuit, escape, or economy?
- What should happen to Magic Anchor access if Vortex is destroyed or transformed away?

### Dash Building

- What is the final name for the building currently called `XYZ`?
- Why does the player choose the Dash building over Death Box or Vortex?
- What should Dash and its upgrades do before numbers are chosen?
- Is Dash primarily escape, engage, repositioning, mining tempo, or something else?
- What should happen to Dash access if this building is destroyed or transformed away?

## Expected Deliverables

- [checklists.md](checklists.md) updated with Phase 0 answers, deferrals, and rejected ideas.
- [requirements.md](requirements.md) updated only for confirmed product decisions or recorded
  ambiguity.
- Draft patch-note bullets for any later player-facing behavior implied by the approved brief.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must list the approved entity briefs, unresolved user questions, rejected ideas, and
any current-implementation behavior that the new brief intends to replace. It must also say that no
implementation files were edited and that Phase 1 should not assign exact numbers where Phase 0
left a decision unresolved.
