# Artillery Unit Checklist

Working checklist for `plans/archive/tech/phase-6-capstones.md`, following
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
  - Artillery uses tank-style oriented pivot movement, uses the same footprint/selection size as a
    tank, and has a new Hard armor class that reduces non-armor-piercing damage by 25%.
  - Artillery automatically sets up toward point-fire targets, takes 3 seconds to set up or tear
    down, and cannot rotate while deployed.
  - Artillery shells take 5 seconds to land after firing.
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
  - Visual/readability target uses the same footprint and selection size as a tank, with the long
    barrel and deployed spades carrying the artillery silhouette.
  - Collision implementation uses a tank-sized oriented body so the unit remains usable in
    snaking/depth pathing scenarios.
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
  - Shell travel: 5 seconds after firing.
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

Read `docs/context/protocol.md`, then updated the Rust/JS protocol mirrors and
`docs/design/protocol.md`.

- [x] Confirm whether the existing unit kind encoding is enough or a new tag is needed.
  - Added new unit kind string/code: `artillery` / compact kind code `16`.
- [x] Confirm whether snapshots need new fields for movement, attack, setup, ability, projectile,
      or status state.
  - No new entity snapshot fields are required for the first pass.
  - Artillery reuses existing `facing`, `weaponFacing`, `setupState`, `setupFacing`, `orderPlan`,
    and owner-only `abilities` fields.
- [x] Confirm whether client commands need new order types or fields.
  - No new top-level command discriminator is needed.
  - Point Fire uses existing `useAbility` with new ability id `pointFire`, `units`, `x`, `y`, and
    optional `queued`.
  - Added `cmd.pointFire(...)` as a JS builder for the same wire shape.
- [x] Define how queued movement, queued attacks, queued abilities, cancellation, and interrupted
      orders appear on the wire.
  - Added owner-only `pointFire` order-plan marker / compact stage code `10`.
  - Point Fire remains terminal in the order queue by rule; runtime implementation must reject later
    queued stages after an accepted Point Fire.
  - Stop, move, teardown, redirect, and invalid/unaffordable fire feedback continue to use existing
    command and notice channels.
- [x] Define whether new transient events are required.
  - Added owner-only `artilleryTarget` event / compact event code `7`:
    `{x, y, radiusTiles, delayTicks}`.
  - Added visual-only `artilleryImpact` event / compact event code `8`:
    `{x, y, radiusTiles}`.
- [x] Verify every event is player-facing and intentionally consumed; do not leak debug-only events
      such as raw fire markers into lobby or match messages.
  - `artilleryTarget` is player-facing pre-impact feedback for the firing player only.
  - `artilleryImpact` is player-facing explosion feedback for all recipients after impact.
- [x] Verify fog gating for entity views, target ids, projectile/tracer state, death events, impact
      positions, and ability effects.
  - Contract requires enemies never receive `artilleryTarget`, even with vision of the gun.
  - Contract requires `artilleryImpact` to be sent to all active recipients after impact without
    revealing terrain, updating exploration, or carrying entity visibility.
- [x] Verify inbound command validation: dedupe, cap, range-check, overflow-check, and reject invalid
      target or coordinate data.
  - Contract uses existing `useAbility` fields so unit-list dedupe/cap and coordinate finite/range
    validation must be implemented in the simulation command pass.
  - Point Fire target remains a world position, not an entity id.
- [x] Add or update protocol tests for new tags, fields, commands, events, and fog-filtered payloads.
  - Updated Rust compact snapshot tests.
  - Updated JS protocol contract tests and parity coverage.

Exit criteria:

- [x] The server and client agree on every tag, field name, and shape.
- [x] The wire format is tested before client visuals are considered complete.

## Phase 3: Simulation Implementation

Read `docs/context/server-sim.md` before changing game systems, services, orders, combat,
movement, production, AI, or self-play behavior.

- [x] Add authoritative Artillery unit kind, stable rule ids, build definition, and Gun Works
      training gate.
  - `EntityKind::Artillery` uses compact kind code `16`.
  - Artillery trains from Gun Works for 300 steel / 100 oil, 5 supply, 750 ticks, with 150 HP,
    4-tile sight, 0.922 px/tick speed, tank-style pivot movement, and a large support-gun body.
- [x] Add Gun Works `ArtilleryUnlock` research and production prerequisite.
  - `UpgradeKind::ArtilleryUnlock` costs 200 steel / 200 oil and takes 900 ticks.
- [x] Add the new `Hard` armor behavior.
  - Non-armor-piercing damage is reduced by 25%; armor-piercing damage applies in full.
- [x] Add authoritative Point Fire command translation and validation.
  - Point Fire uses `useAbility` / `pointFire` with a world-position target.
  - Inbound unit lists are deduped/capped through the existing command path.
  - Non-finite targets are rejected and finite targets are clamped before range checks.
  - Invalid min-range, max-range, wrong-unit, under-construction, moving, or deployed-arc targets
    are rejected without moving the unit or spending steel.
- [x] Add Point Fire queue semantics.
  - Point Fire projects as a `pointFire` order-plan stage.
  - Point Fire is terminal: later queued orders are not accepted after an accepted queued Point
    Fire.
- [x] Add Artillery setup, teardown, and deployed movement restrictions.
  - Point Fire starts setup facing the target point when packed.
  - Deployed, setting-up, and tearing-down Artillery cannot move.
  - Movement and teardown reset Artillery accuracy.
