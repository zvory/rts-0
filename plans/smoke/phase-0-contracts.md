# Phase 0: Contracts and Test Matrix

## Objective

Freeze the rules and contract changes before touching implementation code. Smoke crosses protocol,
balance, command execution, fog, combat, projection, client targeting, and rendering, so ambiguous
behavior needs to be removed up front.

## Contract Decisions

- Use one ability vocabulary for both existing Rifleman Charge and new Smoke. Charge is already an
  ability with a cooldown, but it is self-activated and has no target point. Smoke is a targeted
  world-point ability. Do not build a smoke-only command, cooldown field, HUD branch, or order path.
- Add a generic `useAbility` command shape for targeted abilities:

```text
{ c: "useAbility", ability: "smoke", units: u32[], x: f32, y: f32, queued?: bool }
```

- Keep the existing `charge` command working during the migration. Future ability plumbing may add
  a self-target/no-target command form for Charge, but it must not break existing replays or clients
  until an explicit protocol migration removes `charge`.
- Ability definitions must include target mode:
  - `self`: applies immediately to each eligible owned carrier; used by Rifleman Charge.
  - `worldPoint`: carries `x`/`y`, may need movement-to-range and queue state; used by Smoke.
- Add owner-only ability cooldown projection as a generic field over one-off fields such as
  `smokeCooldownLeft`:

```text
abilities?: [{ ability: "smoke", cooldownLeft: u16 }]
```

- During migration, Rifleman Charge cooldown may be projected in both the legacy
  `chargeCooldownLeft` field and the generic `abilities` list, or only in the generic list after
  every decoder/HUD path is updated in the same change. The target end state is that Charge appears
  as `{ ability: "charge", cooldownLeft }` and Smoke appears as
  `{ ability: "smoke", cooldownLeft }` in the same owner-only list.
- Add active smoke projection as a top-level snapshot field instead of fake neutral entities:

```text
smokes?: [{ id: u32, x: f32, y: f32, radiusTiles: f32, expiresIn: u16 }]
```

- Compact snapshot representation:
  - bump compact snapshot version when `abilities` and `smokes` are implemented;
  - encode entity abilities as an owner-only optional compact slot after existing owner-only fields,
    preserving old compact slots until the version bump;
  - encode top-level smoke clouds as a separate compact array, omitted when empty.
- Smoke cloud visibility is based on the player's current authoritative visibility after smoke
  suppression has been applied. The smoke object is visible if any tile in the cloud is currently
  visible to that recipient. Owners/friendlies get no special smoke-object ownership.
- Out-of-range Smoke launch staging uses the point on the segment from target to caster at
  `range - launch_tolerance`, clamped to map bounds and pathable by the carrier.

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
- Existing Rifleman Charge still activates only eligible owned riflemen, respects Training Centre
  tech, and blocks repeat activation through the unified ability cooldown model.

Client/unit tests where practical:

- Ability command-card data can represent both self-target abilities like Charge and world-point
  abilities like Smoke without one-off command-card branches.
- Rifleman command card still shows Charge from the unified ability affordance model and sends the
  existing command until the explicit protocol migration changes it.
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

- All rule decisions above are accepted.
- The implementation test list is copied into the relevant phase issues or commit notes.
- No gameplay code has changed in this phase.
