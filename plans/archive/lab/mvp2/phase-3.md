# Phase 3 - Faction Spawn Palette

## Phase Status

- [x] Done.

## Objective

Replace the main unit spawn form with a faction-filtered palette that arms a spawn tool and sends
the clicked world position to the existing typed lab spawn operation.

## Work

- Build a compact lab spawn palette with owner selection, faction selection, unit-kind options, and
  the existing completion choice where it still applies.
- Use the client faction catalog mirror to populate units. If importing playable faction metadata
  from the lobby creates the wrong dependency direction, extract a small shared faction metadata
  helper instead of duplicating lists.
- When the operator chooses a palette item, arm the spawn lab tool introduced in Phase 2.
- On world click, send `spawnEntity` through `LabClient` with the selected owner, kind, completion
  flag, and exact clicked world coordinates.
- Surface accepted and rejected spawn results through the existing lab result/status path.
- Remove the primary reliance on manual `X` and `Y` spawn fields. Do not keep a secondary advanced
  spawn fallback; unit spawning is the required MVP2 path.

## Expected Touch Points

- `client/src/lab_panel.js`
- `client/src/client_intent.js` if the spawn payload needs additional intent fields
- `client/src/match.js`
- `client/src/input/index.js`
- `client/src/config.js` if a shared catalog helper is needed
- `client/src/lobby_view.js` or a new small shared faction metadata module if playable faction
  labels need extraction
- `client/styles.css`
- `tests/client_contracts.mjs`
- `scripts/check-client-architecture.mjs`
- `scripts/check-faction-catalog-parity.mjs` if catalog exports or mirrors change
- `docs/design/client-ui.md`

## Implementation Checklist

- [x] Define the palette data source from existing faction catalogs and labels.
- [x] Add owner and faction controls that update the available unit palette deterministically.
- [x] Add palette item selection that arms the spawn lab tool.
- [x] Send spawn requests with clicked world coordinates, not camera-center or map-center defaults.
- [x] Surface server validation errors without leaving the UI in a misleading active state.
- [x] Remove or demote manual `X`/`Y` unit spawn fields from the primary workflow.
- [x] Add tests for faction filtering, tool arming, exact coordinate forwarding, and result display.
- [x] Run focused verification and record exact results in the handoff.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

If faction catalog exports or parity-sensitive config changes:

- `node scripts/check-faction-catalog-parity.mjs`

## Manual Test Focus

Open a lab, choose Kriegsia and Ekat in the faction dropdown, confirm the palette changes, pick a
unit, and click several visible world points to confirm units spawn under the cursor. Try an invalid
or occupied placement and confirm the server error is visible and the tool state remains
understandable.

## Handoff Expectations

Describe the palette data source and any catalog helper extracted. Call out any known palette-label
or preview limitations for Phase 5 polish.
