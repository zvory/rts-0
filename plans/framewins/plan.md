# Frame Wins Plan

## Purpose

Remove three measured sources of redundant client-frame work while preserving every frame's state,
timing semantics, and pixels. The phases cover retained HP/status geometry, shared entity preparation,
and row-dirty world-fog geometry in that order. Each phase is an independent measured decision: an
implementation that does not earn its architectural and code complexity is reverted and completed as
a documented no-go rather than merged as speculative production machinery.

## Planning Evidence

The required clean-worktree `node scripts/client-flamegraph.mjs --preview` capture on planning base
`80ec0837` used only `supply-300-hellhole-stream`. Its ordinary harness summary reported
`frame.work` 11.6 ms average / 16 ms p95, `match.renderer` 9.0 ms average,
`renderer.update` 4.9 ms average, `renderer.present` 4.1 ms average,
`match.presentationFrame` 1.3 ms average, `match.frameEntityViews` 0.4 ms average,
`match.selectionScene` 0.4 ms average, and `renderer.fogDraw` 0.3 ms average. The ranked profile
reported presentation `detach` at 5.9% self time, garbage collection at 4.9%,
`entitiesInterpolated` at 1.9%, selection `detachPlainRecord` at 1.5%, `entityRecord` at 1.4%, and
renderer `_drawFog` at 1.1%; diagnostics also reported 133.6 HP-bar clears per frame.

These values justify implementation experiments but are not acceptance baselines for a later
executor. Each phase must recapture current `origin/main` immediately before editing and compare it
with that phase's candidate on the same local machine and browser configuration.

## Overall Constraints

- Preserve the full 30 Hz snapshot stream and Match-owned per-RAF presentation. Do not reduce the
  cadence of entity reconciliation, interpolation, selection, HP/progress, fog, animation, effects,
  HUD, minimap, or overlays.
- Preserve one current logical frame per RAF, one synchronous present, and the same render clock.
  Do not present stale state, stagger entities across frames, move work outside measured
  `frame.work`, or hide scheduling cost.
- Keep `supply-300-hellhole-stream` unchanged as the sole supply-scale client renderer benchmark.
  Do not special-case its route, workload id, fixture contents, browser, camera, or timing.
- Preserve viewport, DPR, visual fidelity, layer order, blend behavior, interpolation, and every
  visible pixel. Do not substitute lower-resolution textures, reduced effects, or benchmark-camera
  culling for redundant-work removal.
- Preserve the detached frozen `PresentationFrameV1` boundary, last-successful-present
  `SelectionSceneV1` semantics, Pixi compatibility adapter seam, authoritative fog rules, and
  renderer failure isolation.
- Do not change the wire protocol, simulation, balance, snapshot cadence, snapshot-stream asset, or
  fog authority in this plan.
- Keep mutable caches and staging private to their existing owner. Every cache key must enumerate
  the exact draw/data inputs, commit only after successful work, retry after failure, reset on map or
  generation change, and release resources during normal destroy/rematch teardown.
- Profiles are attribution evidence, not scorecards. Use repeated unprofiled `frame.work` and named
  phase measurements for the go/no-go decision; inclusive profile percentages overlap and must not
  be added.
- Keep raw profiles, PNGs, diff images, and benchmark summaries under ignored `target/` paths. The
  handoff records their paths, commands, browser identity, base/candidate commits, run order, and
  rejection reasons without committing machine-local timing artifacts.

## Shared Before/After Measurement Protocol

Every phase uses a pristine base worktree at the phase's current `origin/main` and a separate
candidate worktree. Install the lockfile dependencies with `npm ci` when needed, record the exact
Chrome executable/version, and keep viewport `1440x900`, DPR 1, CPU throttle 1, duration 15 seconds,
ready assets, workload identity, and other machine load as stable as practical.

Before source edits, capture and inspect all three current artifacts together:

```bash
node scripts/client-flamegraph.mjs --preview
node scripts/client-perf-harness.mjs \
  --workload supply-300-hellhole-stream \
  --seconds 15 \
  --viewport 1440x900 \
  --dpr 1 \
  --cpu-throttle 1 \
  --output-root target/client-perf/framewins/<phase>/before
```

Inspect the flamegraph PNG, ranked JSON, ordinary `summary.json`, frame-budget summary, target phase,
diagnostic counters, and hottest source functions before implementing. After implementation and
focused correctness checks, run a candidate flamegraph and the same unprofiled command under an
`after` output root. Preserve the base worktree and alternate base/candidate runs in an ABBA-style
order until each revision has five valid unprofiled samples, unless the phase document specifies a
stronger paired protocol. Do not compare one lucky run from each revision.

For each revision, report medians for `frame.work` average/p95/max, `match.renderer`,
`renderer.update`, `renderer.present`, the phase-specific target timings, RAF dispatch/frame gaps,
and relevant diagnostics. Reject samples with runtime/page/request errors, changed workload shape,
changed viewport/DPR/browser, missing frames, focus/visibility problems, or materially different
host-load evidence. Keep profiled timings separate from the unprofiled medians and use the
after-profile only to confirm that the intended hotspot shrank rather than moved outside the
measured frame.

## Shared Exact-Parity Gate

