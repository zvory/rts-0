# Client Boundary Refactor Plan

## Purpose

Refactor browser client composition and state boundaries so `Match` remains the app-shell composer
and `GameState` stops accumulating unrelated transient controller/UI state. HUD, input, minimap,
renderer feedback, and prediction should communicate through explicit small facades instead of
mutating broad shared state.

## Overall Constraints

- Do not change wire protocol, balance mirrors, command semantics, or server authority.
- Keep the no-build-step ES module client and global PixiJS convention.
- Maintain `destroy()` ownership for listeners, timers, WebSocket handlers, Pixi objects, textures,
  and GPU resources.
- Avoid new non-shell cross-area imports unless documented in `scripts/check-client-architecture.mjs`
  with a reason.
- Run `node scripts/check-client-architecture.mjs` after each client implementation phase.
- After each phase, provide a handoff naming verification results, remaining compatibility shims,
  and the core client flows that should be manually tested.
- Implement, commit, merge to `main`, and push each phase before starting the next phase.

## Boundary Invariants

- `Match` is the only cross-area composer. HUD, input, minimap, renderer, and prediction receive
  explicit dependencies from `Match`; they do not import each other to coordinate behavior.
- `GameState` owns authoritative snapshot view state, selection, control groups, relationship
  helpers, fog-facing visibility data, and named display overlays only.
- `ClientIntent` owns browser-local cursor/command intent: placement, command-card submenu state,
  command targeting, click feedback, hover previews, support-weapon previews, and ability previews.
- Gameplay commands continue to flow only through `commandIssuer.issueCommand`; intent facades must
  never call `Net.command` directly.
- Renderer inputs are data-only read models. Renderer code must not prune TTLs, mutate intent, or
  call state methods with side effects during draw.
- Prediction display overlays must never affect fog, authority reads, spectators, replay viewers,
  dev-watch passive viewers, hidden enemies, allies, resources, ability objects, or command
  sequence allocation policy.

## Phase Summaries

### [Phase 1 - Baseline Contracts](phase-1.md)

Add executable coverage for current client intent, preview, HUD dispatch, renderer feedback, and
prediction optimism behavior before moving state. Document the target boundary and phased migration
rule in the client design doc. This phase should not move runtime behavior.

### [Phase 2 - Client Intent State](phase-2.md)

Extract placement, command-card submenu state, command targeting, command feedback, and preview
slots from `GameState` into a model-area client intent helper. Keep `GameState` compatibility
accessors for one phase so existing input, HUD, minimap, and renderer code still works. This creates
the new API while minimizing immediate churn.

### [Phase 3 - HUD And Input Intent Facade](phase-3.md)

Route HUD, input, and minimap through the explicit client intent facade. Command issuing should
still go through `commandIssuer.issueCommand` so prediction sequencing and command-budget behavior
remain stable. Temporary `GameState` compatibility fields can remain until renderer migration is
done.

### [Phase 4 - Renderer Feedback View Model](phase-4.md)

Give renderer feedback a stable narrow view model for placement, command feedback, previews,
ability objects, and selected entities. Drawing should remain visually identical. Tests should
assert the view-model shape so future renderer changes do not reach back through broad state.

### [Phase 5 - Prediction Display Boundary](phase-5.md)

Separate prediction bookkeeping from `GameState` display mutation through an explicit prediction
view/update seam. `PredictionController` should keep client sequence and optimistic bookkeeping,
while `GameState` applies a named display overlay. Spectator, replay, and dev-watch paths must stay
prediction-disabled.

### [Phase 6 - Remove Shims And Tighten Policy](phase-6.md)

Remove temporary `GameState` compatibility fields after HUD, input, minimap, and renderer use the
explicit facades. Update the client architecture checker, large-file baselines, and docs to enforce
the new boundaries. This should be cleanup-only, not a new feature phase.

## Remaining Phase Preconditions

- Phase 4 may start only after HUD, input, and minimap intent writes are routed through role-shaped
  facades from Phase 3.
- Phase 5 may start only after renderer feedback no longer depends on broad intent state.
- Phase 6 is cleanup-only. If removing shims reveals behavior changes, split that behavior work into
  a new phase instead of folding it into enforcement.

## Non-Goals

- Do not redesign HUD layout, renderer art, command-card semantics, or hotkeys.
- Do not add a framework, bundler, or generated JS build step.
- Do not split large files just to reduce line count; extract only around real boundaries.

## Handoff Rules

Each phase handoff must name remaining compatibility reads such as direct `state.commandTarget` or
`state.placement`, exact verification results, and the manual client flows most likely to regress.
