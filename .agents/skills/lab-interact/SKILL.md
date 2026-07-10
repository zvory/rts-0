---
name: lab-interact
description: Arrange and inspect small authoritative Bewegungskrieg scenes, then capture clean Pixi screenshots with the local Lab Interact CLI. Use for graphics, rendering, terrain, rig, animation, faction-color, fog, or camera review in a task worktree when Browser Use and Computer Use are unnecessary.
---

# Lab Interact Capture

Use `node scripts/lab-interact/cli.mjs <command> '<JSON-object>'` from the task worktree. The first
command starts that worktree's private daemon automatically. Lab mutations are ephemeral and never
edit source files.

1. Run `open`, retain `result.sessionId`, then run `catalog` before choosing players or kinds.
2. Keep the scene small. Use short aliases in `spawn` and confirm changes with `inspect`.
3. Use `time` to pause or step authoritative state. Position the view with `camera`. Single-unit
   focus without padding uses the intentionally close 32-world-pixel default.
4. Run `screenshot` with a safe name, normally `presentation: "clean"`, a bounded viewport such as
   1000×700 at DPR 1, and any subject aliases.
5. Open the returned `pngPath` once with the local image viewer. Share that path and a concise scene
   result; the adjacent JSON manifest contains reproduction facts.
6. Run `close` when the session is complete. Use `shutdown` for immediate daemon teardown; otherwise
   it closes itself after 30 minutes without an accepted interaction.

Capture files are confined to `target/lab-interact/<session-id>/captures/` and ignored by Git. Do
not request arbitrary paths, add image bytes to Git, or use Lab Interact to play a full match.

## Recovery

- `chromeUnavailable`: install Chrome/Chromium or set `CHROME` before `open`.
- `assetLoadFailed`, `captureRenderError`, or `captureTimeout`: fix the source asset/render issue;
  do not accept a fallback capture.
- `occupied`/`labRejected`: choose a clear spawn location and confirm it with `inspect`.
- interrupted work or `sessionLimit`: run `status`, then `close` the active session or `shutdown`.
