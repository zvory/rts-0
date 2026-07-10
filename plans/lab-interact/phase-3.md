# Phase 3 - Screenshot MVP And Agent Workflow

Status: done.

Migration note: renderer readiness, clean presentation, capture, manifest, and graphics-review
work are completed underlying MVP capabilities. Phase 0 must deeply rename their paths and skill
and expose screenshot capture through the CLI; the former transport and image-content return are
not supported.

## Goal

Complete the initial Lab Interact milestone with reliable visual capture. An agent can arrange a
small authoritative scene, position the normal camera, write a clean Pixi PNG and bounded manifest,
inspect that local artifact once, and share its stable path with the user.

## Delivered Scope

- Renderer visual-asset readiness tracks live PNG atlases, frame strips, visual-profile overrides,
  decals, and other assets whose late arrival could produce fallback or incomplete captures.
- App-shell-owned clean presentation hides DOM chrome while preserving the normal Pixi viewport,
  camera, render loop, resize behavior, reversibility, and teardown.
- Capture readiness waits for authoritative scene evidence, requested room-time/camera/viewport/DPR,
  settled renderer assets and relevant fonts, two successful frames, and no page/frame/render or
  subject-texture errors.
- The bounded `screenshot` command requires the current opaque `sessionId` and accepts a safe
  artifact name, presentation mode, viewport/device-pixel-ratio override, and optional subject
  aliases. Camera movement remains a separate command.
- Browser clipping captures the composited visible Pixi viewport after DOM chrome is hidden.
- PNG and JSON manifests live under
  `<worktree>/target/lab-interact/<session-generation>/captures/` after the Phase 0 rename.
- Manifests record worktree root/branch/head, build, URL/mode, map/scenario/seed, authoritative tick
  and room-time state, viewport/DPR, camera, subjects, visual profile, runtime versions, asset
  readiness, errors, and bounded originating CLI request metadata.
- CLI output returns concise JSON with absolute PNG and manifest paths. The agent uses the local
  image viewer on the PNG path; image bytes are not printed or embedded in a transport response.
- The focused Lab Interact skill and repository graphics guidance keep scenes small, invoke the
  CLI, inspect the returned PNG exactly once, and share the path.

## Delivered Touch Points

- renderer asset readiness and rig helpers
- app/bootstrap/HTML/CSS clean-presentation composition
- bridge/driver capture and bounded command schema
- `.gitignore` output confinement
- `.agents/skills/lab-interact/SKILL.md` after Phase 0 deletes the old skill path
- `AGENTS.md`, client UI design/context, testing context, and troubleshooting guidance
- focused renderer/capture contracts and a live screenshot smoke

## Constraints

- Do not add video, deterministic render clocks, setup/replay persistence, image-diff thresholds,
  automatic visual approval, or source-asset writes in this MVP.
- Do not use Computer Use, Browser Use, mouse/keyboard UI automation, or token-heavy iterative page
  inspection. Puppeteer owns Chrome internally; the agent inspects only the returned local PNG.
- Never capture fallback rigs while authored assets are unresolved; fail or wait within a bound.
- Do not conflate hiding DOM chrome with hiding Pixi-native terrain, fog, selection, feedback, or
  effects.
- Do not make screenshots pixel-golden CI gates or put generated media/manifests into Git.
- Current output paths, launch gates, module/class names, diagnostics, and skill names must use Lab
  Interact/lab-interact after Phase 0.

## Verification To Preserve During Migration

- Client contracts for clean presentation, resize/camera stability, asset readiness/failure,
  rematch teardown, and hidden-DOM cleanup.
- CLI schema/result tests for safe names, output confinement, bounded metadata, returned absolute
  paths, and failure propagation.
- A first-use CLI screenshot smoke that spawns an aliased stationary tank, pauses/steps, focuses at
  fixed zoom, captures a 1000x700/DPR-1 PNG, and verifies dimensions, nontrivial size, manifest
  facts, and zero page/frame/render errors.
- A two-entity smoke or manual fixture covering camera fitting and alias summaries without a pixel
  golden.
- `node scripts/check-client-architecture.mjs`, focused client contracts and live screenshot smoke,
  docs health, and suite-selection verification.
- Inspect one produced PNG with the local image viewer and record its path in the Phase 0 handoff.

## Manual Testing Focus

- From a fresh Codex task in a graphics worktree, use CLI commands to capture one stationary unit,
  inspect the returned PNG once, and share the path.
- Repeat with two opposing units and confirm padding, faction colors, terrain, and clean
  presentation.
- Force an invalid visual asset and confirm capture fails clearly, then capture/reset/rematch again
  to confirm UI and WebGL teardown remain healthy.

## MVP Review Gate And Handoff Record

The underlying capture capability is complete, but its original Codex Desktop workflow failed at
the superseded transport boundary. After Phase 0 merges, repeat the manual images through the CLI
and review command usability, shared-session lifecycle, output paths, image quality, and failure
messages. Do not begin Phase 4 until that migrated workflow is explicitly approved.
