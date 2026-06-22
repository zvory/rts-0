# Phase 2 - SVG Authored Decal Assets

## Phase Status

- [ ] Done.

## Objective

Replace procedural placeholder marks with SVG-authored decal masks while preserving the Phase 1
runtime contract. SVGs should be easy for LLMs to inspect and edit, but runtime should rasterize the
assets and continue stamping bitmap masks into the single permanent decal texture.

## Scope

- Add SVG decal source assets.
  - Infantry/player paint masks: start with roughly 12 to 20 small variants.
  - Vehicle/support scorch masks: start with roughly 8 to 12 variants or a smaller set combined
    with procedural hull geometry.
  - Keep SVGs simple: explicit `viewBox`, flat paths/polygons/circles, alpha/white mask shapes,
    stable ids, no external images, no scripts, no remote refs, and no filters that are expensive
    or inconsistently rasterized.
  - Use names that describe the role, for example `infantry-splash-01.svg` and
    `vehicle-scorch-01.svg`.
- Add an asset manifest/loader that works with the no-build client.
  - Prefer static `.svg` files under a client-served asset path plus an ES module manifest listing
    their URLs.
  - If static SVG loading proves awkward in tests, use a narrowly-scoped JS module of SVG strings
    and document why.
  - Rasterize once per match/renderer lifetime into an offscreen atlas or `ImageBitmap` set.
  - Keep a procedural fallback from Phase 1 when assets fail to load.
- Implement player-tinted stamping.
  - Infantry marks can be strongly player-colored, since the desired read is team-colored paint
    rather than realistic red blood.
  - Vehicle/support marks should combine black/charcoal hull scorch with clear player-color paint,
    scrape, or melted-mark fragments.
  - Preserve owner color from Phase 1 recovery; fall back to neutral color when missing.
- Implement variation controls.
  - Deterministic variant index.
  - Seeded rotation, scale, flip, opacity, and small x/y offset.
  - Vehicle/support marks should use recovered facing for hull orientation when available.
  - Infantry marks can use seeded orientation independent of facing.

## Expected Touch Points

- `client/assets/decals/` or another clearly client-served SVG asset directory
- New manifest such as `client/src/renderer/decals/manifest.js`
- `client/src/renderer/decals.js` or split helpers under `client/src/renderer/decals/`
- `client/src/renderer/index.js` for loader lifecycle wiring
- Focused tests for asset manifest validity and tint/variant selection

Avoid touching:

- Server protocol files
- Rust simulation or lobby code
- Unit rig SVGs except to reuse safe parsing/rasterization helpers if they are already suitable
- Build tooling or frontend framework setup

## Implementation Details

- Keep source SVGs monochrome or alpha-mask-like so team tint can be applied at stamp time.
- Do not precolor SVGs for specific players.
- Do not stamp by inserting SVG DOM nodes into the game DOM.
- Do not create one Pixi sprite per chosen SVG.
- Do not load or rasterize the same SVG every time a unit dies.
- If using `Image`/`createImageBitmap`, make loader state explicit:
  - pending atlas;
  - ready atlas;
  - failed atlas with procedural fallback.
- Make renderer behavior deterministic before and after assets finish loading. A death that occurs
  before the atlas is ready should either use the procedural fallback immediately or queue until the
  atlas is ready; choose one behavior and document it.
- Keep all asset cleanup in the renderer destroy path. Abort or ignore late async loads after
  teardown.

## Verification

- `node scripts/check-client-architecture.mjs`
- `node --check` on any new JS asset manifest/helper files if they are not covered by a test.
- Focused test coverage for:
  - manifest has at least the expected infantry and vehicle/support variant counts;
  - SVG files are reachable from the manifest path;
  - SVG files avoid scripts/external refs;
  - tint selection uses owner player color;
  - deterministic variant/rotation/scale choices for a fixed seed.
- `git diff --check`

If SVG rasterization itself only works in a browser environment, cover pure manifest/selection logic
in Node and call out browser smoke/manual verification in the handoff.

## Manual Testing Focus

Kill multiple units owned by different players. Confirm marks are visibly player-tinted, varied
enough not to look repeated, grounded under fog/units, and still consistent with the low-resolution
terrain style. Check that reloading/rematching does not leave old SVG loads, textures, or decals
behind.

## Handoff Expectations

The handoff must state where SVG sources live, how the loader resolves them, what happens when
assets are not ready or fail, and whether the procedural fallback remains enabled. Include sample
screenshots or a clear visual note for at least one infantry and one vehicle/support death.
