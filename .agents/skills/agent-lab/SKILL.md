---
name: agent-lab
description: Arrange and inspect small authoritative Bewegungskrieg scenes, then capture clean Pixi screenshots with the local Agent Lab MCP tools. Use for graphics, rendering, terrain, rig, animation, faction-color, fog, or camera review in a task worktree when Browser Use and Computer Use are unnecessary.
---

# Agent Lab Capture

Use the project-scoped `agent_lab` MCP server from a fresh trusted task attached to the worktree containing the change. It starts that worktree's normal private server and headless Pixi client; lab mutations are ephemeral and never edit source files.

1. Call `lab_open`, retain `sessionId`, then call `lab_catalog` before choosing players or kinds.
2. Keep the scene small: usually one stationary subject, or two opposing units. Use short aliases in `lab_spawn`; confirm changes with `lab_inspect`.
3. Use `lab_time` to pause or step authoritative state. Position the view with `lab_camera` rather than screenshot options. For one-unit detail captures, call camera `focus` without padding (the 32-world-pixel default is intentionally close). Multi-subject and non-unit focus keep their 48-world-pixel default; use explicit padding when the composition needs a different amount of context.
4. Call `lab_screenshot` with a safe name, normally `presentation: "clean"`, a bounded viewport such as 1000×700 at DPR 1, and any subject aliases. It waits for authoritative state, fonts, relevant visual assets, two frames, and error-free rendering.
5. Inspect the returned PNG image once. Share its `pngPath` with the user along with the concise scene result; the adjacent JSON manifest contains reproduction facts.
6. Call `lab_close` even after a failed capture. It is idempotent and cleans up the private browser and server.

Capture files are confined to `target/agent-lab/<session-id>/captures/` and are ignored by Git. Do not request arbitrary paths, add image bytes to Git, or use Agent Lab to play a full match.

## Recovery

- `chromeUnavailable`: install Chrome/Chromium or set `CHROME` before opening the session.
- MCP tools missing: reload Codex or start a fresh trusted task so `.codex/config.toml` is discovered.
- `assetLoadFailed`, `captureRenderError`, or `captureTimeout`: inspect the concise error and fix the source asset/render issue; do not accept a fallback capture.
- `occupied`/`labRejected`: choose a clear spawn location and confirm with `lab_inspect`.
- interrupted work or `sessionLimit`: call `lab_close`; idle sessions are also reaped after five minutes.
