# Phase 9 - Lobby and Score UI Exposure

Status: done.

## Goal

Expose team games through normal user-facing UI only after authoritative team safety and client
command safety exist. This phase is presentation and exposure, not another simulation behavior
change.

## Scope

- Add compact lobby UI for presets, grouped team rows, and host-only AI/team controls.
- Keep `ffa` as the default visible preset.
- Make host-only controls clear and ensure non-host controls are disabled or absent.
- Show enough seat/team state that solo, FFA, 1v2, 1v3, and 2v2 setup is understandable in one tab.
- Add Team column to score UI.
- Highlight all rows whose `teamId` matches `winnerTeamId`.
- Keep `winnerId` support for singleton FFA compatibility.
- Keep entity body color per owner, not per team, unless a later art pass changes this deliberately.
- Remove or retire any temporary test/dev UI gate introduced earlier.

## Expected Touch Points

- `docs/design/client-ui.md`
- `client/src/lobby.js`
- `client/src/net.js`
- `client/src/app.js`
- `client/src/match.js`
- `client/index.html`
- `client/styles.css`
- `tests/client_contracts.mjs`
- `tests/client_smoke.mjs`
- `tests/team_integration.mjs`

## Verification

```bash
node tests/client_contracts.mjs
node tests/team_integration.mjs
node tests/client_smoke.mjs
node scripts/check-client-architecture.mjs
```

Required automated scenarios:

- Lobby renders the default FFA preset and singleton team rows.
- Host can configure every supported preset from UI-backed command paths.
- Non-host cannot mutate preset, team assignment, or AI team seating.
- Score table renders Team column and highlights all winning-team rows.
- Solo, FFA, 1v2, 1v3, and 2v2 remain scriptable through the shared integration helpers.

## Acceptance Criteria

- Team setup is possible from both UI and WebSocket tests.
- Normal lobby exposure is no longer hidden behind a temporary test/dev gate.
- Team score display is clear and test-covered.
- UI exposure does not introduce new simulation or protocol behavior beyond already-tested fields.

## Manual Testing Focus

Use one browser tab to confirm the host can see preset controls, grouped team rows, and score-screen
team display. Do not require manual multi-tab validation.

## Handoff Requirements

The phase handoff must describe UI behavior, list any deliberately deferred polish, and confirm the
temporary gating state.

## Implementation Notes

- Lobby team setup is now exposed in the normal lobby: hosts see a preset selector for `ffa`,
  `solo`, `1v2`, `1v3`, and `2v2`; player rows are grouped by team; supported team presets show
  per-seat team selectors and per-team AI add buttons. Non-hosts see preset/team labels without
  mutation controls.
- `ffa` remains the default visible preset and renders each active player as a singleton team row.
- The score screen now includes a Team column and highlights every score row whose `teamId` matches
  `winnerTeamId`; singleton FFA `winnerId` highlighting remains as a compatibility fallback.
- Temporary normal-lobby gating for non-FFA team presets is retired. Team games are reachable from
  the ordinary lobby UI.
- Deferred polish: entity body colors remain owner-colored, and the lobby keeps compact native
  selects/buttons rather than adding bespoke formation previews or multi-tab setup flows.
