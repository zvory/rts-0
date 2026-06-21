# New Unit Checklist

Use this checklist whenever adding a unit. New units must be split across multiple work sessions or
commits; do not try to design, implement, test, tune, polish, and ship a unit in one pass.

Each unit should get its own working checklist copied from this file. Keep unfinished items visible
so reviewers can see what is complete, what is deferred, and what still needs a separate pass.

## Mandatory Workflow Gate

When starting any new unit, development begins with requirements, not implementation. Do not edit
implementation code until Phase 0 and Phase 1 are complete, written down, and reviewed. The only
allowed files before that gate are planning, checklist, and design documents.

After Phase 0 and Phase 1 are complete, stop and hand off the brief/spec unless the user explicitly
says to proceed with implementation code.

## Phase 0: Unit Brief

Create a short unit brief before touching implementation code.

- [ ] Name the unit and its intended battlefield role.
- [ ] Define the player-facing description used by the UI.
- [ ] Define the strategic purpose: what problem does this unit solve, and what should counter it?
- [ ] List expected unusual interactions with existing units, buildings, terrain, fog, commands, or
      AI behavior.
- [ ] Decide whether this unit is allowed in the initial implementation, debug-only, or hidden until
      later polish.
- [ ] Start patch-note bullets for player-facing changes and keep them updated through the work.

Exit criteria:

- [ ] The brief is written down in the task, issue, design note, or commit body.
- [ ] Known unknowns are explicit instead of implied.
- [ ] No implementation files have been edited.
- [ ] The next step is either user review or a clearly scoped implementation phase.

## Phase 1: Rules And Balance Specification

Read [docs/context/balance.md](context/balance.md) first. If changing player-visible stats, update
the authoritative balance source and the mirrored client config together.

- [ ] Cost is specified.
- [ ] Supply impact is specified.
- [ ] Build source is specified: which building, unit, queue, or ability creates it.
- [ ] Build hotkey is specified.
- [ ] Build time is specified.
- [ ] Research prerequisite, tech prerequisite, or unlock timing is specified.
- [ ] Hit points are specified.
- [ ] Armor, armored status, tags, status immunities, or special vulnerabilities are specified.
- [ ] Sight range is specified.
- [ ] Collision size, selection size, and render size are specified.
- [ ] Movement speed is specified.
- [ ] Movement semantics are specified: ground, blocked-by-terrain, ignores collision, setup mode,
      cannot move while attacking, transport-like, or other special behavior.
- [ ] Pathing semantics are specified if different from ordinary ground units.
- [ ] Attack range is specified.
- [ ] Damage, cooldown, windup, projectile behavior, area behavior, and target filters are specified.
- [ ] Abilities are specified, including cost, cooldown, range, target rules, queued behavior, and
      cancellation behavior.
- [ ] Economy, repair, build, harvest, or production interactions are specified if relevant.
- [ ] AI availability and intended AI usage are specified.

Exit criteria:

- [ ] The unit's numbers and rules can be reviewed without reading implementation code.
- [ ] Any unresolved tuning item is marked as deferred and assigned to a later pass.
- [ ] No implementation files have been edited.
- [ ] The next step is either user review or a clearly scoped implementation phase.

## Phase 2: Contract And Wire Design

Read [docs/context/protocol.md](context/protocol.md) before changing snapshots, commands, events, or
wire-visible unit state. Every protocol change must update Rust and JS mirrors plus
[docs/design/protocol.md](design/protocol.md).

- [ ] Confirm whether the existing unit kind encoding is enough or a new tag is needed.
- [ ] Confirm whether snapshots need new fields for movement, attack, setup, ability, projectile, or
      status state.
- [ ] Confirm whether client commands need new order types or fields.
- [ ] Define how queued movement, queued attacks, queued abilities, cancellation, and interrupted
      orders appear on the wire.
- [ ] Define whether new transient events are required.
- [ ] Verify every event is player-facing and intentionally consumed; do not leak debug-only events
      such as raw fire markers into lobby or match messages.
- [ ] Verify fog gating for entity views, target ids, projectile/tracer state, death events, impact
      positions, and ability effects.
- [ ] Verify inbound command validation: dedupe, cap, range-check, overflow-check, and reject invalid
      target or coordinate data.
- [ ] Add or update protocol tests for new tags, fields, commands, events, and fog-filtered payloads.

Exit criteria:

- [ ] The server and client agree on every tag, field name, and shape.
- [ ] The wire format is tested before client visuals are considered complete.

## Phase 3: Simulation Implementation

Read [docs/context/server-sim.md](context/server-sim.md) before changing game systems, services,
orders, combat, movement, production, AI, or self-play behavior.

- [ ] Add the unit kind and creation path.
- [ ] Add authoritative stats and server-side rule handling.
- [ ] Implement movement and pathing behavior.
- [ ] Implement attack behavior, including windup, cooldown, projectile or hitscan resolution,
      target filters, and target loss behavior.
- [ ] Implement abilities, status effects, setup modes, or special order handling.
- [ ] Implement production, prerequisite, and queue behavior.
- [ ] Implement death, cleanup, supply, ownership, and match-end interactions.
- [ ] Handle stale ids, invalid targets, impossible positions, and interrupted orders as no-ops or
      recoverable errors.
- [ ] Avoid `unwrap()`, `expect()`, unchecked indexing, and unchecked arithmetic on tick or network
      paths.
- [ ] Search for unusual interactions and either test them or record them as explicit follow-up.

Exit criteria:

- [ ] `Game::tick()` remains panic-free for this unit's paths.
- [ ] The unit can be spawned, ordered, attacked, killed, and cleaned up in simulation tests.

