# New Building Checklist

Use this checklist whenever adding a building, building-like entity, transformed structure, tech
structure, defense, drop-off, or faction home structure. Buildings should get the same up-front
brief and rules review as units because they affect economy, map control, tech pacing, fog, pathing,
UI, and player strategy.

Each building should get its own working checklist copied from this file or referenced from a
plan-specific checklist. Keep unfinished items visible so reviewers can see what is complete, what
is deferred, and what still needs a separate pass.

## Mandatory Workflow Gate

When starting any new building, development begins with requirements, not implementation. Do not
edit implementation code until Phase 0 and Phase 1 are complete, written down, and reviewed. The
only allowed files before that gate are planning, checklist, and design documents.

If the building belongs to a new unit, faction, or new production model, pair this checklist with
[docs/new-unit-checklist.md](new-unit-checklist.md). After Phase 0 and Phase 1 are complete, stop
and hand off the brief/spec unless the user explicitly says to proceed with implementation code.

## User Engagement Rule

Every building brief must be reviewed as a player-facing decision, not only as an implementation
object. The author should ask the user to choose the building's fantasy, reason to exist, tradeoff
against alternatives, expected counterplay, and what the player should learn after placing or losing
it. If the user gives a direction but not a number, record the direction as approved and mark the
number as deferred instead of guessing silently.

## Phase 0: Building Brief

Create a short building brief before touching implementation code.

- [ ] Name the building and identify whether the name is final, placeholder, or needs renaming.
- [ ] Define the player-facing description used by the UI.
- [ ] Define the strategic purpose: what problem does this building solve, and what should counter
      it?
- [ ] Define the building's relationship to the faction or unit that creates it.
- [ ] Decide whether it is built, transformed, summoned, placed at match start, repaired, consumed,
      upgraded, captured, or otherwise created.
- [ ] Identify the main player tradeoff for choosing this building over alternatives.
- [ ] List expected unusual interactions with existing units, buildings, terrain, fog, commands,
      upgrades, AI behavior, match history, or replay playback.
- [ ] Decide whether this building is allowed in the initial implementation, debug-only, hidden, or
      blocked until later polish.
- [ ] Start patch-note bullets for player-facing changes and keep them updated through the work.

Exit criteria:

- [ ] The brief is written down in the task, issue, design note, or commit body.
- [ ] Known unknowns are explicit instead of implied.
- [ ] No implementation files have been edited.
- [ ] The next step is either user review or a clearly scoped rules/balance spec.

## Phase 1: Rules And Balance Specification

Read [docs/context/balance.md](context/balance.md) first. If changing player-visible stats, update
the authoritative balance source and the mirrored client config together in the later implementation
phase.

- [ ] Creation source is specified: starting loadout, worker build, hero build, unit transform,
      ability, map script, or another rule.
- [ ] Creation command and hotkey are specified.
- [ ] Cost is specified, including free transforms, consumed units, refund rules, and cancellation.
- [ ] Build, transform, summon, or upgrade time is specified.
- [ ] Research prerequisite, tech prerequisite, build limit, or unlock timing is specified.
- [ ] Hit points are specified.
- [ ] Armor, armored status, tags, status immunities, repairability, capture rules, and special
      vulnerabilities are specified.
- [ ] Footprint, placement grid, terrain restrictions, collision, blocking, and pathing interactions
      are specified.
- [ ] Selection size, render size, construction size, damaged-state readability, and minimap
      behavior are specified.
- [ ] Sight range, fog reveal behavior, owner-only information, and remembered-building behavior are
      specified.
- [ ] Supply provided, supply used, population cap changes, or no-supply behavior is specified.
- [ ] Production, research, ability unlock, passive aura, drop-off, resource storage, or other
      economy/tech interactions are specified.
- [ ] Attack, defensive, targeting, projectile, area, or no-weapon behavior is specified.
- [ ] Death behavior is specified: wreckage, refund, death event, unlocked ability loss, tech loss,
      supply block, spawned units, or no special effect.
