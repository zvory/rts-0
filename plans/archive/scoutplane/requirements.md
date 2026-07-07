# Scout Plane Requirements

Status: Implemented for normal-match review. This document records the approved Phase 0 unit brief
and Phase 1 rules/balance specification for the Scout Plane feature, plus the final Phase 8 audit
notes. It is the factual behavior contract and review checklist, not a standalone implementation
plan.

## Phase 8 Implementation Audit

- Implemented: Kriegsia City Centre production unlocked by completed Gun Works or Vehicle Works;
  50 Steel / 50 Oil cost; 600-tick build time; 0 Supply; one active or in-production Scout Plane
  per player; active-plane select-and-pan behavior; normal production cancellation/interruption.
- Implemented: launch from the City Centre to the first rally point or building position, direct
  aerial movement, 4-tile orbiting, immediate and queued orbit retargeting, selection/control group
  support, manual dismiss, and automatic dismissal after fuel exhaustion.
- Implemented: 1 Oil per 20 ticks upkeep with an 8 Oil reserve, refill on renewed income, 12-tile
  owner/team aerial vision through terrain and building blockers, smoke-blocked aerial sight,
  fog-safe enemy/spectator/replay/lab projection, non-combat targeting filters, no ground
  collision reservation, and checkpoint/replay state.
- Implemented: City Centre command-card button, `Z` grid hotkey, `S` RTS-classic hotkey,
  selected-plane move/dismiss card, mixed-selection command routing, lab spawn/review path, rough
  client-native FW 189-inspired rendering, and minimap aircraft blip.
- Intentionally deferred: dedicated audio, final commissioned aircraft art/icon polish, AI
  production/management, balance tuning beyond the approved numbers, and all non-goal aircraft
  combat or anti-air systems.

## Purpose

Scout Planes give Kriegsia a paid, late-early scouting tool from the City Centre once the player
has committed to Gun Works or Vehicle Works tech. The plane should create temporary strategic
information without becoming a combat unit, blocker, or target. It is expensive enough to matter,
costs ongoing oil attention, and is limited to one active or in-production plane per player.

## Unit Brief

- Name: Scout Plane.
- Visual reference: Focke-Wulf Fw 189-style twin-boom scout aircraft.
- Role: persistent aerial reconnaissance.
- Player-facing description: "Build a scout plane that launches from the City Centre, circles the
  rally area, and reveals fog while consuming oil."
- Strategic purpose: reveal expansions, artillery targets, attack paths, and enemy tech without
  committing a ground scout through blockers.
- Counterplay: deny or smoke the scouted area, pressure oil income, or exploit the player's spent
  resources. The plane itself is not attackable.
- Current availability: normal playable feature through Kriegsia City Centre production after Gun
  Works or Vehicle Works.
- Unusual interactions:
  - Selectable and commandable despite being non-combat and non-colliding.
  - Produced like a normal City Centre unit, but flies directly to an aerial orbit instead of using
    ground pathing.
  - Reveals through terrain and building line-of-sight blockers.
  - Smoke still blocks Scout Plane vision.
  - Enemy players can see the plane only while it is inside their current vision.
  - The active plane persists if the launching City Centre is destroyed.

## Rules And Balance

- Spawn source: selected completed City Centre production queue.
- Unlock requirement: at least one completed Gun Works or completed Vehicle Works owned by the
  player.
- Spawn cost: 50 Steel and 50 Oil.
- Build time: 20 seconds, or 600 ticks at 30 Hz.
- Build hotkeys: grid hotkey `Z`; RTS classic hotkey `S`.
- Active limit: one active or in-production Scout Plane per player.
- If the player already has an active Scout Plane, the City Centre Scout Plane button should select
  the existing plane and pan the camera to it instead of spending resources or queueing another.
- If the player already has a Scout Plane in production, no second Scout Plane should be queued.
- Supply impact: 0 Supply.
- Production interruption: use existing production behavior if the producing City Centre is
  destroyed before completion; do not add Scout Plane-specific cancellation or refund rules.
- Hit points: 40 HP.
- Armor/status: non-targetable by attacks, non-repairable, non-harvestable, non-garrisonable, immune
  to combat targeting and collision effects.
- Sight range: 12 tiles.
- Movement speed: 2 world pixels per tick.
- Movement semantics: flying, ignores terrain pathing, ignores unit/building collision, and does
  not reserve or block occupancy.
