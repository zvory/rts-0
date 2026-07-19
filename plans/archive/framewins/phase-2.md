# Phase 2 - Shared Entity Preparation

## Phase Status

- [ ] Not started.

## Objective

Collapse the three per-frame entity variant traversals into one `GameState` batch operation, then
prepare each interpolated entity once as shared frame-local immutable input for presentation and
selection. Preserve the renderer's admitted-field boundary, the full detached selection
interaction, prediction/interpolation values, and last-successful-frame selection semantics. The
preparation sidecar, source references, admitted metadata, and unpublished failed-frame data are
frame-local; only the detached interaction may outlive its producing frame, and only through the
existing last-successful `SelectionSceneV1` lifetime. Use two internal checkpoints so the simpler
traversal win can be measured separately from the more complex shared-detachment work.

## Current Evidence and Scope Gate

The planning baseline spent 1.3 ms/frame in `match.presentationFrame`, 0.4 ms in
`match.frameEntityViews`, and 0.4 ms in `match.selectionScene`, for 2.1 ms/frame in the targeted
group. It recorded three `entitiesInterpolated()` calls per frame. The profile reported 5.9% self
time in presentation `detach`, 1.9% in `entitiesInterpolated`, 1.5% in selection
`detachPlainRecord`, 1.4% in `entityRecord`, and 4.9% in garbage collection.

Before editing, rerun the shared current-main baseline and inspect these exact paths. Defer shared
detachment if the hotspot has moved or the combined group is too small to clear this phase's
absolute acceptance threshold.

## Checkpoint A - Batched Entity Variants

- Add one production `GameState` method that creates:
  - render-alpha entities with prediction/display overlays
  - alpha-1 current entities with prediction/display overlays
  - alpha-1 authoritative entities without prediction/display overlays
  in one traversal of `_cur.entities`.
- Preserve entity order, missing-prior behavior, x/y interpolation arithmetic, facing and
  weapon-facing angle interpolation, prediction corrections, progress extrapolation, optimistic
  production/rally overlays, shot-reveal/vision-only records, and variant isolation.
- Preserve current visual-clock sampling semantics and ordering. Do not replace current values with
  a stale timestamp, reduce prediction cadence, or alias records that callers are allowed to treat
  independently.
- Let `buildFrameEntityViews()` use the production batch method and retain its existing
  `entitiesInterpolated()` fallback for test doubles. Keep `selectedEntities()` behavior and order
  unchanged.
- Replace the old three-call diagnostic with truthful production counters such as one state-view
  build and one entity traversal; keep fallback call counts truthful.
- Run characterization, parity, and a checkpoint benchmark before starting Checkpoint B. Retain
  this portion alone only if it clears the fallback threshold in the final stop criteria.

## Checkpoint B - Shared Frame-Local Immutable Preparation

- Add one focused helper, preferably `client/src/presentation/entity_snapshot.js`, that owns the
  single admitted presentation-field schema currently in `presentation/frame.js`.
- For each interpolated entity, perform one plain-data clone/freeze for the complete selection
  interaction while producing an aligned admitted presentation sidecar during the same walk. Reuse
  already-certified frozen nested values when building the final presentation record.
- Keep selection interaction and presentation admission separate:
  - Selection retains the complete detached interaction shape expected today.
  - Presentation continues to expose only admitted fields plus derived relationship, team color,
    selected state, visual bounds, and anchors.
  - Unsupported, cyclic, typed-array, or non-finite data in an admitted subtree takes the existing
    bounded presentation-record drop path.
  - Unsupported data in a field not admitted to presentation must not broaden or incorrectly drop
    the renderer record.
- Return prepared entries aligned with `interpolatedEntities` from the frame-local context.
  `frame_recovery.js` injects them into both `PresentationFrameAssembler` and
  `buildSelectionScene`; no consumer performs an id lookup that can reorder duplicates or records.
- Standalone assembler/selection callers, remembered buildings, and test doubles retain the legacy
  detachment fallback. Do not force a broad API migration merely to reach the fast path.
- Do not share a `PresentationFrameV1` entity object as `SelectionProxy.interaction`: the renderer
  whitelist is narrower than the interaction record.