## Phase 4: Client Commands, UI, And Debug Mode

Read [docs/context/client-ui.md](context/client-ui.md) before changing rendering, HUD, input, or
match modules.

- [ ] Add build button, command card, tooltip, description, cost display, and disabled state.
- [ ] Add or update hotkeys for building the unit and using abilities.
- [ ] Ensure command routing can issue movement, attacks, abilities, queued movements, queued
      attacks, queued abilities, cancellation, and mode toggles for the unit.
- [ ] Ensure the client understands every movement, attack, ability, order, event, and status field
      emitted for the unit.
- [ ] Add lab creation or scenario support so humans can quickly spawn and inspect the unit.
- [ ] Update dev scenarios, self-play fixtures, or manual debug scenarios where this unit should
      appear.
- [ ] Ensure teardown remains clean for any new listeners, timers, textures, sounds, or GPU objects.

Exit criteria:

- [ ] A reviewer can create the unit from a lab or scenario without playing a full tech path.
- [ ] The unit can be controlled from the client using normal and queued commands.

## Phase 5: Visual Design And Animation

Visuals can be rough in the first implementation, but every rough or missing asset must be tracked.

- [ ] Stationary appearance is specified.
- [ ] Moving appearance is specified if different.
- [ ] Attacking appearance is specified.
- [ ] Setup, deployed, channeling, ability, construction, damaged, and death states are specified if
      relevant.
- [ ] Facing, turret, barrel, projectile, tracer, recoil, muzzle flash, impact, and area indicators
      are specified if relevant.
- [ ] Selection, hover, health, range, ability radius, and targeting affordances are specified.
- [ ] Fog, occlusion, minimap, and team-color readability are checked.
- [ ] Placeholder art is clearly labeled if final art is deferred.

Exit criteria:

- [ ] The unit is visually distinguishable from existing units at normal zoom.
- [ ] Missing visual polish is documented as follow-up instead of silently skipped.

## Phase 6: Audio Design

Audio is normally late-phase polish, but the design debt must be visible.

- [ ] Attack start sound is specified.
- [ ] Projectile, travel, beam, or loop sound is specified if relevant.
- [ ] Attack landing or impact sound is specified.
- [ ] Ability cast, success, failure, toggle, setup, undeploy, or cooldown sounds are specified if
      relevant.
- [ ] Movement, idle, selection, command acknowledgement, production complete, and death sounds are
      specified if relevant.
- [ ] Repetition, volume, distance falloff, and spam risk are reviewed.
- [ ] Placeholder or missing sounds are tracked as deferred work.

Exit criteria:

- [ ] The unit does not ship with accidental, misleading, or debug-only sounds.
- [ ] Deferred audio work is called out in patch notes or follow-up tasks.

## Phase 7: Test Matrix

Read [docs/context/testing.md](context/testing.md) before deciding which suites are required.

- [ ] Unit creation test: production path, debug spawn path, prerequisites, build time, supply, and
      affordability.
- [ ] Movement test: ordinary path, blocked path, queued movement, interruption, stop, and special
      pathing semantics.
- [ ] Attack test: range, target filters, cooldown, windup, projectile or hitscan resolution, damage,
      death, target loss, and fog visibility.
- [ ] Ability test: cost, cooldown, range, valid targets, invalid targets, queued ability behavior,
      cancellation, and status cleanup.
- [ ] Wire test: unit kind, snapshot fields, commands, events, order queues, invalid payloads, and
      client mirror parsing.
- [ ] Fog test: no hidden entity, target id, projectile, death event, impact position, or ability
      effect leaks to players who cannot see it.
- [ ] Robustness test: stale ids, duplicate ids, capped unit lists, bad coordinates, impossible
      targets, disconnected clients, and repeated commands.
- [ ] Client smoke/manual test: create the unit, select it, order it, queue orders, attack with it,
      use abilities, kill it, and rematch without leaked listeners or WebGL resources.
- [ ] AI/self-play test if the AI can build, command, counter, or encounter the unit.
- [ ] Regression test for any bug found during implementation.

Minimum verification before merge:

- [ ] `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default`
- [ ] `node tests/server_integration.mjs` with a running server, if server/client integration changed.
- [ ] `node tests/regression.mjs` with a running server, if protocol, hardening, or network behavior
      changed.
- [ ] `node tests/ai_integration.mjs` with a running server, if AI/lobby behavior changed.
- [ ] `tests/run-all.sh --no-rust` if client rendering/input/UI changed.
- [ ] Architecture checks if server architecture or crate boundaries changed.

Exit criteria:

- [ ] The selected suites match the changed files and risk.
- [ ] Any skipped suite has a concrete reason recorded in the final summary.

## Phase 8: Human Review Package

Make the change easy to review before merging.

- [ ] Keep the unit implementation split into reviewable commits or phases.
- [ ] Summarize player-facing behavior and known deferred work.
- [ ] Include patch-note bullets for stats, economy, combat behavior, UI affordances, and expected
      strategic impact.
- [ ] Link or name the debug scenario used for human inspection.
- [ ] Call out cross-file contracts touched: balance mirror, protocol mirror, `Game` API, fog rules,
      hardening limits, or design docs.
- [ ] Confirm there are no stray debug logs, debug events, hidden console output, temporary UI labels,
      placeholder names, or accidental lobby/match messages.
- [ ] Confirm every deferred item has an owner, follow-up task, or explicit decision to leave it out.

Exit criteria:

- [ ] A reviewer can answer what changed, how to try it, what was tested, and what remains unfinished
      without reconstructing the whole implementation from code.
