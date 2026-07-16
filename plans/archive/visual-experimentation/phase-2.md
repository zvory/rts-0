# Phase 2 - Static Entrenchment Samples

## Phase Status

Status: done.

## Objective

Make the first useful visual experimentation loop work. A developer should be able to open a lab URL
with the checked-in trench profile and see multiple labeled entrenchment visual candidates rendered
in the real PixiJS match renderer, with normal terrain, units, buildings, fog, selection rings, HP
bars, pan, and zoom still available.

## Scope

- Add a profile-owned renderer read model for static visual samples. It should be built from the
  resolved profile and passed into `Renderer.render` without mutating `GameState`.
- Add renderer-owned drawing for static entrenchment candidates. The first implementation may use
  checked-in procedural candidate descriptors or checked-in registered assets, but every candidate
  must come from the allowlisted profile/registry path.
- Add lightweight world-space labels for candidates, using pooled Pixi objects or another
  renderer-owned teardown-safe mechanism.
- Keep samples out of selection, hit testing, command targeting, minimap blips, fog sources,
  snapshots, scenario authoring data, and local lab scenario exports.
- Ensure broken candidate entries fail soft: skip the broken candidate where possible, record a
  local render/profile error, and continue rendering the rest of the scene.
- Include at least one profile that compares several entrenchment candidates side by side near real
  lab terrain and units.
- Update focused tests around the profile read model, renderer sample normalization, no-GameState
  mutation, and invalid candidate handling.

## Out Of Scope

- No real-unit rig overrides.
- No dynamic asset loading from URLs or user-provided paths.
- No server-side trench state, infantry behavior, occupied-trench logic, fog, minimap, command, or
  protocol changes for static samples.
- No requirement to hot reload candidate files.

## Expected Touch Points

- `client/src/match.js`
- `client/src/frame_recovery.js`
- `client/src/renderer/index.js`
- `client/src/renderer/trenches.js` or a new renderer-owned visual experiment helper
- `client/src/renderer/layers.js` if a dedicated visual label/sample layer is needed
- The visual profile registry from Phase 1
- `tests/client_contracts/renderer_contracts.mjs`
- `tests/client_contracts/ground_decal_contracts.mjs` or new focused visual experiment contracts
- `docs/design/client-ui.md` if renderer APIs, layers, or exported contracts change
- `plans/visual-experimentation/phase-2.md` status marker in the implementation commit

## Edge Cases To Cover

- Static sample records are rendered even though they are absent from `state.entities`,
  `state.trenches`, snapshots, and `state.selection`.
- Static samples never appear in minimap entity rendering or fog-source entity lists.
- Labels stay aligned to world positions across pan and zoom and do not leak DOM or Pixi objects
  after `Match.destroy()`.
- A profile with one invalid candidate still renders the valid candidates.
- Static samples draw in a layer that makes sense against terrain, real trenches if present, units,
  HP bars, selection rings, fog, and command feedback.
- The normal trench renderer continues to draw authoritative trench state when no visual profile is
  active.

## Verification

- Focused visual experiment client contract tests added by the phase
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node tests/select-suites.mjs --verify`
- A targeted browser/client smoke run if the phase adds visual behavior that can be exercised
  locally
- `git diff --check`

## Manual Test Focus

Run the server and open the trench visual profile in the lab. Pan and zoom from close inspection to
normal gameplay scale, confirm labels identify each candidate, and compare candidates against real
terrain, units, buildings, selection rings, HP bars, fog, and any real trench state in the chosen
scenario. Confirm candidate samples cannot be selected, commanded, seen on the minimap as units, or
submitted as lab scenario data.

## Handoff Expectations

Describe the renderer read model, candidate shape, label implementation, and how static samples are
kept separate from `GameState`. Name the manual URL used for the trench profile and list any visual
polish that Phase 3 should avoid mixing into real-unit override work.
