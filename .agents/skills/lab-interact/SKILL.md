---
name: lab-interact
description: Arrange and inspect small authoritative Bewegungskrieg scenes, then capture clean Pixi screenshots with the local Lab Interact CLI. Use for graphics, rendering, terrain, rig, animation, faction-color, fog, or camera review in a task worktree when Browser Use and Computer Use are unnecessary.
---

# Lab Interact Capture

Use `node scripts/lab-interact/cli.mjs <command> '<JSON-object>'` from the task worktree. The first
command starts that worktree's private daemon automatically. Lab mutations are ephemeral and never
edit source files. Use `node scripts/lab-interact/cli.mjs help <command>` or
`node scripts/lab-interact/cli.mjs <command> --help` when an exact input shape, default, or bound is
uncertain; command help never starts or inspects the daemon.

1. Run `open`, retain `result.sessionId`, then run `catalog` before choosing players or kinds.
   `open` is safe to repeat: it returns the active session. Run `close` first only when a fresh
   session or different launch options are needed. A cold build may take tens of seconds and emits
   its one JSON result only after readiness; keep the process attached, or use concurrent `status`
   to confirm `opening: true`.
2. Keep the scene purposeful. Use one bulk `spawn`, `update`, or `remove` request for related
   changes, with short aliases where useful, and confirm the coherent result with `inspect`. These
   operations accept up to 400 inputs/references; aliases, inspection, focus, and screenshot
   subjects use the same large-scene bound. During autonomous combat, use `activity`, `targetId`,
   and HP changes rather than treating `state: "idle"` or an empty explicit `orderPlan` as proof
   that no engagement occurred. Adjust producer auto-build with one normal `order` command shaped
   as `{c:"adjustProductionRepeat",buildings:[...],unit:"<kind>",delta:1|-1}`; each command adds
   or removes one allocation across the resolved producer set.
3. Use `time` to pause or step authoritative state. Position the view with `camera`. Single-unit
   focus without padding uses the intentionally close 32-world-pixel default.
4. Run `screenshot` with a safe name, a bounded viewport such as 1000×700 at DPR 1, and any subject
   aliases. Use `presentation: "clean"` to hide UI chrome or `presentation: "normal"` to retain
   visible Lab panels and game UI.
5. Inspect the returned capture once during local QA. The CLI returns `result.preview.url` for every
   visual artifact: share that Tailnet URL and a concise scene result with the user. Never share a
   raw `target/lab-interact` path; the adjacent JSON manifest remains local reproduction evidence.
6. Run `close` when the session is complete. Use `shutdown` for immediate daemon teardown; otherwise
   it closes itself after 30 minutes without an accepted interaction.

Capture files are confined to `target/lab-interact/<session-id>/captures/` and ignored by Git. Do
not request arbitrary paths, add image bytes to Git, or use Lab Interact to play a full match. A
Tailnet Preview URL remains available while the Lab daemon is running; `close` preserves it, while
`shutdown` or daemon idle teardown removes it.

## Recovery

- `chromeUnavailable`: install Chrome/Chromium or set `CHROME` before `open`.
- `assetLoadFailed`, `captureRenderError`, or `captureTimeout`: fix the source asset/render issue;
  do not accept a fallback capture.
- `occupied`/`labRejected`: inspect the structured blocker and bounded authoritative suggestions;
  retry the corrected batch with one returned legal position when appropriate.
- `daemonCheckoutMismatch`: run `status` to inspect the preserved scene. When it is safe to discard,
  run the returned `shutdown` recovery command and retry from the current checkout; never silently
  replace an active mismatched daemon.
- interrupted work: run `open` to recover the active session id, or `status` to inspect it. Run
  `close` followed by `open` only when the existing scene should be discarded.