- Do not retain a cross-frame WeakMap, entity cache, or invalidation graph. Preparation sidecars and
  unpublished failed-frame data become unreachable after the frame; the detached interaction may
  remain reachable only through the successfully published selection scene until a later successful
  present replaces it.
- Update `docs/design/client-ui.md` and `docs/design/client-rendering.md` if the prepared sidecar or
  batched variants become a documented cross-module contract.
- Mark this phase Done in this file in the implementation or measured-no-go commit.

## Expected Touch Points

- `client/src/state.js`
- `client/src/frame_entity_views.js`
- `client/src/frame_recovery.js`
- `client/src/presentation/frame.js`
- one focused preparation helper under `client/src/presentation/`
- `client/src/input/selection_projection.js`
- `client/src/frame_profiler.js` only for bounded once-per-frame diagnostics
- `scripts/client-perf-harness.mjs` and `scripts/client-perf/browser_profile.mjs` for the opt-in
  allocation-sampling mode described below
- focused client-performance harness contracts for the new allocation artifact
- `tests/client_contracts/frame_entity_contracts.mjs`
- `tests/client_contracts/presentation_frame_contracts.mjs`
- `tests/client_contracts/selection_projection_contracts.mjs`
- `tests/client_contracts/pixi_presentation_adapter_contracts.mjs`
- `docs/design/client-ui.md` and `docs/design/client-rendering.md` when contracts change

No protocol, server, simulation, fixture, renderer-backend API version, or client architecture
import exception belongs in this phase.

## Characterization and Focused Tests

Before refactoring, build a legacy-output oracle that covers:

- alpha 0, fractional alpha, and alpha 1
- missing prior entity and stable entity order
- facing/weapon-facing wraparound
- owned predicted movement and prediction correction
- build, deconstruction, production, and progress extrapolation
- optimistic production and rally overlays
- resources, shot reveals, vision-only records, spectator/player fog-source filtering
- independently mutable source records and nested metadata

Then prove:

- Batched and legacy variant arrays are deeply equal for the oracle, with independent records where
  current callers require isolation.
- Production performs one entity traversal; generic test-double fallback reports its real calls.
- Ordinary acyclic prepared and legacy `PresentationFrameV1` entity layers and `SelectionSceneV1`
  proxies/interactions serialize identically.
- Source mutation after preparation cannot change presentation or selection data; all required
  ordinary records and arrays remain frozen.
- Hidden/unadmitted fields cannot reach the renderer. Admitted versus unadmitted nested NaN,
  typed-array, unsupported-prototype, depth, and cycle cases preserve current bounded-drop behavior.
- Cyclic selection interactions use a graph-aware structural comparison that verifies own keys,
  values, frozen state, object identity, and back-edges; do not JSON-serialize cycles or silently
  change the selection clone's current cycle-preserving behavior.
- Projection footprints, proxy order, click/hover/marquee/double-click/control-group behavior, and
  entity-target commands remain unchanged.
- A renderer update/present failure keeps the previously published selection scene and does not
  retain or publish the failed frame's prepared interaction.
- The two-recipient secrecy test still proves that presentation and selection cannot reconstruct a
  never-received entity.

Run at least:

```bash
node tests/client_contracts.mjs
node tests/prediction_controller.mjs
node tests/progress_extrapolator.mjs
node tests/babylon_two_recipient_contract.mjs
node tests/minimap_input_contracts.mjs
node scripts/check-client-architecture.mjs
node tests/select-suites.mjs --verify
node scripts/check-docs-health.mjs
git diff --check
```

Run the browser/client-smoke suites selected by `node tests/select-suites.mjs --from=<phase-base>`.
GitHub's `Main test gate` remains authoritative.

## Before/After Performance and Allocation Gate

Follow the shared canonical five-sample ABBA protocol for the parent and final candidate. Also
capture an identical unprofiled checkpoint after batched variants and before shared detachment, so
the final handoff can distinguish their savings; if the measured effect is smaller than baseline
spread, expand the parent/checkpoint/final comparison to nine samples without changing commits.

Primary timing metrics are:

- per-frame combined `match.frameEntityViews + match.presentationFrame + match.selectionScene`
- each of those three phases separately
- `frame.work` average/p95/max and rendered throughput, defined for every sample as
  `perf.summary.frameCount / (run.durationMs / 1000)` rather than a late instantaneous FPS field
