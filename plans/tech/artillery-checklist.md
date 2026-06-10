# Artillery Unit Checklist

Working checklist for `plans/tech/phase-6-capstones.md`, following
`docs/new-unit-checklist.md`.

## Phase 0: Unit Brief

- [x] Name the unit and its intended battlefield role.
  - Artillery: late Superior Firepower capstone and extreme-range positional siege weapon.
- [x] Define the player-facing description used by the UI.
  - Short description: "Slow, expensive long-range siege gun."
  - Tooltip draft: "Must deploy into a narrow firing arc, cannot fire nearby, and spends steel with
    every shot."
- [x] Define the strategic purpose: what problem does this unit solve, and what should counter it?
  - Solves entrenched positions and remote economy denial after Superior Firepower survives the
    Mobile Warfare tank timing.
  - Counters should include scouting, flanks, smoke or breakthrough pressure once implemented,
    minimum-range abuse, and forcing teardown/reposition.
- [x] List expected unusual interactions with existing units, buildings, terrain, fog, commands, or
      AI behavior.
  - Point fire repeats at the same target point until stopped, redirected, out of steel, or ordered
    to move.
  - Multiple selected Artillery units firing at one point should spread their target points so they
    do not all hit the exact same location and overkill. The spreading algorithm can be chosen
    during implementation.
  - The firing player sees target/impact preview markers and sees the resulting explosions even in
    fog.
  - Enemies do not see pre-impact markers, even if they have vision of the firing gun, but do see
    the explosion after impact even through fog.
  - Accuracy resets on any move order or teardown. Changing target point or pausing fire does not
    reset accuracy.
  - Point fire automatically sets up Artillery facing the target point. Players do not need a
    separate explicit setup order before firing.
  - Artillery has no attack while not set up.
  - Setup and teardown each take 3 seconds and should have visible setup/teardown animation.
  - Artillery cannot rotate while deployed. Changing its firing arc requires teardown/redeploy.
  - Point-fire orders outside the deployed arc, inside minimum range, or outside maximum range are
    rejected with normal command-failure feedback; the unit should not move or redeploy to satisfy
    that order.
  - The shared AT Gun-style deploy command should support redeploying Artillery when already
    deployed.
  - Point fire is terminal for the current queue: once Artillery starts firing at a position, it
    continues firing there and later queued orders should not be accepted after that point-fire
    order.
  - Artillery uses tank-style oriented pivot hull movement. It should read visually as roughly
    2 tiles by 1.5 tiles. Collision may use capsule or otherwise shaved-down geometry so it remains
    usable in snaking/depth pathfinding scenarios.
  - Artillery has a new `Hard` armor class: non-armor-piercing damage is reduced by 25%;
    armor-piercing damage applies in full.
  - Artillery shots damage friendly units and buildings.
  - Artillery explosions render at the impact point without changing fog or exploration state.
  - If Artillery lacks the 10 steel ammunition cost when a shot would fire, it holds the order,
    emits normal insufficient-steel feedback, waits 3 seconds, then tries to fire again.
  - AI ignores Artillery for the first pass.
- [x] Decide whether this unit is allowed in the initial implementation, debug-only, or hidden until
      later polish.
  - Available in normal matches immediately after the Gun Works Artillery unlock is researched.
- [x] Start patch-note bullets for player-facing changes and keep them updated through the work.
  - Superior Firepower gains Artillery as a late Gun Works unit after a new Gun Works upgrade.
  - Gun Works can research Unlock Artillery for 200 steel / 200 oil over 30 seconds.
  - Artillery is planned as a 300 steel / 100 oil, 5-supply siege unit with 150 HP, 10-50 tile point
    fire, a 20-degree firing arc, 3-second reload, and 10 steel ammunition cost per fired shot.
  - Artillery has 4-tile sight, tank-length build time, and moves 20% slower than AT Guns.
  - Artillery uses tank-style oriented pivot movement, reads visually as roughly 2x1.5 tiles, and
    has a new Hard armor class that reduces non-armor-piercing damage by 25%.
  - Artillery automatically sets up toward point-fire targets, takes 3 seconds to set up or tear
    down, and cannot rotate while deployed.
  - Artillery shells take 4 seconds to land after firing.
  - Artillery impact is planned to deal 150 armor-piercing damage in a 1-tile radius, with
    non-armor-piercing splash falloff to 10 damage at 3 tiles.
  - Artillery damage includes friendly fire.
  - Multi-Artillery point fire spreads shots around the ordered point to reduce overkill.
  - Artillery explosions are visible to all players after impact even through fog; only the firing
    player sees pre-impact target markers.

Exit criteria:

- [x] The brief is written down in the task, issue, design note, or commit body.
- [x] Known unknowns are explicit instead of implied.
  - Collision geometry is intentionally left to the simulation implementation pass, but the
    player-facing movement/readability requirements are fixed above.

## Phase 1: Rules And Balance Specification

