# Client Architecture Plan

## Purpose

Make the browser client trend toward lower coupling during ordinary feature work, without taking
large visual risks. The server has architecture checks and clear seams; the client should get the
same pressure toward good boundaries while staying conservative about anything players can see or
click.

The primary goal is amortized quality: each future client change should either preserve the current
module boundaries or improve them slightly. The first investments should be structural and easy to
verify with scripts, Node contract tests, and smoke tests.

## Overall Constraints

- Prefer ratchets over rewrites. Start from the current client shape and prevent new coupling.
- Treat UI behavior as high-risk. Refactors that touch DOM rendering, command cards, input, or Pixi
  drawing must have programmatic checks before they land.
- Keep visible UI unchanged during direct investments unless a phase explicitly says otherwise.
- Favor pure functions and explicit dependencies. New code should avoid implicit `this` helper
  coupling when an explicit argument list or small collaborator object works.
- Keep `Match` as the per-match composition root, but move special-mode logic out when it can be
  tested without a browser.
- Keep `GameState` as shared client model state, not an input implementation owner.
- Implement one phase at a time. Each phase must be committed, merged to `main`, and pushed before
  the next phase begins.

## Phase Summaries

### [Phase 1 - Client Architecture Checker](phase-1.md)

Add a lightweight static checker for `client/src` that maps modules into coarse areas and prevents
new coupling while accepting the current client shape. Wire it into suite selection and the normal
test runner so client architecture drift is caught locally. This phase has no runtime behavior or
visual impact.

### [Phase 2 - Neutral Command Targeting Boundary](phase-2.md)

Move the command targeting state machine out of `input/` to a neutral client module so `GameState`
no longer imports an input implementation detail. Keep the exported class, public methods, command
targeting semantics, minimap behavior, hotkeys, and HUD command-card behavior unchanged. This phase
is a path move plus import and documentation updates, with command-targeting tests as proof.

### [Phase 3 - Match Health Extraction](phase-3.md)

Extract latency, jitter, server lag, and status-badge payload bookkeeping from `Match` into a small
`MatchHealth` collaborator. Keep `StatusBadge` and the visible status payload shape unchanged while
preserving the frame-loop behavior through delegation. This phase reduces `Match` responsibility
without changing gameplay, rendering, input, or layout.

### [Phase 4 - Replay Controls Extraction](phase-4.md)

Move replay and scenario speed, seek, pause, step, status, and vision controls from `Match` into a
dedicated `ReplayControls` collaborator. Preserve the existing DOM structure, classes, labels,
`data-*` attributes, hidden states, and cleanup behavior. Because this touches UI-adjacent code,
DOM contract tests are required before the phase is considered complete.

### [Phase 5 - Command Card Descriptor Layer](phase-5.md)

Introduce a pure descriptor layer for HUD command-card buttons so command availability and metadata
can be tested without rendering DOM. Keep the existing HUD renderer responsible for creating the
same HTML, classes, titles, hotkeys, repeatability flags, and click behavior from those descriptors.
This phase is intentionally conservative because command cards are player-facing and command
semantics must stay unchanged.

### [Phase 6 - Agent Workflow and Documentation](phase-6.md)

Document the client architecture rules, checker command, module map, and future-change checklist in
the client UI design/context docs. Make teardown ownership, cross-area imports, command-card
coverage, rendering verification, mirrored protocol/config contracts, and large-file ratchets easy
for future agents to follow. This phase should be docs-only except for small checker or
test-selector polish if a previous phase revealed a workflow gap.

## Non-Goals

- Do not redesign the visual style or HUD layout as part of architecture cleanup.
- Do not rewrite the client into a framework or add a JS build step.
- Do not convert the whole renderer/input helper system in one pass.
- Do not use line count alone as a quality proxy. Use it as one ratchet alongside import direction,
  public seams, teardown ownership, and behavior tests.
- Do not make gameplay, balance, protocol, or command semantics changes unless a phase explicitly
  calls for a behavior-preserving move.

## Required Order

Phases 1 and 2 are the highest-confidence direct investments. They should be done first because
they add enforcement and remove the current concrete boundary inversion without touching visible UI.

Phases 3 through 5 should be done one at a time, with each phase landing only after its contract
tests and smoke test pass. If any phase starts requiring visual judgment to prove correctness, stop
and add a narrower programmatic check before continuing.

Phase 6 should come after the implementation phases so its documentation can describe the actual
checker behavior and extracted seams that landed. Do not start a later phase from an unmerged phase
branch; update `main`, create a fresh phase branch/worktree, and continue from the pushed result.

## Agent Progress And Handoff Rules

Each phase file has an "Implementation Segments" checklist. Agents must mark each segment complete
as they finish it, not only at the end. If a segment becomes unnecessary, mark it complete with a
short note explaining why it was skipped.

At handoff, agents must include:

- the final completed segment checklist
- the verification commands they ran and their results
- the "Manual Test Prompt" from that phase, filled in with anything that changed or could not be
  verified automatically
- what the next agent should do next, including whether to continue with the next numbered phase or
  address a discovered gap first

Manual testing should stay scoped to the changed surface. If an agent believes a broader manual pass
is needed, they should say which automated or contract check is missing before asking for it.

When a phase is complete, mark that phase document as done in the same implementation commit. Do
not mark later phase documents complete early.
