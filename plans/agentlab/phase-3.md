# Phase 3 - Screenshot MVP And Agent Workflow

Status: done.

## Goal

Complete the initial Agent Lab milestone with reliable visual capture. An agent should be able to
arrange a small authoritative scene through Phase 2 tools, position the normal camera, receive a
clean Pixi screenshot as image content, inspect it once, and share a stable local artifact path with
the user.

## Scope

- Track renderer visual-asset loading explicitly. The renderer should retain bounded readiness
  promises/status for live PNG atlases, frame strips, visual-profile overrides, decals, and other
  assets whose late arrival could make a screenshot capture a fallback or incomplete frame.
- Add a shared clean-presentation mode owned by the app shell. It should hide the version badge,
  HUD, minimap, menus, lab panels, room-time controls, toasts, overlays, and other DOM chrome while
  keeping the normal Pixi viewport/camera/render loop alive and correctly resized.
- Keep clean presentation reversible and teardown-safe so sequential captures or rematches do not
  leak classes, listeners, dimensions, or hidden UI state.
- Extend the driver with a capture-readiness barrier:
  - authoritative scene mutation/order result observed;
  - requested room-time state confirmed;
  - requested camera/viewport/device-pixel-ratio applied;
  - relevant renderer assets settled successfully;
  - document fonts settled where they affect Pixi labels;
  - at least two successful animation frames after the final change;
  - no uncaught page errors, frame-loop errors, missing-texture fallback for the subject, or
    unresolved render errors.
- Add `lab_screenshot` with bounded inputs such as session id, safe artifact name, presentation
  mode, viewport/device-pixel-ratio override, and optional subject aliases for manifest focus.
  Camera movement should normally happen through `lab_camera`, not a growing screenshot-options
  object.
- Capture the visible Pixi viewport after DOM chrome is hidden. Do not rely on
  `ElementHandle.screenshot()` alone to exclude overlapping DOM; browser screenshot clipping
  captures composited pixels.
- Write PNG and JSON manifest under
  `<worktree>/target/agent-lab/<session-id>/captures/`. Add `/target/agent-lab` to `.gitignore`.
- Include in the manifest: worktree root/branch/head, server build, URL/mode, map/scenario/seed,
  authoritative tick and room-time state, viewport/DPR, camera, selected subject summaries,
  visual-profile id, Chrome/Puppeteer versions, asset readiness, page/frame/render errors, and the
  originating MCP request metadata needed to reproduce the capture.
- Return the PNG as MCP image content plus a concise structured result containing the absolute
  artifact and manifest paths. If the host cannot render returned image content, the path must
  remain directly inspectable through the existing local image viewer.
- Add a focused project skill for the Agent Lab workflow and update `AGENTS.md` specialized
  guidance for graphics/rendering changes. The guidance should tell agents when to use the tools,
  how to keep scenes small, how to inspect the returned PNG, and how to share it with users.
- Document failure recovery for missing Chrome, stale MCP configuration, asset load failure,
  occupied spawn position, timeout, and orphan cleanup.

## Expected Touch Points

- `client/src/renderer/index.js` and narrow asset-loader/rig helpers
- `client/src/app.js`, `client/src/bootstrap.js`, `client/index.html`, and client CSS for clean
  presentation composition
- the Agent Lab bridge/driver/MCP schemas from Phases 1-2
- `.gitignore`
- `.agents/skills/agent-lab/SKILL.md` and only the focused references/scripts it needs
- `AGENTS.md`
- `docs/design/client-ui.md`, `docs/context/client-ui.md`, and `docs/context/testing.md`
- focused renderer/capture contracts plus one live screenshot smoke

## Constraints

- Do not add video, deterministic render clocks, setup/replay persistence, image diff thresholds,
  automatic visual approval, or source-asset writing.
- Do not use Computer Use, Browser Use, mouse/keyboard UI automation, or token-heavy iterative page
  inspection. Puppeteer may own Chrome internally, and the agent should inspect only the returned
  artifact.
- Do not silently capture a fallback rig while an authored asset is still loading. Fail with an
  actionable asset error or wait within the bounded timeout.
- Do not hide Pixi-native world layers such as terrain, fog, selection, feedback, or effects unless
  the tool has a small documented presentation option. DOM chrome hiding and world-layer policy are
  different concerns.
- Do not make screenshots a required pixel-golden CI comparison. Browser smoke should validate
  dimensions, nonempty output, readiness metadata, and absence of capture errors.
- Do not put image bytes or manifests into Git, PR bodies, or source directories.

## Verification

- Add client contracts for clean presentation enter/exit, viewport resize/camera stability,
  renderer asset readiness, late-load failure, rematch teardown, and no hidden-DOM leak after
  capture.
- Add MCP schema/result tests for safe names, output-root confinement, bounded metadata, image
  content MIME/type, and failure propagation.
- Add a private-server screenshot smoke that creates an aliased stationary tank in a clear region,
  pauses/steps to observe it, focuses at a fixed zoom, captures a fixed 1000x700/DPR-1 PNG, and
  verifies PNG dimensions, nontrivial byte size, manifest facts, and zero page/frame/render errors.
- Add a second smoke or manual fixture with two entities so camera fitting and alias summaries are
  covered without a pixel golden.
- Run `node scripts/check-client-architecture.mjs`.
- Run `node tests/client_contracts.mjs` and the focused live screenshot smoke.
- Run `node scripts/check-docs-health.mjs` and `node tests/select-suites.mjs --verify` if routing
  changes.
- Use the local image viewer to inspect the produced PNG during implementation and record the
  artifact path in the phase handoff.

## Manual Testing Focus

- From a fresh Codex task attached to a graphics-change worktree, ask the agent to spawn and capture
  one stationary unit, inspect the image, and share the file.
- Repeat with two opposing units framed together and confirm camera padding, faction colors,
  terrain, and clean presentation are correct.
- Force one missing/invalid visual asset and confirm capture fails clearly instead of returning a
  misleading fallback image.
- Capture, leave clean mode, reset/rematch, and capture again to confirm UI and WebGL teardown stay
  healthy.

## MVP Review Gate And Handoff

After implementation, mark this phase done and report the screenshot tool schema, readiness
barrier, clean-presentation ownership, output/manifest locations, skill/AGENTS workflow, focused
verification, and the two manually reviewed images. Stop the implementation sequence here and ask
for explicit review of tool usability and capture quality before Phase 4; do not treat the
existence of later phase files as authorization to continue automatically.