- [ ] Repair, rebuild, reclaim, transform-back, upgrade, or consume behavior is specified.
- [ ] AI availability and intended AI usage are specified.

Exit criteria:

- [ ] The building's numbers and rules can be reviewed without reading implementation code.
- [ ] Any unresolved tuning item is marked as deferred and assigned to a later pass.
- [ ] No implementation files have been edited.
- [ ] The next step is either user review or a clearly scoped implementation phase.

## Phase 2: Contract And Wire Design

Read [docs/context/protocol.md](context/protocol.md) before changing snapshots, commands, events,
construction state, tech unlocks, or wire-visible building state. Every protocol change must update
Rust and JS mirrors plus [docs/design/protocol.md](design/protocol.md).

- [ ] Confirm whether the existing building kind encoding is enough or a new tag is needed.
- [ ] Confirm whether snapshots need new fields for construction, transformation, passive aura,
      tech unlock, production, rally, damage, capture, or other state.
- [ ] Confirm whether client commands need new order types or fields.
- [ ] Define how cancellation, queued actions, interrupted construction, blocked placement, and
      invalid transforms appear on the wire.
- [ ] Define whether new transient events are required.
- [ ] Verify every event is player-facing and intentionally consumed.
- [ ] Verify fog gating for building views, target ids, construction events, destruction events,
      unlock state, ability effects, and remembered-building data.
- [ ] Verify inbound command validation: dedupe, cap, range-check, overflow-check, placement-check,
      and reject invalid target or coordinate data.
- [ ] Add or update protocol tests for new tags, fields, commands, events, and fog-filtered payloads.

Exit criteria:

- [ ] The server and client agree on every tag, field name, and shape.
- [ ] The wire format is tested before client visuals are considered complete.

## Phase 3: Simulation Implementation

Read [docs/context/server-sim.md](context/server-sim.md) before changing game systems, services,
orders, construction, production, economy, combat, AI, or self-play behavior.

- [ ] Add the building kind and creation path.
- [ ] Add authoritative stats and server-side rule handling.
- [ ] Implement placement, footprint, blocking, transform, or starting-loadout behavior.
- [ ] Implement production, research, ability unlock, aura, drop-off, or economy behavior.
- [ ] Implement attack or defensive behavior if relevant.
- [ ] Implement death, cleanup, refund, supply, ownership, and match-end interactions.
- [ ] Handle stale ids, invalid targets, impossible positions, cancelled actions, and interrupted
      orders as no-ops or recoverable errors.
- [ ] Avoid `unwrap()`, `expect()`, unchecked indexing, and unchecked arithmetic on tick or network
      paths.
- [ ] Search for unusual interactions and either test them or record them as explicit follow-up.

Exit criteria:

- [ ] `Game::tick()` remains panic-free for this building's paths.
- [ ] The building can be created, selected, interacted with, damaged, destroyed, and cleaned up in
      simulation tests.

## Phase 4: Client Commands, UI, And Debug Mode

Read [docs/context/client-ui.md](context/client-ui.md) before changing rendering, HUD, input, or
match modules.

- [ ] Add build, transform, production, research, ability, or command-card buttons with costs,
      hotkeys, tooltips, disabled states, and unavailable reasons.
- [ ] Ensure command routing can issue placement, transformation, cancellation, rally, production,
      research, ability, queued actions, and mode toggles for the building.
- [ ] Ensure the client understands every building state, order, event, and status field emitted by
      the server.
- [ ] Add lab creation or scenario support so humans can quickly spawn and inspect the building.
- [ ] Update dev scenarios, self-play fixtures, or manual debug scenarios where this building should
      appear.
- [ ] Ensure teardown remains clean for any new listeners, timers, textures, sounds, or GPU objects.

Exit criteria:

- [ ] A reviewer can create or inspect the building from a lab or scenario without playing a full
      tech path.
