# Phase 5 - Client UI, Prediction, Renderer, and Lab

Status: pending.

## Goal

Fix stale active docs for the browser client, HUD, command UX, prediction, renderer behavior, and
lab/debug tooling.

## Scope

- Audit client docs for app entry points, match lifecycle, HUD and command cards, input routing,
  hotkeys, settings, audio, alerts, minimap, status badges, prediction, frame recovery, renderer
  feedback, unit lab, replay inspection, and debug/lab control surfaces.
- Fix stale UI behavior claims and stale file/module routing that would mislead future agents.
- Update capsules only when their routing pointers or section references are stale.
- Do not redesign UI, retune visuals, or broaden into implementation changes unless generated docs
  are wrong because of a label/reference helper.

## Suggested Evidence

- `docs/context/client-ui.md`
- `docs/design/client-ui.md`
- `docs/context/protocol.md`
- `client/src/app.js`
- `client/src/main.js`
- `client/src/match.js`
- `client/src/hud.js`
- `client/src/input/**`
- `client/src/renderer/**`
- `client/src/prediction_*.js`
- `client/src/progress_extrapolator.js`
- `client/src/minimap.js`
- `client/src/settings*.js`
- `client/src/lab_*.js`
- `client/unit-lab.js`
- `tests/client_contracts.mjs`
- `tests/prediction_controller.mjs`
- `tests/progress_extrapolator.mjs`

Useful searches:

```bash
rg -n "HUD|command card|input|hotkey|prediction|renderer|minimap|settings|audio|lab|replay|status|frame|tooltip|locked" docs/design/client-ui.md docs/context/client-ui.md client/src tests -S
```

## Verification

Run focused checks that match the final diff. Likely commands:

```bash
node scripts/check-client-architecture.mjs
node scripts/check-docs-health.mjs
git diff --check
```

If generated wiki or client config labels are touched, also run `node scripts/check-wiki.mjs` and
`node scripts/check-faction-catalog-parity.mjs`.

## Manual Testing Focus

Later manual smoke should inspect the HUD command card, settings/audio panel, minimap, unit lab,
and replay/lab entry points named by changed docs.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must list stale UI claims fixed,
source evidence checked, verification run, and any UI flows that still need browser/manual testing.
