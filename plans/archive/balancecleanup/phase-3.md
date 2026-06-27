# Phase 3 - Client Config Split

Status: done.

## Goal

Split the browser config mirror into focused modules while preserving `client/src/config.js` as the
stable public import surface.

## Scope

- Move client-owned presentation data such as colors, fog alpha, camera defaults, labels, icons, and
  layout hints into clearly named client config modules.
- Move Rust-owned mirror data such as body records, client-visible scalars, `STATS`, `ABILITIES`,
  `UPGRADES`, `RESOURCE_AMOUNTS`, `WORKER_BUILDABLE`, `FACTION_CATALOGS`, and
  `FIXTURE_FACTION_ID` into focused mirror modules behind stable re-exports.
- Keep `client/src/config.js` exporting the same names and helper functions consumed by HUD,
  renderer, input, fog, minimap, lab, tests, and parity scripts.
- Preserve the current faction exposure boundary: `FACTION_CATALOGS` includes the checked
  fixture-only catalog, command-card helpers fail closed for unknown ids, and playable lobby
  selector data remains constrained by `PLAYABLE_FACTIONS`.
- Update `scripts/check-client-architecture.mjs` so `client/src/config/**` or
  `client/src/config_*.js` modules are classified as the pinned `rules-mirror` area and may import
  only the intended mirrors such as `protocol.js` or same-area config modules.
- Update `docs/design/client-ui.md` and `docs/context/client-ui.md` to describe the new internal
  config modules and import rule.
- Update architecture or hotspot grouping checks for the new internal module paths if Phase 1 did
  not already cover the exact paths.
- Do not change command-card order, labels, icons, hotkeys, costs, cooldowns, or helper return
  shapes.

## Touch Points

- `client/src/config.js`
- possible `client/src/config/*.js` or `client/src/config_*.js` files
- `client/src/lobby_view.js`, only if imports need a mechanical update that preserves
  `PLAYABLE_FACTIONS`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md`
- `scripts/hotspot-analysis.mjs` and `docs/hotspot-analysis.md`, only if Phase 1 did not already
  cover the chosen split paths
- `tests/client_contracts/config_contracts.mjs` or parity checks if a new split needs stronger
  public-surface assertions

## Constraints

- Preserve every exported name from `client/src/config.js`.
- Keep dependencies flowing through explicit imports; do not introduce cross-module imports that
  violate the client architecture check.
- Do not change any numeric values, catalog membership, ability descriptor, upgrade descriptor,
  command-card layout, or presentation string.
- Keep Rust-owned mirror data separate from client-only presentation modules so future reviews can
  see which fields are validated against Rust.
- Do not turn generated output into the source of truth unless Phase 2 explicitly chose generation
  and documented why reviewability remains acceptable.

## Verification

- `node scripts/check-faction-catalog-parity.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-wiki.mjs`
- `node scripts/check-docs-health.mjs` if docs are touched
- `git diff --check`

## Manual Testing Focus

No new gameplay behavior is expected. Manually open the main game or lab later and check that the
HUD command card, build menu, ability buttons, fog/render previews, and faction-specific menus still
look unchanged.

## Handoff

Mark this phase done only after committing the client split. Summarize the new module boundaries,
unchanged public exports, parity/client-contract verification, any architecture-check updates, and
whether Phase 4 may split Rust balance internals.
