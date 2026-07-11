# Phase 13.5 - Foundation Evidence Gate

## Phase Status

- [ ] Not started.

## Depends On

- Phase 13 merged with the locked representative asset and measured integration evidence.

## Objective

Collect the complete command-backed foundation evidence and verify every content-expansion gate.
This phase does not make the product decision: it either passes all mandatory gates and marks done,
or returns `blocked` without changing status. The separate manual final review records
`go`, `revise`, or `stop` after the evidence PR merges and the plan archives.

## Work

- Run every stable scenario—`quiet`, `dense-placeholders`, `active-effects`, `fog-overlays`,
  `lifecycle`, `vegetation`, `vegetation-shadows`, and `representative-asset`—three times with the
  Phase 0 frozen metadata. Validate schemas, compare against owning baselines/budgets, and record
  report paths, SHA-256, summaries, and any timing warnings.
- Add/run a true same-page ten-cycle browser gate that constructs and destroys ordinary Babylon
  `Match` instances through `App`/network handler wiring. Every cycle includes asset load,
  entity/effect/fog/shadow/vegetation allocation, event capture, reset, resize, and rematch; exact
  registry, canvas, Match rAF, listener, context, pending-load, pool, shadow, and Lab-bridge counts
  return to baseline.
- Run the real two-recipient secrecy fixture, default Pixi absence check, live/replay/spectator/Lab
  route/role checks, all entity-kind catalog coverage, all target classifications, frozen event
  capture, resource late-load/double-release, counter reset, vegetation matrix, and stale-shadow
  clear commands.
- Audit `docs/design/client-rendering.md` and `docs/design/rendering-parity.md`. Every row has current
  status/evidence plus content-expansion versus default-cutover requirement; placeholder is not
  parity and no active fact depends on this plan directory.
- Evaluate the mandatory gate list below mechanically. Any missing/failing/stale evidence returns a
  structured `blocked` result, leaves this phase unchecked, and preserves the active plan for repair.
- If and only if every gate passes, update evidence hashes/summaries, mark the phase done, commit,
  and let the normal PR helper archive the plan. Do not write a candidate recommendation.

## Mandatory Content-Expansion Gates

- Shared consumers no longer depend on raw orthographic camera representation.
- Real perspective entity targeting, marquee, ground commands, minimap, listener, and framing pass.
- Default Pixi loads no Babylon code/bytes and Match is the sole rAF owner.
- Renderer frames/events/SelectionScene are least-privilege and two-recipient secrecy passes.
- Frozen short-event capture uses detached revisions/two clocks without extending TTL or stepping ticks.
- Coordinates, asset validation/fallback, and resource ownership contracts pass.
- Placeholders/effects/vegetation/shadows have bounded sharing/pooling/tier policies.
- Counters are per-frame, reports are comparable, and formula-based budgets are current.
- The generated representative GLB passes articulation/anchors/team/shadow/effect/resource/budget behavior.
- Ten true same-page lifecycle cycles return every exact owned count to baseline.
- Remaining work is explicit and classified for content expansion versus default cutover.

## Expected Touch Points

- benchmark all-scenario/comparison and ten-cycle lifecycle launchers
- `tests/browser_babylon_foundation.mjs` wired into the authoritative browser runner
- `tests/client_contracts/babylon_foundation_contracts.mjs` (create it in this phase)
- `docs/design/client-rendering.md`
- `docs/design/rendering-parity.md`
- `plans/render3d/phase-13.5.md` status update only after every gate passes

## Explicit Exclusions

- No new capability, asset, visual polish, budget relaxation, faction conversion, default/cohort
  rollout, Pixi retirement, deployment, or product recommendation.

## Implementation Checklist

- [ ] Run/hash/summarize all scenarios and comparisons.
- [ ] Pass the true same-page ten-cycle exact-count browser gate.
- [ ] Re-run all named security/interaction/capture/resource/performance/shadow checks.
- [ ] Audit durable contract/parity/budget evidence and gate classification.
- [ ] Mark done only if every mandatory gate passes; otherwise return blocked unchanged.

## Verification

    node scripts/validate-rendering-assets.mjs --all
    node scripts/rendering-benchmark.mjs --backend babylon --scenario all --repeat 3 --output target/rendering-benchmarks/phase-13.5.json
    node tests/browser_babylon_foundation.mjs --cycles 10
    node tests/client_contracts/babylon_foundation_contracts.mjs
    node tests/client_contracts/babylon_resource_contracts.mjs
    node tests/client_contracts/babylon_visibility_contracts.mjs
    node tests/client_contracts/babylon_interaction_contracts.mjs
    node tests/client_contracts/babylon_performance_contracts.mjs
    node tests/client_contracts/babylon_shadow_contracts.mjs
    node tests/client_contracts/presentation_capture_contracts.mjs
    node tests/browser_renderer_loading.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser
    git diff --check

GitHub's `Main test gate` remains the authoritative repository-wide suite. Required browser/backend
commands fail rather than skip when Chrome, WebGL, or Babylon readiness is unavailable.

## Manual Test Focus

No new visual implementation occurs. Review the final representative capture, report summaries,
ten-cycle counts, and parity rows for internal consistency; the separate post-merge final review
owns the product recommendation.

## Handoff Expectations

Lead with `evidence gate passed` or the exact structured blocker. Include all report paths/hashes/
summaries, comparison/budget results, ten-cycle counts, secrecy/role/capture/resource evidence,
durable ledger paths, and remaining classified risks. State that Babylon remains opt-in, Pixi
remains default, and the manual final review—not this executor—owns `go`, `revise`, or `stop`.
