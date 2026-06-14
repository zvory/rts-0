# Phase 4 - Selection Budget Grid UI

Status: Done.

## Goal

Replace the multi-selected summary with a two-row command-budget grid that makes command supply
usage visible. The grid should show large units occupying multiple cells, the current `used / cap`,
Command Car cap expansion, and a brief red overflow message when a selection attempt exceeds the
budget.

## Scope

- Update `client/src/hud.js` selected-panel rendering for multi-selection.
- Add or update CSS for:
  - two-row command-budget grid
  - responsive cells whose width narrows as Command Car-expanded caps add columns
  - selected entity blocks spanning one or more cells
  - larger text for high-supply blocks
  - red overflow flash text/counter state
- Render selected entities with existing acronyms from `STATS[kind].icon`.
- Render block sizes by command weight:
  - 1 supply: one cell
  - 2 supply: vertical or horizontal two-cell block, whichever fits deterministically
  - 3 supply: three-cell horizontal block unless the layout helper proves a better deterministic fit
  - 4 supply: `2x2`
  - 5 supply: deterministic near-rectangle with one visually reserved cell or a documented fallback
  - 6 supply: `3x2`, so Tanks fill a two-tall, three-wide block
- Keep packing deterministic and simple. Do not build an expensive optimal bin-packer unless the
  simple layout visibly fails for expected selections.
- Show `used / cap` where cap is 24 plus 12 for each selected Command Car.
- Keep the grid as two rows at every cap. Expanded Command Car budget adds columns, and the cell
  width should shrink within the selected-panel width instead of introducing summarization,
  collapsing, hidden overflow, paging, or horizontal scroll.
- On overflow, briefly show red text. Prefer concise text such as `Selection limit reached` near
  the `used / cap` counter; if full rule text is used, keep it short:
  `You can command up to 24 supply at once.`
- Ensure the expanded Command Car cap can grow the visible grid beyond 24 cells.

## Expected Deliverables

- Multi-selection uses the budget grid instead of only per-kind chips.
- Tanks visibly occupy six cells as a two-row by three-column block.
- Command Cars expand the cap and visible grid by adding narrower columns, with every selected entity
  still represented directly.
- Overflow selection attempts produce a brief red warning/counter flash.
- Single-selection detail remains intact.

## Verification

- Add focused HUD tests or DOM-render tests for:
  - 24/24 infantry-style selection
  - four 6-supply Tanks
  - Command Car-expanded cap with narrower cells and no collapsed/summarized entries
  - overflow flash state
- Run the targeted HUD/command-card Node tests identified in Phase 0.
- Use a browser/manual check for layout at normal desktop and narrow viewport sizes if automated
  screenshots are not already available. Include at least one stacked-Command-Car selection large
  enough to prove the two-row narrower-cell rule is still legible.

## Manual Testing Focus

Select infantry, Tanks, mixed armies, buildings, and Command Car groups. Confirm text fits, blocks
do not overlap, the grid remains legible at common viewport sizes, and overflow feedback is visible
without being noisy.

## Handoff Expectations

The handoff must describe the final visual grammar for 1-, 2-, 3-, 4-, 5-, and 6-supply entities,
state how narrow cells become for stacked Command Cars, and call out any layout compromises that
should be watched in playtests.