- [ ] The building can be controlled from the client using normal expected commands.

## Phase 5: Visual Design And Animation

Visuals can be rough in the first implementation, but every rough or missing asset must be tracked.

- [ ] Idle, construction, completed, damaged, selected, and destroyed appearances are specified.
- [ ] Transformation, upgrade, production, research, channeling, aura, or ability states are
      specified if relevant.
- [ ] Team color, footprint readability, entrance/facing cues, range indicators, rally markers, and
      targeting affordances are specified.
- [ ] Fog, occlusion, minimap, and remembered-building readability are checked.
- [ ] Placeholder art is clearly labeled if final art is deferred.

Exit criteria:

- [ ] The building is visually distinguishable from existing buildings at normal zoom.
- [ ] Missing visual polish is documented as follow-up instead of silently skipped.

## Phase 6: Audio Design

Audio is normally late-phase polish, but the design debt must be visible.

- [ ] Placement, construction start, construction complete, upgrade complete, production complete,
      selection, command acknowledgement, damaged, death, and ability sounds are specified if
      relevant.
- [ ] Ambient loop, aura loop, weapon loop, or channel loop behavior is specified if relevant.
- [ ] Repetition, volume, distance falloff, and spam risk are reviewed.
- [ ] Placeholder or missing sounds are tracked as deferred work.

Exit criteria:

- [ ] The building does not ship with accidental, misleading, or debug-only sounds.
- [ ] Deferred audio work is called out in patch notes or follow-up tasks.

## Phase 7: Test Matrix

Read [docs/context/testing.md](context/testing.md) before deciding which suites are required.

- [ ] Creation test: production path, debug spawn path, prerequisites, build time, supply, and
      affordability.
- [ ] Placement or transform test: valid position, blocked position, terrain restriction, overflow
      guard, cancellation, refund, and interrupted builder/source behavior.
- [ ] Interaction test: production, research, ability unlock, economy/drop-off, aura, repair,
      consume, upgrade, or other special behavior.
- [ ] Attack/defense test if relevant: range, target filters, cooldown, projectile or hitscan
      resolution, damage, death, target loss, and fog visibility.
- [ ] Wire test: building kind, snapshot fields, commands, events, construction state, invalid
      payloads, and client mirror parsing.
- [ ] Fog test: no hidden building, target id, death event, construction position, unlock state, or
      ability effect leaks to players who cannot see it.
- [ ] Robustness test: stale ids, duplicate ids, capped unit lists, bad coordinates, impossible
      targets, disconnected clients, repeated commands, and destroyed source entities.
- [ ] Client smoke/manual test: create the building, select it, issue expected commands, use its
      unlocked behavior, destroy it, and rematch without leaked listeners or WebGL resources.
- [ ] AI/self-play test if the AI can build, command, counter, or encounter the building.
- [ ] Regression test for any bug found during implementation.

Exit criteria:

- [ ] The selected suites match the changed files and risk.
- [ ] Any skipped suite has a concrete reason recorded in the final summary.

## Phase 8: Human Review Package

Make the change easy to review before merging.

- [ ] Keep the building implementation split into reviewable commits or phases.
- [ ] Summarize player-facing behavior and known deferred work.
- [ ] Include patch-note bullets for stats, economy, tech behavior, UI affordances, and expected
      strategic impact.
- [ ] Link or name the debug scenario used for human inspection.
- [ ] Call out cross-file contracts touched: balance mirror, protocol mirror, `Game` API, fog rules,
      hardening limits, or design docs.
- [ ] Confirm there are no stray debug logs, debug events, hidden console output, temporary UI
      labels, placeholder names, or accidental lobby/match messages.
- [ ] Confirm every deferred item has an owner, follow-up task, or explicit decision to leave it out.

Exit criteria:

- [ ] A reviewer can answer what changed, how to try it, what was tested, and what remains
      unfinished without reconstructing the whole implementation from code.