Read: `docs/context/balance.md`, `docs/design/balance.md`.

- [x] Cost is specified.
  - 300 steel / 100 oil.
- [x] Supply impact is specified.
  - 5 supply.
- [x] Build source is specified: which building, unit, queue, or ability creates it.
  - Trained from Gun Works.
- [x] Build hotkey is specified.
  - Train hotkey: `E`.
- [x] Build time is specified.
  - Tank-length build time: 750 ticks, approximately 25 seconds at 30 Hz.
- [x] Research prerequisite, tech prerequisite, or unlock timing is specified.
  - Requires Gun Works research `Unlock Artillery`: 200 steel / 200 oil, 900 ticks
    (30 seconds), UI description "Unlocks production of Artillery".
- [x] Hit points are specified.
  - 150 HP.
- [x] Armor, armored status, tags, status immunities, or special vulnerabilities are specified.
  - New `Hard` armor class: non-armor-piercing damage is reduced by 25%;
    armor-piercing damage applies in full.
- [x] Sight range is specified.
  - 4 tiles.
- [x] Collision size, selection size, and render size are specified.
  - Visual/readability target is roughly 2 tiles by 1.5 tiles, larger than a tank and about twice
    the AT Gun's footprint.
  - Collision implementation may use capsule or shaved-down geometry as long as movement remains
    usable in snaking/depth pathing scenarios.
- [x] Movement speed is specified.
  - 20% slower than AT Guns. Current AT Gun speed is 1.152 px/tick, so Artillery target speed is
    0.922 px/tick before final constant rounding.
- [x] Movement semantics are specified: ground, blocked-by-terrain, ignores collision, setup mode,
      cannot move while attacking, transport-like, or other special behavior.
  - Ground unit with tank-style oriented pivot hull movement.
  - Must tear down before moving after deployment.
  - Cannot move while setting up, deployed, firing, or tearing down.
- [x] Pathing semantics are specified if different from ordinary ground units.
  - Uses ordinary blocking terrain/collision, with tank-style orientation and larger hull handling.
- [x] Attack range is specified.
  - Minimum range: 10 tiles.
  - Maximum range: 50 tiles.
  - Deployed field of fire: 20 degrees total.
- [x] Damage, cooldown, windup, projectile behavior, area behavior, and target filters are
      specified.
  - Point-fire only; target is a world position, not a unit.
  - Setup: 3 seconds. Teardown: 3 seconds.
  - Reload: 3 seconds.
  - Ammunition cost: 10 steel, paid only when a shot actually fires.
  - Shell travel: 4 seconds after firing.
  - Accuracy starts at 5-tile CEP on the first shot after setup and improves to 2-tile CEP on the
    fifth shot after setup while maintaining fire.
  - Impact deals 150 armor-piercing damage in a 1-tile radius.
  - Outside the 1-tile radius, impact deals non-armor-piercing splash falloff from 150 damage down
    to 10 damage at 3 tiles.
  - No splash damage beyond 3 tiles.
  - Damage affects enemy and friendly units/buildings.
- [x] Abilities are specified, including cost, cooldown, range, target rules, queued behavior, and
      cancellation behavior.
  - Point Fire hotkey: `X`.
  - Point Fire automatically sets up toward the target if packed.
  - Point Fire is terminal in an order queue; later queued orders should not be accepted after it.
  - Invalid point-fire orders are rejected without moving, redeploying, or spending steel.
  - Stop, move, teardown, and redirect orders cancel or replace current fire according to normal
    command semantics; movement/teardown resets accuracy.
- [x] Economy, repair, build, harvest, or production interactions are specified if relevant.
  - Requires the new Gun Works unlock before production.
  - Each fired shot costs 10 steel; failed/rejected/unaffordable pending shots do not spend steel.
- [x] AI availability and intended AI usage are specified.
  - AI ignores Artillery for the first implementation pass.

Exit criteria:

- [x] The unit's numbers and rules can be reviewed without reading implementation code.
- [x] Any unresolved tuning item is marked as deferred and assigned to a later pass.
  - Final collision shape and shot-spread algorithm are implementation choices constrained by the
    rules above.

## Phase 2: Contract And Wire Design

Not started. Read `docs/context/protocol.md` before changing snapshots, commands, events, or
wire-visible unit state.

## Phase 3: Simulation Implementation

Not started. Read `docs/context/server-sim.md` before changing game systems, services, orders,
combat, movement, production, AI, or self-play behavior.

## Phase 4: Client Commands, UI, And Debug Mode

Not started. Read `docs/context/client-ui.md` before changing rendering, HUD, input, or match
modules.

## Phase 5: Visual Design And Animation

Not started.

## Phase 6: Audio Design

Not started. Audio is deferred for the first pass, but firing and impact sounds remain desired
follow-up work.

## Phase 7: Test Matrix

Not started. Read `docs/context/testing.md` before deciding which suites are required.

## Phase 8: Human Review Package

Not started.