- [x] Add range, arc, reload, ammunition, shell delay, scatter, and impact damage behavior.
  - Minimum range is 10 tiles; maximum range is 50 tiles.
  - Deployed arc is 20 degrees total.
  - Setup, teardown, and reload are 3 seconds each.
  - Steel is paid only when a shot actually fires; unaffordable shots hold the order and retry
    after the reload delay.
  - Shells land 5 seconds after firing.
  - Scatter improves from 5 tiles toward 2 tiles across maintained fire after setup.
  - Impacts deal 150 armor-piercing damage inside 1 tile and non-armor-piercing splash falloff to
    10 damage at 3 tiles.
  - Splash damages friendly and enemy units/buildings; neutral resource nodes are excluded.
- [x] Add fog-safe Artillery event emission.
  - `artilleryTarget` is emitted only to the firing owner.
  - `artilleryImpact` is emitted to all players after impact and does not update fog or exploration.
- [x] Keep AI ignoring Artillery for the first pass.
  - AI combat classification treats Artillery as unavailable/non-combat for now.
- [x] Add focused simulation coverage.
  - Covered terminal queued Point Fire promotion.
  - Covered owner-only target events, global impact events, and steel spent at fire time.
- [x] Update architecture baseline for the intentional simulation growth.
  - Blessed the new order/shell executor surface with reason:
    `artillery point-fire simulation adds order and shell executor`.

Exit criteria:

- [x] Authoritative rules exist for production, unlock, armor, movement, setup, Point Fire, shell
      scheduling, impact damage, and event visibility.
- [x] The new simulation paths are panic-conscious on command/tick inputs and reuse checked command
      validation where client data enters the sim.
- [x] Focused Rust simulation/rules/protocol/contract coverage passes.

## Phase 4: Client Commands, UI, And Debug Mode

Read `docs/context/client-ui.md` before changing rendering, HUD, input, or match modules.

- [x] Wire Artillery into client HUD training/research affordances.
  - Gun Works command card mirrors the server train order: Mortar Team (`Q`), AT Gun (`W`),
    Artillery (`E`), with AT Gun Crews (`S`) and Unlock Artillery (`D`) below their unlocked
    units.
- [x] Add Point Fire command UI for selected Artillery via `useAbility` / `pointFire`.
  - Selected Artillery exposes Point Fire on `X`; issuing it uses `cmd.pointFire(...)`.
  - The targeted preview shows maximum range, minimum dead zone, splash radius, and firing corridor.
- [x] Render Point Fire command feedback and owner-only order-plan markers.
  - Local issued commands and server-accepted `pointFire` stages draw artillery crosshair markers.
- [x] Consume and render `artilleryTarget` and `artilleryImpact` events without changing fog or
      exploration.
  - `artilleryTarget` creates an owner-only pre-impact marker with shell-delay lifetime.
  - `artilleryImpact` creates a short-lived explosion at the impact point.
- [x] Add client/debug-mode affordances needed to test Artillery.
  - Debug human starts now include five Artillery units alongside the other combat units.
- [x] Add focused client contract coverage.
  - Covered Gun Works train/research slots, Artillery/Point Fire config, artillery event local
    state, and Point Fire input/preview behavior.

## Phase 5: Visual Design And Animation

- [x] Stationary appearance is specified.
  - Tank-sized gameplay footprint with an exposed field-gun carriage, large wheels, split trails,
    heavy breech, long barrel, and muted team-color cradle panels.
- [x] Moving appearance is specified if different.
  - Packed Artillery keeps its trails tucked in and shows light rear dust while moving.
- [x] Attacking appearance is specified.
  - Point Fire uses recoil, a short muzzle flash, owner-only target crosshair, and a descending
    shell cue during the 5-second travel delay.
- [x] Setup, deployed, channeling, ability, construction, damaged, and death states are specified if
      relevant.
  - Setup/teardown reuses the AT Gun-style support-weapon animation timing; deployed Artillery
    spreads rear spades/feet and keeps the barrel visible on its setup facing.
  - Construction, damage, and death continue to use existing generic unit visuals for this pass.
- [x] Facing, turret, barrel, projectile, tracer, recoil, muzzle flash, impact, and area indicators
      are specified if relevant.
  - Facing follows tank-style hull movement; weapon facing follows setup/point-fire facing.
  - Artillery targets show outer splash and inner danger rings; impacts use a larger jagged blast,
    shock ring, and shrapnel strokes.
- [x] Selection, hover, health, range, ability radius, and targeting affordances are specified.
  - Selection/health use the tank-sized unit radius.
  - Selected deployed Artillery shows its 50-tile, 20-degree firing wedge; Point Fire targeting
    shows max range, 10-tile dead zone, splash radius, and firing corridor.
- [x] Fog, occlusion, minimap, and team-color readability are checked.
  - Impact events remain visual-only and do not reveal terrain or update exploration. Enemies only
    see the impact, not the pre-impact target marker.
  - Minimap behavior stays generic for units; no special Artillery blip is added in this pass.
- [x] Placeholder art is clearly labeled if final art is deferred.
  - Final authored sprites, bespoke construction/damaged/death states, and stronger shell travel
    polish are deferred to a later art pass.

Exit criteria:

- [x] The unit is visually distinguishable from existing units at normal zoom.
- [x] Missing visual polish is documented as follow-up instead of silently skipped.

## Phase 6: Audio Design

Not started. Audio is deferred for the first pass, but firing and impact sounds remain desired
follow-up work.

## Phase 7: Test Matrix

Not started. Read `docs/context/testing.md` before deciding which suites are required.

## Phase 8: Human Review Package

Not started.
