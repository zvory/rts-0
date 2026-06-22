# Phase 5 - HUD Helper Extraction

Status: done.

## Goal

Extract focused helpers from `client/src/hud.js` while preserving the existing `HUD` public surface
and player-visible command-card behavior.

## Scope

- Read `docs/context/client-ui.md` and the HUD section of
  `plans/hotspots/responsibility-map.md`.
- Reuse the local helper pattern from `hud_command_card.js`, `hud_selection_panel.js`, and
  `hud_unit_commands.js`.
- Extract resource-row rendering, control-group tab rendering, and command-card DOM/button rendering
  where doing so reduces `hud.js` context without hiding command semantics.
- Keep command intent dispatch, command issuer calls, affordability checks, cooldowns, missing
  resource feedback, hotkey resolution, and `ClientIntent` usage stable.
- Update `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if new HUD helper paths are
  not grouped under `client-hud`.
- Prefer using the split client contract domain files if Phase 2 created them; otherwise run the
  stable full client contract command.

## Touch Points

- `client/src/hud.js`
- `client/src/hud_*.js` or narrowly named HUD helper modules
- HUD-related client contract files
- `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if grouping needs new helper paths
- `plans/hotspotcleanup/phase-5.md`

## Constraints

- Do not change command ids, descriptors, stat values, affordability rules, cooldown display,
  selected-unit panel behavior, control-group behavior, or command issuer calls.
- Do not introduce broad cross-area imports. Helpers should receive dependencies explicitly or stay in
  the HUD area.
- Do not change balance/config numbers.
- New modules that hold listeners or resources must implement and be called through `destroy()`, but
  this phase should avoid adding such modules unless necessary.

## Verification

- `node tests/client_contracts.mjs` or the targeted HUD contract file plus the stable runner
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-faction-catalog-parity.mjs` only if config descriptors are touched
- `git diff --check`

## Manual Testing Focus

Manually check a local match for resource display, selection panel, control groups, command-card
actions, disabled affordability states, cooldowns, and missing-resource feedback.

## Handoff

After implementation, mark this phase done and summarize the new HUD helper responsibilities,
commands run, manual checks performed or still needed, and any command-card logic deliberately left in
`hud.js`.
