# 300-Supply Client Frame-Budget Measurement Foundation

## Plan Status

Concluded. Phase 0 was dispensed and Phase 1 established the reusable workload and frame-measurement
foundation. The former optimization specifications in Phase 2 and Phase 3 were retired after a
steady-state V8 CPU profile showed that function-level evidence should determine a fresh plan's
scope and ordering.

This directory is historical measurement context, not an executable optimization plan. The normal
owned-PR workflow archives it when the retirement lands so a future agent can create a new
`plans/framebudget/` from current profiling evidence.

## Phase Summaries

### [Phase 0 - Build the 300-Supply Lab Hellhole](phase-0.md)

Phase 0 was dispensed instead of combining server and client saturation into one workload. The
existing Lab scenario remains the server-heavy simulation/projection lane, while the generated
snapshot stream is the repeatable client-only renderer lane. Keeping the lanes separate prevents
server variance from obscuring client optimization comparisons.

### [Phase 1 - Own and Measure the Complete Frame](phase-1.md)

Phase 1 made `Match` the sole match RAF owner and placed one explicit Pixi update/present inside the
measured frame. It added stable phase, budget, diagnostic, and active-player 200/300-supply evidence.
Those contracts and workloads remain the foundation for future profiling and comparisons.

### [Phase 2 - Retired Pending Specification](phase-2.md)

The old Phase 2 bundled rig construction, animation sampling, frame-entity derivation, selection
copying, and adapter cleanup from coarse phase timing. A CPU flame graph confirmed some of those
areas but also exposed different relative costs and diagnostic overhead. The implementation spec
was therefore removed instead of being treated as approved work.

### [Phase 3 - Retired Pending Specification](phase-3.md)

The old Phase 3 bundled fog, minimap, HP/selection, and trench caching before function-level cost
was available. Fresh profiling identified fog as important but did not justify carrying every
bundled cache task or its previous order forward. The implementation spec was therefore removed and
must be recreated only where current evidence supports it.

## Replacement Planning Gate

Before creating replacement implementation phases:

1. Start from a clean task worktree on current `origin/main`.
2. Run `node scripts/client-flamegraph.mjs --preview` for the canonical deterministic client-only
   renderer baseline.
3. Inspect the Tailnet PNG, ranked function JSON, ordinary harness `summary.json`, and the hottest
   source functions. Do not infer implementation scope from flame width or coarse phase labels
   alone.
4. Profile `supply-300-active` as well when a conclusion depends on prediction, active-player fog,
   or production-cap safety.
5. Create a fresh `plans/framebudget/` with no more than three executable phases before the next
   measured checkpoint. Order work by current function-level evidence, architectural dependency,
   and reversibility rather than by the retired phase numbers.

## Preserved Constraints

- Do not raise the production supply cap or change balance as part of client performance work.
- Preserve server-authoritative fog, the detached `PresentationFrameV1` boundary, last-successful-
  present selection semantics, one Match-owned RAF, and exactly one explicit present per frame.
- Whole-map zoom remains the representative worst case; viewport culling is not benchmark success.
- Keep the generated Hellhole snapshot stream unchanged during before/after comparisons.
- Treat Chrome CPU throttling and local profiles as comparative machine-local evidence, not device
  certification.
- Keep raw profiles, flame graphs, screenshots, and benchmark output under ignored `target/` paths.
- After each future phase, provide a handoff naming changed contracts, focused validation, retained
  before/after evidence, the next action, and core manual tests. Each phase must use an owned PR,
  arm auto-merge, wait for merge, and verify its head is reachable from `origin/main`.

## Deferred Until Fresh Planning

- Route-specific rig construction and animation sampling changes.
- Fog recomputation and geometry caching.
- Frame-entity, presentation, and selection-copy consolidation.
- Diagnostic-counter sampling or aggregation changes.
- Minimap, HP/selection, trench, Pixi, allocation, or garbage-collection optimizations.
- Remote playtester CPU-profile collection through DevTools, an extension, or a launcher.
