# Visual Experimentation Multi-Phase Plan

## Purpose

Implement the local visual experimentation workflow described in [requirements.md](requirements.md).
The first usable slice should let a developer open a lab URL, view checked-in labeled trench
candidates in the real renderer, pan and zoom around them, then iterate by editing checked-in files
and refreshing. Later work should add per-instance real-unit visual overrides without changing
simulation, protocol, balance, fog, command, or lab scenario data.

This plan is ready for executor implementation. Run one phase at a time from a clean worktree and do
not start a later phase until the previous phase PR has merged to `origin/main`.

## Readiness Review

- The requirements are specific enough to build from: profile id only on the URL, checked-in
  allowlisted registry, no arbitrary asset paths, renderer-owned static samples, and local-only
  per-instance unit overrides.
- No product decision is needed before Phase 1. The plan chooses a small implementation path where
  the browser parses and resolves a local profile while the server room, wire protocol, and
  checkpoint-backed lab setup payloads remain unchanged.
- The first iteration loop is complete after Phase 2. Phase 3 broadens the workflow to live unit rig
  experiments once the lab profile and renderer-only sample surface are already proven.

## Phase Summaries

### [Phase 1 - Profile Launch And Registry](phase-1.md)

Add sanitized `visualProfile` parsing to the lab launch path and resolve it through a checked-in
client registry. A missing profile keeps normal lab behavior, while an invalid or unknown profile
fails closed with a local developer-visible error and no asset/path fetch. The phase may apply an
initial camera view from the profile, but it does not draw candidates yet.

### [Phase 2 - Static Entrenchment Samples](phase-2.md)

Render profile-owned, renderer-only entrenchment samples and lightweight labels inside the real lab
renderer. The samples must live outside `GameState`, snapshots, selection, commands, minimap blips,
fog sources, and scenario authoring data. This is the first shippable visual iteration loop for
comparing trench candidates at gameplay scale.

### [Phase 3 - Real Unit Visual Overrides](phase-3.md)

Add per-instance visual override selection for real scenario-backed units without changing their
unit kind or simulation behavior. Alternate checked-in SVG rig candidates should reuse the existing
rig importer and runtime animation inputs so movement, facing, weapon facing, recoil, setup state,
selection, HP bars, and fog context remain real. The phase should finish by auditing the delivered
workflow against the visual experimentation requirements and documenting any deferred polish.

## Overall Constraints

- Keep this workflow local and lab-scoped. Do not add production-facing visual experiment UI,
  multiplayer synchronization, hot reload, remote asset loading, file pickers, uploads, or public
  catalog persistence.
- Do not change the server wire protocol, compact snapshot shape, checkpoint payloads,
  checkpoint-backed lab setup JSON, local lab setup exports, balance values, commands, combat,
  fog, pathing, minimap authority, or simulation state for this local workflow.
- Treat the URL as an id selector only. `visualProfile` must be sanitized, must not enter the lab
  room string as executable data, and must never be interpreted as a path, URL, SVG body, module
  name, or image source.
- Resolve profiles and candidates only through checked-in allowlisted code or data. Broken profiles
  or candidates should fail soft, log or surface a local error, skip the broken piece where possible,
  and keep the lab usable.
- Preserve client architecture boundaries. Use `App` and `Match` as the composition points, keep lab
  transport/UI app-owned, classify any new top-level client files in
  `scripts/check-client-architecture.mjs`, and prefer renderer-owned helpers under
  `client/src/renderer/` for drawing.
- Keep renderer-only samples in a profile-owned read model that is passed to the renderer. Do not
  write them into `GameState`, authoritative snapshots, lab scenario setup, selection state,
  command targeting, fog visibility, or minimap entity sources.
- Any module that owns Pixi display objects, textures, DOM listeners, or GPU resources must support
  normal match teardown.
- Keep the first implementation deliberately simple. Restarting the server or refreshing the browser
  after editing profile/candidate files is acceptable.

## Executor Workflow

- Each phase must be implemented in its own clean `/tmp/rts-worktrees` worktree on a `zvorygin/`
  branch.
- Each phase must be committed, pushed, opened as an owned PR, and have auto-merge armed with
  `scripts/agent-pr.sh`.
- After opening each phase PR, the implementing agent must run `scripts/wait-pr.sh <pr>` and wait
  until GitHub reports the PR merged and the head SHA is reachable from `origin/main` before
  reporting the phase complete or starting the next phase.
- When a phase is complete, mark that phase document as done in that phase's implementation commit.
- After each phase, the implementing agent must provide a handoff message for the next phase. The
  handoff must describe completed behavior, changed files/contracts, verification commands, known
  risks, and the core manual checks the next agent should run.
- Manual testing notes should cover the core feature risks for that phase, not an exhaustive matrix.
