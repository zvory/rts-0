# Phase 3 - Real Unit Visual Overrides

## Phase Status

Status: not started.

## Objective

Allow checked-in visual candidates to override the rendered art for specific real scenario-backed
units. The units remain ordinary simulated entities, but the local renderer can draw different
checked-in rig candidates for different instances of the same unit kind.

## Scope

- Add profile rules for selecting real units by simple local selectors such as entity id, unit kind
  plus ordinal, or unit kind plus nearest world position.
- Add checked-in candidate registration for alternate unit SVG rigs or rig fragments through the
  existing rig importer/runtime path where practical.
- Route per-instance override decisions inside renderer drawing, not by changing `entity.kind`,
  `STATS`, protocol fields, authoritative snapshots, or simulation data.
- Reuse real runtime inputs for overridden units: movement position, facing, weapon facing, turret
  rotation, recoil, setup/deploy state, occupied-trench state, selection, HP bars, fog context, and
  shot-reveal behavior where applicable.
- Fail soft when a selector matches no unit, multiple units unexpectedly, or a candidate rig is
  invalid. Surface enough local diagnostics for a developer to fix the profile.
- Add a small real-unit profile that places or uses multiple same-kind units side by side with
  different candidate art.
- Audit the implemented workflow against [requirements.md](requirements.md) and document any
  remaining deferred polish or follow-up plan.

## Out Of Scope

- No fake unit kinds.
- No unit stat, command, combat, pathing, balance, fog, or protocol changes.
- No production-facing art selection UI.
- No collaborative or shareable visual review state.
- No arbitrary SVG, JavaScript, image, URL, or path loading.

## Expected Touch Points

- The visual profile registry from earlier phases
- `client/src/match.js`
- `client/src/renderer/index.js`
- `client/src/renderer/units.js`
- `client/src/renderer/rigs/live_routing.js`
- `client/src/renderer/rigs/svg_importer.js` only if the importer needs a narrow contract extension
- `client/src/renderer/rigs/runtime.js` only if runtime routing needs candidate-specific options
- `tests/rig_schema.mjs`
- `tests/svg_rig_importer.mjs`
- `tests/rig_runtime.mjs`
- New focused selector/override contract tests
- `docs/design/client-ui.md` if renderer rig routing contracts change
- `plans/visual-experimentation/requirements.md` only for factual audit notes or approved
  requirement clarifications
- `plans/visual-experimentation/phase-3.md` status marker in the implementation commit

## Edge Cases To Cover

- Two real units of the same kind can render different candidates in the same lab scene.
- Selecting and commanding an overridden unit still uses the real entity id and normal lab controls.
- Candidate art receives the same animation inputs as normal live art, including facing,
  weapon-facing, recoil, setup/deploy state, and vehicle track motion where relevant.
- Invalid candidate SVGs do not stop the match loop and should use the renderer's existing
  soft-failure path or a clear local diagnostic.
- A selector that no longer matches the scenario fails visibly for local developers.
- Overrides do not affect minimap blips, fog visibility, hit testing, command legality, unit ranges,
  HP/selection overlays, or server state.
- Normal matches and labs with no visual profile keep current live rig routing.

## Verification

- Focused selector and visual override contract tests added by the phase
- `node tests/rig_schema.mjs`
- `node tests/svg_rig_importer.mjs`
- `node tests/rig_runtime.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- A targeted browser/client smoke run for the override profile if local Chrome/server access is
  available
- `git diff --check`

## Manual Test Focus

Open the real-unit override profile in the lab and inspect multiple same-kind units with different
candidate art side by side. Select, move, attack, deploy where applicable, zoom, pan, and inspect fog
edges to confirm the units behave like real units while only the local art changes. Return to a
normal lab or match with no `visualProfile` and confirm standard unit art still renders.

## Handoff Expectations

Name the supported selector forms, candidate registration path, override routing helper, and the
profile URL used for manual testing. Include the requirements audit result and call out any remaining
visual polish that should be handled as a separate follow-up rather than hidden inside this phase.
