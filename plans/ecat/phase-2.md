# Phase 2 - Ability Object Projection Protocol

Status: Done.

## Goal

Expose active ability world objects through a fog-filtered snapshot projection path. This phase
adds protocol and state transport only; it does not need polished client rendering.

## Scope

- Define a protocol DTO for active ability world objects, for example `AbilityObjectView`, with
  fields for:
  - id
  - owner
  - ability id
  - object kind
  - x/y world position
  - expires-in ticks when applicable
  - optional source caster id only when safe for the recipient
  - optional owner-only state fields needed by Phase 4 or later
- Add the object list to `Snapshot` and compact snapshot transport.
- Mirror the compact and object-shaped decoding in `client/src/protocol.js`.
- Add fog-filtered projection from the ability runtime store in `Game::snapshot_for`,
  `snapshot_for_spectator`, and `snapshot_full_for`.
- Establish visibility policy:
  - own objects are visible to the owner if their position is known to the owner
  - enemy objects are visible only when their position is in current team-visible fog
  - spectator/full-world views follow their existing fog/full-world mode
  - owner-only details never leak to enemies
- Update `docs/design/protocol.md` and the relevant server-sim/client-ui design sections.
- Add protocol parity and snapshot projection tests.

## Expected Deliverables

- Snapshots can carry active ability objects.
- Enemies do not receive hidden object positions or owner-only details.
- The client decodes the new field without rendering errors.
- Existing smoke, mortar, artillery, and entity projection behavior remains unchanged.

## Out of Scope

- Creating real dash, projectile, or anchor objects from gameplay commands.
- Client art or HUD polish.
- Targeting or clicking projected ability objects.
- Changing normal entity visibility rules.

## Verification

- Run focused Rust snapshot/fog tests for own, enemy, spectator, and full-world projection.
- Run protocol parity or compact snapshot tests that cover the new field.
- Run a narrow client protocol decode test if one exists for compact snapshots.

## Manual Testing Focus

Start a normal local match and confirm snapshots still stream and the client does not error in the
console. If a debug fixture object is added for testing, confirm it appears only when the viewer can
see its position.

## Handoff Expectations

The handoff must name the protocol field and compact slot, summarize the fog policy, list tests
covering hidden enemy objects, and identify what client state/rendering Phase 3 should consume.
