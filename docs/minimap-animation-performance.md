# Minimap animation Hellhole comparison — 2026-10-29

The completed production client shows firing recoil and machine-gunner setup/teardown on the
minimap. The requested historical comparison measures the combined cost of PR #1712's articulated
Tank/Anti-Tank Gun minimap fidelity and this animation change, plus intervening main changes.
It does not isolate the incremental cost of this PR alone.

## Revisions and method

- Before: `f28b4a465` (first parent of the merge of PR #1712, `84e6e8019`). Clean detached worktree.
- After: `d4d8f9bd6` on `zvorygin/minimap-recoil`, based on `913c0ac18`, including the
  reviewed first-use animation preload fix.
- Canonical `fixed-roster-hellhole-stream`; identical checked-in stream, harness, terrain,
  camera, viewport 1440×900, DPR 1, Chrome 156, CPU throttle 1, default terrain shadow mode.
- Three alternating before/after runs, 15 measured seconds each after harness warmup. No
  concurrent Interact/browser workload. CPU profiling disabled for these headline runs.
- Uncapped completion-paced production frame path (`RTS_CLIENT_PERF_UNCAPPED=1`), pipeline depth
  two, authored 30 Hz stream. These are asynchronous WebGL capacity measurements, not gameplay
  display FPS or GPU-complete throughput. No presentation cadence or fidelity was reduced.
- All six runs passed: after draining, submitted = dispatched = completed; no failed, superseded,
  stale, pending, or in-flight frames. Ordinary browser/console failure checks also passed.

## Results

| Revision | Run | Presentations/s | Frame work avg ms | Frame work p95 ms | Minimap avg ms | Minimap p95 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| before | 1 | 134.2 | 2.7 | 4 | 0.4 | 1 |
| before | 2 | 132.53 | 2.7 | 4 | 0.4 | 1 |
| before | 3 | 132.73 | 2.8 | 4 | 0.5 | 1 |
| after | 1 | 120.8 | 3.7 | 8 | 1.4 | 2 |
| after | 2 | 119.67 | 3.7 | 8 | 1.4 | 2 |
| after | 3 | 119.07 | 3.8 | 8 | 1.4 | 2 |

Median completed throughput: **132.73 → 119.67/s (−9.84%)**. Median frame work average:
**2.7 → 3.7 ms (+1.0 ms)**. Minimap average: **0.4 → 1.4 ms (+1.0 ms)**.
Frame-work p95 moved from the **4 ms to 8 ms histogram bucket**, and minimap p95 from **1 to
2 ms**. Histogram percentiles are coarse bucket estimates, not exact latency quantiles.
The after runs miss the repository's 4.17 ms p95 headroom proxy; both revisions retain substantial
headroom against a 16.67 ms frame-work budget. Throughput and CPU work are separate measurements.

The minimap becomes the most frequent largest main-thread phase in the after runs. It now samples
production poses and draws separately transformed cached atlas sprites, so this is an expected,
measurable cost increase that should be reviewed before merge.

## Profiles, visual checks, and limitations

A separate 15-second profiled uncapped after run passed the harness and produced valid main/worker
flamegraph PNGs and ranked JSON. Main-thread self samples included Canvas restore (3.3%), drawImage
(3.0%), and minimap pose layers (1.3%); frame cloning and worker diagnostics remained larger overall
contributors. The PNG and ranked summary were inspected together. This profile preceded only the
full-frame strip crop/loadout correction; the repeated headline runs include the final correction.
Historical/final profile attempts intermittently hit Chrome negative CPU-profile time deltas during
worker flamegraph conversion, after successful harness measurement. No trace samples were repaired
or used to replace the repeated headline measurements.

A browser-rendered storyboard checks packed/half-setup/deployed/firing Machine Gunners and resting/
firing Tanks and AT Guns. Decoded canvas pixels differ for each intended setup/recoil transition,
with no page errors; full strip frames retain the deployed head and barrel. An authoritative
Interact Lab scene also passed capture readiness. Exact parity with the old icons is not expected:
these animation and articulated silhouette differences are the intended presentation changes.

Final headline run JSON is under `/tmp/minimap-perf/final/`; profiles and earlier development
runs are under `/tmp/minimap-perf/`. the rendered storyboard is
`/tmp/minimap-perf/animation-storyboard.png`. These artifacts are not committed. Reproduce each
headline run from the corresponding checkout:

```sh
scripts/ensure-node-deps.sh
RTS_CLIENT_PERF_UNCAPPED=1 node scripts/client-perf-harness.mjs \
  --workload fixed-roster-hellhole-stream --seconds 15 \
  --viewport 1440x900 --dpr 1 --output-root /tmp/minimap-perf/reproduction
```

Focused checks passed for minimap inputs, production rig runtime, frame-strip and mortar setup
contracts, protocol, client architecture, docs health, and all 12 minimap export/icon tests. Browser
smoke fails the starting-steel assertion (`49` versus `>=50`) on both this branch and the untouched
`913c0ac18` main client served by the same server binary. The clean-main comparison also showed
prediction/progress timing failures; the branch rerun showed only the steel assertion. These smoke
failures are recorded without changing unrelated economy or prediction behavior.

The PR intentionally remains unmerged with auto-merge disabled for performance review.