- Pathing semantics: direct movement to the orbit target; no ground pathfinding.
- Attack range, damage, cooldown, projectiles, and target filters: none.
- AI availability: deferred. AI should not launch or command Scout Planes in the first
  implementation unless a later phase explicitly adds that behavior.

## Production, Launch, And Movement

- The player selects a City Centre and clicks the Scout Plane command-card button or presses the
  resolved hotkey. The command queues Scout Plane production immediately if prerequisites and
  resources are satisfied; it does not enter world-point targeting mode.
- When production completes, the plane launches automatically from above the producing City
  Centre's world position.
- The plane's lifetime and oil upkeep start when it launches.
- The plane flies from the City Centre toward that City Centre's first rally point.
- If the City Centre has no rally point, the plane's initial orbit center is the City Centre's world
  position.
- Queued rally stages are not required for the first implementation; the first rally point is the
  launch destination.
- Once the plane reaches the rally area, it circles the rally point.
- Orbit radius: 4 tiles.
- The plane remains selectable while active.
- Direct selection, box selection, and control groups should treat the plane like a normal friendly
  unit.
- A move command issued to the selected plane retargets its orbit center.
- Queued move commands append later orbit centers using the existing queued-move command semantics.
- Mixed selections should preserve normal land-unit control: land units receive ordinary land
  commands, while any selected Scout Plane receives only aerial retarget/dismiss commands that apply
  to the plane.

## Oil Upkeep And Dismissal

- Upkeep is one Pump Jack worth of oil income.
- Current Pump Jack rate is 2 Oil every 40 ticks, equivalent to 0.05 Oil per tick or 1.5 Oil per
  second at 30 Hz.
- Upkeep starts immediately when the plane launches.
- Resource payments should be integer Oil deductions while preserving the one-Pump-Jack average
  upkeep rate. A simple acceptable implementation is 1 Oil every 20 ticks.
- The Scout Plane has a 5-second fuel tank. At the target upkeep rate this is 7.5 Oil worth of
  reserve, rounded up to 8 Oil for integer-resource accounting.
- If upkeep is due and the player has enough Oil, spend the Oil and keep the fuel tank full.
- If upkeep is due and the player has zero Oil, the fuel tank drains for the unpaid upkeep time.
- If the player gains Oil again before the fuel tank reaches zero, the plane should spend available
  Oil rapidly, refill the fuel tank, and continue flying.
- If the fuel tank reaches zero, the Scout Plane is dismissed automatically.
- Do not show a special warning before fuel dismissal in the first implementation.
- The player may manually dismiss the Scout Plane at any time with its cancel/dismiss command.
- Dismissal removes the plane and stops its oil upkeep.

## Vision And Projection

- The Scout Plane grants authoritative owner/team fog vision from its current position.
- Scout Plane sight ignores normal terrain and building line-of-sight blockers.
- Scout Plane sight does not ignore smoke. Smoke between the plane and a tile should block vision
  using the same smoke-blocking policy as normal fog where practical.
- The plane itself is projected to enemy players only while it is inside their current vision.
- The plane must not reveal hidden target data, owner resources, queued commands, or other private
  state to enemies.
- The plane is visible to its owner while active.
- Spectator, replay, and lab vision should follow the same projection principles as other
  authoritative world objects.

## UI And Commands

- City Centre command card gains a Scout Plane button when the owning faction can use the feature.
- Button should be visible but disabled until the player owns a completed Gun Works or completed
  Vehicle Works.
- Disabled reason: "Requires Gun Works or Vehicle Works."
- Button cost display: 50 Steel / 50 Oil.
- Button label should follow current production-button conventions; expected label is "Scout
  Plane".
- Command-card placement: City Centre grid slot `Z`.
- Hotkeys: grid profile `Z`; RTS classic profile `S`.
- Button behavior with no active or in-production plane: queue Scout Plane production from the
  selected City Centre.
- Button behavior with an active plane: select the existing plane and pan the camera to it.
- Button behavior with a Scout Plane already in production: do not queue another Scout Plane.
- Plane command card should expose:
  - Move/retarget orbit center.
  - Cancel/Dismiss.
- Plane should not expose Attack, Hold Position, Stop, train, build, harvest, repair, setup, rally,
  or autocast commands unless a later requirement changes this.
- Final icon/art polish is deferred, but the current button uses the specified hotkeys and
  command-card slot.

## Visual And Audio Direction

