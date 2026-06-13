# Phase 4 - Production and Units Tabs

Status: Done.

## Objective

Render server-backed production and unit inventory tabs in the replay analysis overlay. These tabs
should help observers understand macro state without selecting every production building or
manually counting units.

## Scope

- Consume the Phase 3 replay analysis payload in client state or a replay analysis model owned by
  the overlay.
- Production tab:
  - group rows by player
  - show currently active production or research item
  - show progress and queue depth
  - use player colors and known labels/icons from mirrored config
  - handle empty production cleanly
- Units tab:
  - group current unit counts and steel/oil value by player and kind
  - include totals
  - keep ordering stable and scannable
- Update tabs correctly after replay seek, replay speed changes, pause, and vision changes.
- Keep the army value viewport overlay from Phase 2 available while other tabs are selected if the
  chosen UI design supports separate viewport-specific overlays.
- Avoid command affordances; replay analysis is read-only.

## Expected Touch Points

- replay analysis overlay module
- client state/model code for storing the latest replay analysis payload
- `client/src/protocol.js` consumption of Phase 3 payload
- `client/src/config.js` read-only labels/icons/costs
- `client/styles.css`
- client architecture allowlist only if unavoidable
- `docs/design/client-ui.md` if the module contract needs documentation

## Verification

- Unit or DOM tests for rendering:
  - empty production
  - active unit production
  - active research
  - multiple players
  - unit totals and per-kind rows
  - seek replacement of stale payloads
- Run:

```bash
node scripts/check-client-architecture.mjs
```

- Run JS protocol tests touched by Phase 3 payload handling.

## Manual Testing Focus

Open a replay where both players are producing, pause, seek to different ticks, and confirm
production rows update to the new authoritative state. Compare the Units tab against visible map
state in all-vision replay mode and confirm totals change after fights and production completion.

## Handoff Expectations

The handoff must identify any tabs still placeholder-only, the exact client model that stores
analysis payloads, and any unresolved layout constraints. The next agent should add losses tabs
without changing the Phase 3 protocol unless a documented gap is found.

## Player-Facing Outcome

Replay viewers can read current production and army composition from a dedicated analysis panel.
