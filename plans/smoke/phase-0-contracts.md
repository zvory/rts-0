# Phase 0: Contracts and Test Matrix

## Objective

Freeze the rules and contract changes before touching implementation code. Smoke crosses protocol,
balance, command execution, fog, combat, projection, client targeting, and rendering, so ambiguous
behavior needs to be removed up front.

## Contract Decisions to Record

- Add `smoke` or a generic `useAbility` command shape. Prefer generic:

```text
{ c: "useAbility", ability: "smoke", units: u32[], x: f32, y: f32, queued?: bool }
```

- Add owner-only ability cooldown projection. Prefer a generic field over
  `smokeCooldownLeft`, for example:

```text
abilities?: [{ ability: "smoke", cooldownLeft: u16 }]
```

- Add active smoke projection. Prefer a top-level snapshot field instead of fake neutral entities:

```text
smokes?: [{ id: u32, x: f32, y: f32, radiusTiles: f32, expiresIn: u16 }]
```

- Decide compact snapshot representation and bump compact snapshot version when implementing.
- Define whether smoke cloud visibility is based on the player's current fog after smoke suppression
  or before smoke suppression. Recommended: visible if any tile in the cloud is currently visible
  after authoritative smoke rules, plus owners/friendlies do not get special smoke-object ownership.
- Define exact launch-staging point for out-of-range casters. Recommended: point on the segment from
  target to caster at `range - launch_tolerance`, clamped to map bounds and pathable by the
  carrier.

## Test Matrix

Server simulation tests:

- In-range smoke picks the furthest selected ready scout car and launches immediately.
- Out-of-range smoke moves the closest selected ready scout car to a launch point.
- Launch after movement pays resources at launch time.
- Launch after movement fails with `Not enough oil` or `Not enough steel` if resources were spent.
- Cooldown starts on launch and blocks repeat launches until expiry.
- Completed Steelworks is required.
- Smoke supports Shift queue and promotes deterministically.
- Overlapping smoke clouds block LOS independently and expire independently.
- Smoke blocks AT-gun LOS to a tank across the cloud.
- Enemy units inside smoke are not visible and cannot be targeted.
- Friendly units inside smoke remain visible to their owner/allies but provide no vision.
- Units inside smoke do not acquire or retain targets.
- Non-finite target coordinates and huge unit lists are bounded and panic-free.

Client/unit tests where practical:

- Scout-car command card shows Smoke on `D` only when Steelworks is complete.
- Button disables for insufficient steel, insufficient oil, and cooldown.
- Targeting mode draws dotted 9-tile range circles around selected eligible carriers.
- Left-click sends `useAbility(smoke, selectedIds, x, y, queued)` and exits targeting.
- Shift-left-click sends the queued flag.
- Right-click/Esc cancels targeting.

Integration/manual checks:

- Smoke lets tanks close on deployed AT guns that previously had a clear shot.
- Fog overlay and server-projected entities agree enough that hidden enemies do not appear
  clickable.
- Spectator/dev full-world views show smoke without granting normal players extra intel.

## Documentation Updates

Implementation changes should update:

- `docs/design/protocol.md` for command, snapshot, compact snapshot, and events if added.
- `docs/design/balance.md` for Smoke cost, range, radius, duration, cooldown, and tech.
- `docs/design/server-sim.md` for reusable ability order/execution and dynamic LOS blockers.
- `docs/design/client-ui.md` for targeted ability UX and rendering responsibilities.

## Done

- All rule decisions above are either accepted or replaced with explicit alternatives.
- The implementation test list is copied into the relevant phase issues or commit notes.
- No gameplay code has changed in this phase.
