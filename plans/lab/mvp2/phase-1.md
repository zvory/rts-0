# Phase 1 - Lab Operator Command Surface

## Phase Status

- [x] Done.

## Objective

Let a lab operator select real player-owned entities, see the real command card, and issue normal
orders through lab issue-as while keeping read-only viewers and normal spectator surfaces passive.

## Work

- Audit every client gate that hides command UI or suppresses command input because
  `state.spectator` is true.
- Introduce a narrow command-surface permission concept for `Match` and HUD context. The permission
  should be derived from existing room role, lab metadata, and `LabControlPolicy`, not from URL
  mode or local assumptions.
- Keep prediction disabled for lab operators unless a later plan explicitly designs lab prediction.
- Update command-card descriptor construction so spectator-shaped projection can coexist with
  operator command permission.
- Keep ownership checks policy-driven. A lab operator may inspect broad selections, but gameplay
  commands should only be available for a single controllable owner.
- Preserve read-only lab viewer behavior, replay viewer behavior, and normal spectator behavior.
- Add focused coverage proving lab operator command cards are visible, read-only cards stay hidden,
  mixed-owner selections stay non-commandable, and issue-as wrapping still happens at the existing
  command issuer boundary.

## Expected Touch Points

- `client/src/match.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/lab_control_policy.js`
- `client/src/input/commands.js`
- `client/src/input/index.js`
- `client/src/room_capabilities.js` if command capability parsing needs a small explicit field
- `client/src/protocol.js` only if a capability or start-payload shape changes
- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md` if the command-surface contract becomes stable public client design
- `docs/design/protocol.md` only if protocol or capability shape changes

## Implementation Checklist

- [x] Identify and document the current spectator gates that affect lab command UI.
- [x] Add the smallest command-surface helper or option that lets lab operator command permission
      be represented explicitly.
- [x] Route HUD command-card context through the new permission instead of raw spectator state.
- [x] Confirm `LabControlPolicy.canControlOwner` remains the owner authority for lab commandability.
- [x] Confirm command issuing still calls `issueCommandAs` for lab operators.
- [x] Keep read-only lab viewers, replay viewers, and normal spectators unable to command.
- [x] Add or update client tests for lab command-card visibility and issue-as behavior.
- [x] Run focused verification and record exact results in the handoff.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/hud_command_card.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

If protocol or room capability shapes change, also run:

- `node tests/protocol_parity.mjs`

## Manual Test Focus

Open a lab as operator, select a player-owned rifleman or worker, confirm the command card appears,
and issue a move or stop order. Open a read-only lab viewer or normal spectator path and confirm the
command card and command input stay unavailable.

## Handoff Expectations

Name the new command-surface permission and the old spectator gates it replaces or bypasses. State
whether any protocol/capability shape changed, and call out remaining command-card gaps that should
wait for later phases.
