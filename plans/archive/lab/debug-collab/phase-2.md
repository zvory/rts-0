# Phase 2 - Collaborative Lab Client Experience

## Phase Status

- [x] Done.

## Objective

Make the browser match the collaborative lab authority model. Multiple users in the same lab should
all see the operator control surface and be able to use existing lab tools without fighting local
tool state.

## Work

- Verify that `LabControlPolicy`, `LabPanel`, HUD command cards, input routing, and minimap command
  paths all become available when the server sends `role: "operator"` to a later joiner.
- Fix any remaining client assumptions that only the first lab joiner can operate.
- Keep active tool state local to each browser tab. One collaborator arming spawn or move tools
  must not arm/cancel tools in another collaborator's tab.
- Treat lab dirty state, operation count, result/error handling, shared vision, imported scenarios,
  and snapshots as server-owned shared state.
- Make lab status text clear enough for collaboration without designing a presence list. If copy is
  needed, prefer simple "Operator" or "Read-only" role display over a new role taxonomy.
- Add or update client tests for a later-joiner operator, command-card availability, setup-tool
  availability, and read-only behavior if an explicit read-only path still exists.
- Preserve normal spectator, replay viewer, and non-lab match behavior.

## Expected Touch Points

- `client/src/lab_control_policy.js`
- `client/src/lab_panel.js`
- `client/src/lab_client.js`
- `client/src/match.js`
- `client/src/input/`
- `client/src/hud_command_card.js`
- `client/src/minimap.js` if lab command issuing through minimap needs coverage
- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md` only if the capsule's section list shifts

## Verification

- `node tests/client_contracts.mjs`
- `node tests/hud_command_card.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

If protocol changes from Phase 1 land in the client in this phase, also run:

- `node tests/protocol_parity.mjs`

## Manual Test Focus

Open the same lab room in two browser sessions. In the first, spawn a unit and issue a move command;
in the second, spawn another unit and issue a command for the other side. Confirm each browser keeps
its own armed tool state while both see the shared world update.

## Handoff Expectations

Name any client gates that had to change, list exact verification, and call out whether the next
phase can safely change the visible lobby/debug entry without additional client lab work.
