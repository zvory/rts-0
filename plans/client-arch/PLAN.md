# Client Architecture Plan

## Purpose

Make the browser client trend toward lower coupling during ordinary feature work, without taking
large visual risks. The server has architecture checks and clear seams; the client should get the
same pressure toward good boundaries while staying conservative about anything players can see or
click.

The primary goal is amortized quality: each future client change should either preserve the current
module boundaries or improve them slightly. The first investments should be structural and easy to
verify with scripts, Node contract tests, and smoke tests.

## Principles

- Prefer ratchets over rewrites. Start from the current client shape and prevent new coupling.
- Treat UI behavior as high-risk. Refactors that touch DOM rendering, command cards, input, or Pixi
  drawing must have programmatic checks before they land.
- Keep visible UI unchanged during direct investments unless a phase explicitly says otherwise.
- Favor pure functions and explicit dependencies. New code should avoid implicit `this` helper
  coupling when an explicit argument list or small collaborator object works.
- Keep `Match` as the per-match composition root, but move special-mode logic out when it can be
  tested without a browser.
- Keep `GameState` as shared client model state, not an input implementation owner.

## Phase Index

1. [Phase 1 - Client Architecture Checker](phase-1-client-architecture-checker.md)
2. [Phase 2 - Neutral Command Targeting Boundary](phase-2-neutral-command-targeting-boundary.md)
3. [Phase 3 - Match Health Extraction](phase-3-match-health-extraction.md)
4. [Phase 4 - Replay Controls Extraction](phase-4-replay-controls-extraction.md)
5. [Phase 5 - Command Card Descriptor Layer](phase-5-command-card-descriptor-layer.md)
6. [Phase 6 - Agent Workflow and Documentation](phase-6-agent-workflow-and-documentation.md)

## Non-Goals

- Do not redesign the visual style or HUD layout as part of architecture cleanup.
- Do not rewrite the client into a framework or add a JS build step.
- Do not convert the whole renderer/input helper system in one pass.
- Do not use line count alone as a quality proxy. Use it as one ratchet alongside import direction,
  public seams, teardown ownership, and behavior tests.
- Do not make gameplay, balance, protocol, or command semantics changes unless a phase explicitly
  calls for a behavior-preserving move.

## Suggested Order

Phases 1 and 2 are the highest-confidence direct investments. They should be done first because
they add enforcement and remove the current concrete boundary inversion without touching visible UI.

Phases 3 through 5 should be done one at a time, with each phase landing only after its contract
tests and smoke test pass. If any phase starts requiring visual judgment to prove correctness, stop
and add a narrower programmatic check before continuing.
