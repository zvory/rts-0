# Phase 3 - Golem Brief and Spec

Status: complete; user decisions recorded in [checklists.md](checklists.md) and
[requirements.md](requirements.md).

## Goal

Complete the new-unit checklist Phase 0 brief and Phase 1 rules/balance spec for Golem only. This
phase should define Golem as the economic and tech-conversion piece before any Golem-converted tech
building is designed.

## Scope

- Read [docs/context/balance.md](../../docs/context/balance.md) before editing rules/spec details.
- Use [docs/new-unit-checklist.md](../../docs/new-unit-checklist.md) for this unit.
- Complete only the Golem sections in [checklists.md](checklists.md), or mark items as deferred with
  named unknowns.
- Specify production, mining, transformation, supply, vulnerability, and consumption-healing policy
  only for Golem.
- Update [requirements.md](requirements.md) only when a Golem decision becomes approved product
  direction.

## Out of Scope

- Killing Tools, Anchorage, or Positioning briefs/specs except for dependency questions needed to
  keep Golem transformation coherent.
- Rust, JavaScript, protocol, generated config, tests, art, sound, scenario, replay, AI, or
  deployment changes.
- Implementing the specified rules.
- Future implementation phase files.

## User Interview Focus

- Is a Golem a unit the player controls, a worker-like economic body, a tech currency, a temporary
  summon, or a hybrid?
- What should a Golem feel like compared with four Kriegsia engineers?
- What does the player give up when transforming a Golem into a building?
- Should Golems be vulnerable while mining, transforming, or being consumed for healing?
- Can multiple Golems exist at once, and is there a desired cap?
- Is Golem production playable in the initial implementation, debug-only, or hidden?

## Expected Deliverables

- [checklists.md](checklists.md) updated only for Golem Phase 0 and Phase 1 items.
- [requirements.md](requirements.md) updated only for confirmed Golem product rules.
- No implementation files edited.

## Verification

- Documentation review only.
- Run `git diff --check` before committing.
- No automated gameplay suite is required for this docs-only phase.

## Manual Testing Focus

None. This phase has no gameplay change.

## Handoff Expectations

The handoff must name the approved Golem brief and rules, unresolved tuning questions, and exactly
one next active entity. By default, the next active entity is Killing Tools in
[phase-4.md](phase-4.md). If Golem is not approved, the handoff must say that Golem-converted
building work remains blocked.

## Handoff

Approved Golem brief:

- Golem is Ekat's directly controllable worker-like economy unit and tech-conversion piece.
- Golems are not a broad army roster unit, but they can attack with worker-like semantics.
- Golems concentrate four workers of value into one body: 4 Supply, 160 HP, 4x worker mining, and
  16 worker-like attack damage.
- Zamok builds Golems.
- Golems mine Steel or Oil near Zamok.
- Golems can be permanently consumed to heal Ekat or transformed into buildings.
- Killing or forcing commitment of Golems attacks Ekat's economy, tech path, and healing reserve.

Approved Golem rules:

- Supply: 4.
- HP: 160.
- Movement: worker-like ground movement, currently 2.0 px/tick.
- Sight: worker-like by default, currently 7 tiles.
- Attack: 16 damage using worker-like range, cooldown, and target filters by default.
- Mining: 4x worker mining rate, requiring Zamok proximity.
- Transformation: the Golem is permanently consumed; it disappears immediately; the target building
  immediately exists at low HP. Exact low-HP profile and completion timing are deferred.
- Healing: Ekat can consume a nearby owned Golem to heal to full HP. Exact range and command flow
  are deferred.
- Cap: no hard cap beyond normal Supply unless a later phase adds one.
- AI support and local prediction may remain disabled indefinitely for Ekat.

Unresolved tuning questions:

- Exact Golem cost.
- Golem build hotkey, command-card details, and build time.
- Exact collision, selection, and render size.
- Exact Golem-heal proximity range and command flow.
- Exact transformed-building starting HP profile and completion timing.
- Any future cap beyond normal Supply.

Next active entity:

- Killing Tools in [phase-4.md](phase-4.md).

No implementation files were edited.