Phase 1 must first add a reusable test/tooling driver named
`scripts/client-render-parity.mjs`; the repository does not currently have a client-stream capture
that pins exact frame indices. The driver must accept base and candidate worktrees, workload id,
seed or explicit tick file, sample count, viewport, DPR, alpha, fixed visual timestamp, browser, and
output root. It starts isolated local servers, loads the ordinary snapshot-stream client, stops its
timer, restarts from frame zero, delivers the normal encoded frames through `SnapshotStreamNet` and
the production decoder in order, enters existing fixed capture, waits for required assets, and
captures the same sorted frame indices from each revision.

The command contract established by Phase 1 and reused by later phases is:

```bash
node scripts/client-render-parity.mjs \
  --baseline-worktree <base-worktree> \
  --candidate-worktree <candidate-worktree> \
  --workload supply-300-hellhole-stream \
  --seed <phase-seed> \
  --samples 16 \
  --viewport 1440x900 \
  --dpr 1 \
  --alpha 1 \
  --output-root target/client-perf/framewins/<phase>/parity
```

The driver decodes PNGs to RGBA, compares bytes, writes a bounded JSON summary plus base/candidate/
diff artifacts under the ignored output root, and exits nonzero on a changed pixel, tick/state
mismatch, missing asset, page/render error, or capture-input mismatch. It must also accept an
explicit deterministic tick file so Phase 3 can choose fog-transition ticks. Add focused CLI/
selection/failure contracts and document the exact dependency used for PNG decoding.

Before each runtime change, use that driver to select at least 16 unique stream ticks with a
deterministic PRNG seed and record the seed and tick list in the handoff. Base and candidate must use
identical decoded snapshot state, camera, viewport, DPR, browser, interpolation alpha, visual
timestamp, and ready assets; decoded RGBA must be byte-identical. Keep the driver in test/tooling
ownership, make it work across arbitrary revisions, and do not add a production route, runtime
special case, new exposed app/state handle, or work to the measured frame.

## Complexity Decision Rule

A phase merges runtime changes only when all functional, lifecycle, exact-pixel, and repeated
performance gates pass. The handoff must include a concise complexity inventory: added persistent
state, invalidation inputs, failure/reset/destroy paths, new helpers/files, net source-line change,
and why the measured player-frame win pays for them. Counter reduction or a narrower subphase win is
not sufficient when the phase's end-to-end threshold is missed.

If a candidate misses its threshold, needs a generalized cache/framework, crosses its assigned
ownership boundary, or cannot prove exact pixels, revert the runtime experiment. Mark the phase
done with a no-go result, retain only independently valuable test/tooling changes that do not add
production complexity, and hand the evidence to the next phase without silently weakening the
gate. Because raw timing artifacts are ignored, commit the no-go medians, threshold comparison, and
concise rationale into that phase document before marking it Done.

## Phase Summaries

### [Phase 1 - Retained Status Geometry](phase-1.md)

Move HP, construction, and deconstruction bars into entity-local retained Graphics geometry while
updating their containers on every frame. Rebuild only when exact output inputs change and preserve
pooling, failure retry, layer order, and teardown. Keep the implementation only if repeated local
measurements show a material end-to-end renderer win as well as a large drop in HP geometry clears.

### [Phase 2 - Shared Entity Preparation](phase-2.md)

Produce interpolated, current, and authoritative entity variants in one state traversal and remove
duplicate deep detachment between presentation and selection. Preserve independent variant
semantics, the frozen renderer boundary, and selection state from the last successful present.
Keep the implementation only if repeated measurements materially reduce presentation/view work,
allocation/GC pressure, and total frame work without expanding cross-area coupling.

### [Phase 3 - Row-Dirty World Fog](phase-3.md)

Replace whole-grid world-fog retessellation with renderer-owned row state that rebuilds only exact
changed rows on every real fog revision. Preserve authoritative visible/explored semantics, map and
generation resets, run ordering, failure retry, and every-frame fog freshness. Keep the additional
row objects and invalidation state only if repeated measurements show a worthwhile renderer and
frame-work improvement with exact fog-transition pixels.

## Phase Index

1. [Phase 1 - Retained Status Geometry](phase-1.md)
2. [Phase 2 - Shared Entity Preparation](phase-2.md)
3. [Phase 3 - Row-Dirty World Fog](phase-3.md)

## Implementation and Handoff Process

Implement one phase at a time from fresh `origin/main` on its own `zvorygin/` branch. Mark that phase
document Done in its implementation commit, push it as an owned PR with auto-merge armed, run
`scripts/wait-pr.sh <pr>`, and verify the phase head is reachable from `origin/main` before starting
the next phase. GitHub's `Main test gate` remains the authoritative full suite.

After every phase, provide a handoff describing the implementation or no-go decision, contracts and
invalidation rules, focused checks, parity seed/ticks/results, before/after artifact paths and
medians, complexity inventory, remaining uncertainty, what the next agent should do, and the core
features a human should manually test. When the final phase is marked Done,
`scripts/agent-pr.sh` archives this plan under `plans/archive/framewins/` in the final phase PR.

Suggested unattended execution after review:

```bash
scripts/phase-runner.sh --plan framewins phase-1 phase-2 phase-3 --pr --wait
```

## Deferred Backlog

- Rig sampling/transform optimization, minimap primitive preparation, decal/trench texture tiling,
  diagnostic batching, pool-sweep changes, generic Pixi batch reordering, and culling remain outside
  this plan.
- Do not broaden a phase into those items because a profile shows adjacent cost. Re-profile after
  all accepted phases before creating another implementation plan.