- The plane should be visually distinguishable from ground vehicles at normal zoom.
- Visual reference is the FW 189 twin-boom shape.
- First implementation may use rough vector/client-native art, but placeholder status must be
  documented if final art is deferred.
- The plane should have a lightweight flying animation or directional motion treatment.
- The orbit should read as aerial circling, not ground driving.
- Dedicated Scout Plane audio is deferred; the first implementation does not add launch, orbit, or
  dismiss sounds.

## Contract Notes

- Scout Plane is a produced City Centre unit, not a targeted ability.
- The active plane is modeled as a selectable world entity with special non-targetable,
  non-colliding aerial movement and upkeep rules. Selection, move commands, fog stamping, enemy
  projection, replay/checkpoint behavior, and dismissal are part of the contract.
- Protocol and client command-card contracts include the one-active-or-in-production limit and the
  City Centre button behavior that selects/pans to an existing active plane.
- Fog/projection docs cover the Scout Plane's "aerial sight through blockers but not smoke"
  category.
- Future protocol changes must update Rust DTOs, JavaScript mirrors, compact codes if used, and
  `docs/design/protocol.md` together.
- Client-visible balance/config values must stay mirrored through the existing Rust rules and client
  config parity surfaces.

## Testing Expectations

Focused implementation verification should cover:

- City Centre Scout Plane button is disabled without Gun Works or Vehicle Works and enabled when
  either completed building exists.
- Queueing production spends 50 Steel and 50 Oil and completes after 600 ticks.
- Completion launches the plane from above the selected City Centre.
- The completed plane flies to the City Centre's first rally point, or orbits above the City Centre
  if no rally point exists.
- Production is rejected without resources and emits the normal resource shortage feedback.
- Only one active or in-production Scout Plane exists per player.
- Pressing the City Centre button with an active Scout Plane selects and pans to the existing plane.
- Move command retargets the orbit center.
- Plane moves at 2 px/tick and circles at 4-tile orbit radius.
- Plane reveals 12 tiles from its current position.
- Plane vision ignores terrain/building blockers but not smoke.
- Enemy sees the plane only when the plane is in enemy current vision.
- Plane has 40 HP but cannot be targeted by attacks, repaired, blocked, or collided with.
- Plane persists after the launching City Centre is destroyed.
- Producing City Centre destruction before completion follows existing production behavior.
- Upkeep starts immediately on launch, drains at one Pump Jack worth of oil, uses integer Oil
  payments, and auto-dismisses only after the 5-second fuel tank is exhausted at zero oil.
- Oil income during fuel drain refills the plane and keeps it flying.
- Manual dismiss removes the plane and stops upkeep.
- Replay/checkpoint/lab/spectator projection remains fog-safe.

## Review Route

- Normal match: build Gun Works or Vehicle Works, train one Scout Plane from a City Centre, confirm
  the second City Centre button selects/pans to the active plane or stays blocked while production
  is queued, retarget the orbit with move commands, queue a later retarget, spend/deny Oil to watch
  upkeep and fuel dismissal, and manually dismiss the plane.
- Lab inspection: open `/lab?room=scout-plane-review&map=Default`, spawn a Kriegsia Scout Plane
  from the Lab panel, then inspect selection, control groups, move/queued retargeting, dismissal,
  minimap blip, fog/projection modes, and teardown across rematch/reset.

## Patch Notes

- Added a City Centre Scout Plane unlocked by either Gun Works or Vehicle Works.
- Scout Plane costs 50 Steel / 50 Oil, takes 20 seconds to build, consumes one Pump Jack worth of
  Oil while active, and is limited to one active or in-production plane per player.
- Scout Plane automatically launches from the City Centre to its rally point, then provides 12-tile
  aerial vision while orbiting, ignoring terrain and building blockers but not smoke.
- Scout Plane can be selected, retargeted, and dismissed, but cannot fight or be attacked.
- Rough client-native aircraft art is in place for readability; dedicated audio and final art polish
  remain deferred.

## Non-Goals

- Do not add aircraft combat, anti-air weapons, crashes, repair, veterancy, transport, or bombing
  behavior in the first Scout Plane implementation.
- Do not make the plane block movement, reserve collision, or interact with ground pathfinding.
- Do not let AI launch or manage Scout Planes unless a later AI-specific requirement adds it.
- Do not treat this requirements document as implementation approval for code, protocol, balance,
  art, or test changes.
