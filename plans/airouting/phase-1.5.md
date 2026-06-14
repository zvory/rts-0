# Phase 1.5 - Static Atlas Editor View

Status: Planned.

## Objective

Add a dev/editor-only, non-editable atlas inspection tab to the map editor. The tab should make the
static atlas generated in Phase 1 legible to a designer so the team can verify that the AI's map of
the world is correct before Phase 2 route queries depend on it.

This phase should not change live AI decisions, map authoring data, gameplay behavior, or generated
artifacts committed to the repo.

## Product Requirements

- Primary consumer: designer/reviewer checking whether the AI has a correct static map model.
- The view must be diagnostic and legible, not polished or editable.
- The tab belongs in the existing map editor surface, not a standalone report viewer.
- The server should compute the atlas from the same authoritative map-loading path used by the
  simulation, then expose diagnostic data for the editor to render.
- Keep the first version static-only. Do not include dynamic influence maps, live unit state,
  route scoring, route memory, threat overlays, or Phase 2 route examples.
- Do not generate or commit PNG, SVG, Markdown, or other image/report artifacts. Rendering may use
  SVG, canvas, DOM overlays, or PixiJS internally if that is the simplest editor implementation,
  but the product is an interactive editor tab rather than exported files.
- Keep the feature dev/editor-only. It should not become normal in-match player UI.

## Required Layers

The tab should let the viewer inspect these Phase 1 atlas facts for each bundled authored map:

- Movement-class passability, with a clear way to switch between the initial ground movement
  classes implemented by Phase 1.
- Connected components, with stable component ids visible enough to compare against test failures
  or debug logs.
- Clearance field, rendered as a heatmap or equivalent visual scale so narrow and wide passages are
  obvious.
- Regions, with region boundaries or fills that make the atlas' area decomposition understandable.
- Portals, including center, adjacent region ids, movement classes, and width.
- Semantic anchors for starts, mains, naturals, resource clusters, and selected resource-line
  approach anchors, including their attached component and region.

## Scope

- Add a server-side dev/editor diagnostic endpoint or equivalent existing editor data path that
  returns static atlas debug data for a selected authored map.
- Keep the diagnostic payload derived from public map data and Phase 1 atlas output. Do not expose
  hidden live simulation state or player-specific fog data.
- Add a map editor tab, likely named `Atlas`, `Computed Atlas`, or `Topology`, that requests the
  selected map's atlas diagnostics and renders overlays on the map.
- Provide layer toggles and movement-class selection so the view can isolate one atlas fact at a
  time instead of producing an unreadable all-layers overlay.
- Include enough labels, legend text, hover details, or side-panel details for a designer to answer
  what a color, portal, region, component, clearance value, or anchor means.
- Preserve map editor editing workflows. The atlas tab must be read-only and must not mutate map
  JSON or editor state except ordinary view controls.
- Add focused tests for the diagnostic endpoint/data shape and any client-side layer state that can
  be tested without brittle pixel snapshots.
- Update the relevant design documentation to describe the atlas debug view and its dev/editor-only
  boundary.

## Expected Touch Points

- Atlas module from Phase 1
- Server map/editor routes in `server/src/main.rs` or the existing map catalog/save area
- Map editor client modules under `client/`
- Map editor tests if present, otherwise focused client contract tests for tab/layer state
- `docs/design/server-sim.md` if a public diagnostic map API is introduced
- `docs/design/client-ui.md` or map editor documentation if the editor surface is documented there
- `docs/design/ai.md` only if the AI atlas ownership/debug boundary needs clarification

## Verification

Run focused server and client tests that match the implementation:

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim map
node tests/client_contracts.mjs
```

If the implementation adds a narrower server route test, map editor test, or atlas diagnostic test,
run that instead of broad live suites. Run a live editor smoke check only if the tab wiring cannot
be covered by dep-free tests.

## Manual Testing Focus

Open the map editor on at least one bundled authored map and verify:

- The atlas tab appears only in the editor/dev surface.
- Switching movement class changes passability and clearance if the atlas data differs.
- Components, regions, portals, and semantic anchors are visually distinguishable.
- Portal widths and anchor component/region attachments can be inspected without reading raw JSON.
- Turning layers on and off keeps the view readable and does not mutate the map.

The key acceptance test is that a designer can inspect whether Scout Car/resource-line-relevant
topology has plausible components, portals, anchors, and clearance without reading atlas JSON.

## Handoff Expectations

The handoff must name the diagnostic data path, describe the editor tab controls, list the rendered
atlas layers, and call out any Phase 1 atlas facts that remain hard to inspect visually. It must
state that Phase 2 route-query work can use the view to sanity-check atlas assumptions but should
still rely on typed atlas/query APIs, not visual output.

## Player-Facing Outcome

No intended in-match player-facing change. Designers get a map editor diagnostic view for checking
that the static atlas accurately describes authored maps before AI routing starts consuming it.