- `renderer.update` and `renderer.present` regression guards
- profile self time for interpolation, `entityRecord`, presentation detach, selection detach, and GC

Before runtime edits, add an opt-in `--heap-profile-sampling-bytes <n>` mode to the existing client
performance harness and its browser-profile helper. When enabled, use CDP HeapProfiler allocation
sampling plus before/after forced-GC heap usage over the same post-warmup measurement window and
write a bounded `heap-profile-summary.json`; when absent, the flag must add no page/runtime work and
must not change the ordinary summary. Apply the identical tooling-only commit to both base and
candidate measurement worktrees, then run:

```bash
node scripts/client-perf-harness.mjs \
  --workload supply-300-hellhole-stream \
  --seconds 15 \
  --viewport 1440x900 \
  --dpr 1 \
  --cpu-throttle 1 \
  --heap-profile-sampling-bytes 32768 \
  --output-root target/client-perf/framewins/phase-2/<revision>-alloc
```

Run allocation sampling separately from CPU profiles and unprofiled timing runs. Add only batched
once-per-frame diagnostics for entity traversals, detached plain object/array counts, and reused
nested values; do not add per-object dynamic profiler labels to production. Treat sampled bytes,
retained heap, and GC profile share as supporting directional evidence; timing remains the merge
gate.

The full candidate may merge only when medians show all of:

- at least 20% and at least 0.35 ms/frame reduction in the combined target phases
- at least 3% lower `frame.work` average
- at least 25% fewer controlled clone object/array creations per frame
- no sampled-allocation or retained-heap regression beyond baseline spread; a reduction corroborates
  the clone-count and timing result but has no independent percentage gate
- no GC self-share worsening beyond baseline spread; a reduction corroborates the result
- no actual-throughput regression and no renderer update/present or frame-work p95 regression beyond
  baseline spread

## Exact Data and Pixel Parity

Use the shared seeded 16-tick exact-RGBA gate. At every tick, also serialize and compare normalized
`PresentationFrameV1` entity layers and `SelectionSceneV1` proxies/interactions, including fixed
projection-query results. Pixel equality alone cannot detect an interaction-record or hit-testing
regression. Use the graph-aware structural comparator instead of serialization for the explicit
cyclic-interaction fixtures.

## Local Gameplay Test Focus

Use the project-local `interact` workflow with an ordinary authoritative release match. Exercise
movement/facing interpolation, predicted movement and corrections, build/production progress,
click/hover/marquee/double-click/control groups, allied/spectator/Lab inspection, entity-target and
right-click commands, fog sources, replay/fixed capture, and renderer-failure retry. Confirm that
selection always corresponds to the last successfully presented frame.

## Complexity Stop Criteria

The preferred ceiling is one focused preparation helper, no duplicated entity schema list, no
cross-frame cache, no new cross-area import exception, and no renderer/protocol version change. If
exact behavior requires a general object-graph framework, retained WeakMap/cache, or roughly more
than 250 net production lines beyond tests/docs, stop and revert shared detachment.

If the full candidate misses its gate, retain Checkpoint A alone only when it independently saves at
least 0.15 ms/frame and 10% of `match.frameEntityViews`, passes every parity/contract test, and is
smaller than the reverted shared-preparation design. If neither checkpoint clears its gate, complete
the phase as measured not worth the complexity and proceed to Phase 3 without production changes.

## PR and Handoff Requirements

- Implement on a fresh `zvorygin/` branch after Phase 1 is merged and reachable from
  `origin/main`, regardless of whether Phase 1 was accepted or a no-go.
- Run `scripts/agent-pr.sh --verification "<focused checks, data/pixel parity, allocation evidence, and repeated before/after gate passed or measured no-go documented>"`, then `scripts/wait-pr.sh <pr>` and verify reachability.
- The handoff must report parent/checkpoint/final artifact paths and medians/spread, allocation and
  GC deltas, traversal/clone counts, parity seed/ticks/hashes, normalized frame/selection diffs,
  complexity inventory, manual prediction/selection checks, and exactly what Phase 3 inherits.
