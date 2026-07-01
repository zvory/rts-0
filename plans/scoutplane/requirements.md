# Scout Plane Requirements

Status: Draft product requirements. This document records the approved Phase 0 unit brief and
Phase 1 rules/balance specification for the Scout Plane feature. It is not an implementation plan
and does not authorize Rust, JavaScript, protocol, balance, art, test, or scenario changes by
itself.

## Purpose

Scout Planes give Kriegsia a paid, late-early scouting tool from the City Centre once the player
has committed to Gun Works or Vehicle Works tech. The plane should create temporary strategic
information without becoming a combat unit, blocker, or target. It is expensive enough to matter,
costs ongoing oil attention, and is limited to one active plane per player.

## Unit Brief

- Name: Scout Plane.
- Visual reference: Focke-Wulf Fw 189-style twin-boom scout aircraft.
- Role: persistent aerial reconnaissance.
- Player-facing description: "Launch a scout plane that circles a target area and reveals fog while
  consuming oil."
- Strategic purpose: reveal expansions, artillery targets, attack paths, and enemy tech without
  committing a ground scout through blockers.
- Counterplay: deny or smoke the scouted area, pressure oil income, or exploit the player's spent
  resources. The plane itself is not attackable.
- Initial availability: normal playable feature after user review and explicit implementation
  authorization.
- Unusual interactions:
  - Selectable and commandable despite being non-combat and non-colliding.
  - Reveals through terrain and building line-of-sight blockers.
  - Smoke still blocks Scout Plane vision.
  - Enemy players can see the plane only while it is inside their current vision.
  - The active plane persists if the launching City Centre is destroyed.

## Rules And Balance

- Spawn source: selected completed City Centre.
- Unlock requirement: at least one completed Gun Works or completed Vehicle Works owned by the
  player.
- Spawn cost: 50 Steel and 50 Oil.
- Active limit: one Scout Plane per player.
- If the player already has an active Scout Plane, the City Centre Scout Plane button should select
  the existing plane and pan the camera to it instead of spending resources or spawning another.
- Supply impact: 0 Supply.
- Build time: none; the plane spawns immediately when the command is accepted.
- Hit points: no normal HP. It cannot be damaged or killed by combat.
- Armor/status: non-attackable, non-repairable, non-harvestable, non-garrisonable, immune to all
  combat targeting and collision effects.
- Sight range: 12 tiles.
- Movement speed: 2 world pixels per tick.
- Movement semantics: flying, ignores terrain pathing, ignores unit/building collision, and does
  not reserve or block occupancy.
- Pathing semantics: direct movement to the orbit target; no ground pathfinding.
- Attack range, damage, cooldown, projectiles, and target filters: none.
- AI availability: deferred. AI should not launch or command Scout Planes in the first
  implementation unless a later phase explicitly adds that behavior.

## Launch And Movement

- The player selects a City Centre, clicks the Scout Plane command-card button, then clicks any
  world point.
- Target range is unlimited; the player may target any point on the map.
- The plane spawns at the selected City Centre's world position.
- The plane's lifetime starts when it spawns.
- The plane flies from the selected City Centre toward the clicked point.
- Once it reaches the target area, it circles the clicked point.
- Orbit radius: 4 tiles.
- The plane remains selectable while active.
- A move command issued to the selected plane retargets its orbit center.
- Recommended first implementation selection behavior: direct-click selectable and control-groupable,
  but excluded from ordinary drag-box ground-army selection so it does not pollute normal army
  control. This is an implementation simplicity decision and may be revised if the existing
  selection system makes another approach cleaner.

## Oil Upkeep And Dismissal

- Upkeep is one Pump Jack worth of oil income.
- Current Pump Jack rate is 2 Oil every 40 ticks, equivalent to 0.05 Oil per tick or 1.5 Oil per
  second at 30 Hz.
- First-pass upkeep should be implemented as a discrete 2 Oil charge every 40 ticks, or an
  equivalent accumulator that produces the same rate.
- The Scout Plane has a 10-second oil-starvation grace buffer.
- If upkeep is due and the player cannot pay, the plane consumes grace time instead of disappearing
  immediately.
- If the player successfully pays upkeep again, the grace buffer refills to 10 seconds.
- If the player remains unable to pay until the grace buffer is exhausted, the Scout Plane is
  dismissed automatically.
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
- Button behavior with no active plane: enter world-point targeting mode.
- Button behavior with an active plane: select the existing plane and pan the camera to it.
- Plane command card should expose:
  - Move/retarget orbit center.
  - Cancel/Dismiss.
- Plane should not expose Attack, Hold Position, Stop, train, build, harvest, repair, setup, rally,
  or autocast commands unless a later requirement changes this.
- Exact hotkey, icon text, tooltip copy, and command-card slot are deferred to implementation, but
  the button should not conflict with existing high-use City Centre controls.

## Visual And Audio Direction

- The plane should be visually distinguishable from ground vehicles at normal zoom.
- Visual reference is the FW 189 twin-boom shape.
- First implementation may use rough vector/client-native art, but placeholder status must be
  documented if final art is deferred.
- The plane should have a lightweight flying animation or directional motion treatment.
- The orbit should read as aerial circling, not ground driving.
- A playtest-note TODO exists to download an FW 189 Scout Plane sound effect.
- Audio is deferred unless an implementation phase explicitly includes it.

## Contract Notes For Later Phases

- This feature likely needs a new ability id, command-card affordance, and protocol mirror entry if
  it uses the existing generic ability command path.
- Existing ability handling is currently unit-carrier oriented. A later design phase must decide
  whether Scout Plane is a building-cast ability, a new explicit `callScoutPlane` command, or a
  generalized selectable aerial entity command path.
- The active plane should probably be modeled as a transient world entity or ability object rather
  than a normal trainable unit. The final implementation choice must preserve selection, move
  commands, fog stamping, enemy projection, replay/checkpoint behavior, and dismissal.
- Fog changes must update the authoritative fog/projection design if Scout Plane vision introduces
  a new "aerial sight through blockers but not smoke" category.
- Protocol changes must update Rust DTOs, JavaScript mirrors, compact codes if used, and
  `docs/design/protocol.md` together.
- Client-visible balance/config values must be mirrored through the existing Rust rules and client
  config parity surfaces.

## Testing Expectations

Focused implementation verification should cover:

- City Centre Scout Plane button is disabled without Gun Works or Vehicle Works and enabled when
  either completed building exists.
- Launch spends 50 Steel and 50 Oil and spawns at the selected City Centre.
- Launch is rejected without resources and emits the normal resource shortage feedback.
- Only one active Scout Plane exists per player.
- Pressing the City Centre button with an active Scout Plane selects and pans to the existing plane.
- Move command retargets the orbit center.
- Plane moves at 2 px/tick and circles at 4-tile orbit radius.
- Plane reveals 12 tiles from its current position.
- Plane vision ignores terrain/building blockers but not smoke.
- Enemy sees the plane only when the plane is in enemy current vision.
- Plane cannot be attacked, damaged, repaired, blocked, or collided with.
- Plane persists after the launching City Centre is destroyed.
- Upkeep drains at one Pump Jack worth of oil and auto-dismisses only after the 10-second grace
  buffer is exhausted at zero oil.
- Manual dismiss removes the plane and stops upkeep.
- Replay/checkpoint/lab/spectator projection remains fog-safe.

## Patch Notes Draft

- Added a City Centre Scout Plane call-in unlocked by either Gun Works or Vehicle Works.
- Scout Plane costs 50 Steel / 50 Oil, consumes one Pump Jack worth of Oil while active, and is
  limited to one per player.
- Scout Plane provides 12-tile aerial vision while circling a target point, ignoring terrain and
  building blockers but not smoke.
- Scout Plane can be selected, retargeted, and dismissed, but cannot fight or be attacked.

## Non-Goals

- Do not add aircraft combat, anti-air weapons, plane HP, crashes, repair, veterancy, transport, or
  bombing behavior in the first Scout Plane implementation.
- Do not make the plane block movement, reserve collision, or interact with ground pathfinding.
- Do not let AI launch or manage Scout Planes unless a later AI-specific requirement adds it.
- Do not treat this requirements document as implementation approval for code, protocol, balance,
  art, or test changes.
