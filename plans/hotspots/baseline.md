# Baseline Hotspot Dataset

Generated on 2026-06-21 from `origin/main` at
`63a7de9749e3c77e7adf100fec0f7e1188aa6b50`.

This pass is triage evidence only. It ranks current source files by size, rename-aware history,
recent churn, fix-looking commit subjects, current-line freshness, and short-window co-change
pressure. It does not prove that a file should be split; it identifies files and architectural
groups that deserve the Phase 2 responsibility-map read.

Machine-readable evidence is in
[`plans/hotspots/evidence/baseline-hotspots.json`](evidence/baseline-hotspots.json).

## Scope and Filters

- Current files came from `server/`, `client/`, `tests/`, and `scripts/`.
- Included extensions: `.rs`, `.js`, `.mjs`, `.sh`, `.css`, and `.html`.
- Excluded generated or bulky path fragments: `target/`, `node_modules/`, `dist/`, `build/`,
  `coverage/`, `tmp/`, `replay/`, `replays/`, and `artifacts/`.
- The recent window is 14 days, starting on 2026-06-07.
- The pass found 370 current source files, 162,765 current non-empty LOC, 5,805 rename-aware
  current-file commit touches, 284,262 rename-aware added-plus-deleted lines, 646 recent commits
  touching current source files, and 121 source-file rename events.

## Scoring

The hotspot score is a damped blend, not a verdict:

```text
100 * (
  0.22 * sqrt(non_empty_loc / max)
  + 0.23 * sqrt(rename_aware_touches / max)
  + 0.22 * sqrt(total_churn / max)
  + 0.13 * sqrt(recent_churn / max)
  + 0.10 * sqrt(fix_like_touches / max)
  + 0.10 * sqrt(recent_cochange_degree / max)
)
```

The square-root damping keeps very large files from completely hiding smaller files that are
frequently co-changed. The score should be used to choose what to read first, not to make a cleanup
decision by itself.

## Top Current File Hotspots

| Rank | File | Score | Non-empty LOC | Touches | Churn | Recent churn | Fix-like | Co-change degree |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | `tests/client_contracts.mjs` | 95.43 | 9,028 | 279 | 12,353 | 10,721 | 35 | 220 |
| 2 | `server/src/lobby/room_task.rs` | 90.75 | 7,664 | 167 | 16,838 | 13,503 | 26 | 181 |
| 3 | `server/crates/sim/src/game/services/commands.rs` | 73.56 | 5,208 | 117 | 10,580 | 7,831 | 12 | 169 |
| 4 | `server/crates/sim/src/game/tests.rs` | 63.98 | 5,124 | 83 | 7,359 | 5,659 | 3 | 177 |
| 5 | `server/crates/ai/src/selfplay/tests.rs` | 63.25 | 1,775 | 123 | 12,195 | 1,567 | 16 | 152 |
| 6 | `server/crates/protocol/src/lib.rs` | 61.52 | 3,209 | 133 | 5,162 | 3,765 | 6 | 192 |
| 7 | `client/styles.css` | 57.25 | 3,573 | 101 | 5,494 | 3,694 | 6 | 81 |
| 8 | `server/crates/sim/src/game/services/movement/tests.rs` | 55.52 | 4,803 | 51 | 6,784 | 563 | 9 | 139 |
| 9 | `server/src/main.rs` | 54.15 | 1,954 | 108 | 3,531 | 2,390 | 9 | 178 |
| 10 | `client/src/match.js` | 52.05 | 1,184 | 115 | 4,316 | 2,426 | 6 | 160 |
| 11 | `server/crates/sim/src/game/services/combat/tests.rs` | 50.03 | 3,389 | 54 | 4,247 | 2,157 | 1 | 159 |
| 12 | `server/crates/ai/src/ai_core/decision/tests.rs` | 48.31 | 3,819 | 33 | 4,834 | 1,974 | 2 | 106 |
| 13 | `server/crates/sim/src/game/setup.rs` | 47.66 | 748 | 62 | 4,721 | 3,386 | 4 | 176 |
| 14 | `server/crates/sim/src/game/mod.rs` | 47.59 | 447 | 111 | 4,693 | 374 | 12 | 161 |
| 15 | `client/src/hud.js` | 47.40 | 921 | 80 | 3,809 | 2,427 | 9 | 108 |
| 16 | `client/src/protocol.js` | 45.64 | 1,198 | 106 | 1,805 | 1,103 | 4 | 184 |
| 17 | `server/crates/sim/src/game/services/move_coordinator.rs` | 44.57 | 1,898 | 58 | 3,185 | 477 | 8 | 114 |
| 18 | `client/src/state.js` | 43.40 | 1,031 | 73 | 2,242 | 1,423 | 5 | 147 |
| 19 | `server/crates/sim/src/game/services/pathing.rs` | 42.42 | 1,826 | 39 | 3,503 | 396 | 7 | 114 |
| 20 | `client/src/config.js` | 41.58 | 498 | 126 | 1,250 | 800 | 6 | 129 |

The initial scout remains directionally correct: `tests/client_contracts.mjs`,
`server/src/lobby/room_task.rs`, and `server/crates/sim/src/game/services/commands.rs` are still
the first three current-file hotspots after filtering and rename-aware history.

## Architectural Groups

| Group | Files | Non-empty LOC | Touches | Churn | Recent churn | Top ranked files |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `sim-core` | 41 | 13,698 | 805 | 37,015 | 18,198 | `server/crates/sim/src/game/setup.rs`, `server/crates/sim/src/game/mod.rs`, `server/crates/sim/src/game/systems.rs` |
| `ai` | 36 | 17,233 | 567 | 33,656 | 9,958 | `server/crates/ai/src/selfplay/tests.rs`, `server/crates/ai/src/ai_core/decision/tests.rs`, `server/crates/ai/src/ai_core/actions.rs` |
| `client-ui` | 38 | 15,564 | 469 | 26,009 | 17,984 | `client/styles.css`, `client/src/minimap.js`, `client/src/lobby.js` |
| `server-lobby-runtime` | 17 | 13,075 | 333 | 24,196 | 20,056 | `server/src/lobby/room_task.rs`, `server/src/lobby/mod.rs`, `server/src/lobby/session_policy.rs` |
| `protocol-and-contracts` | 5 | 13,912 | 624 | 23,189 | 18,061 | `tests/client_contracts.mjs`, `server/crates/protocol/src/lib.rs`, `client/src/protocol.js` |
| `sim-services` | 20 | 12,901 | 397 | 19,578 | 6,622 | `server/crates/sim/src/game/services/move_coordinator.rs`, `server/crates/sim/src/game/services/pathing.rs`, `server/crates/sim/src/game/services/order_queue.rs` |
| `server-backend` | 17 | 9,375 | 240 | 12,201 | 9,905 | `server/src/main.rs`, `server/src/dev_scenarios.rs`, `server/crates/sim-wasm/src/lib.rs` |
| `scripts-tooling` | 27 | 8,734 | 138 | 12,040 | 12,020 | `scripts/check-client-architecture.mjs`, `scripts/client-perf-harness.mjs`, `scripts/docdrift-sweep.mjs` |
| `sim-command-service` | 3 | 5,842 | 138 | 11,403 | 8,235 | `server/crates/sim/src/game/services/commands.rs`, `server/crates/sim/src/game/command.rs`, `server/crates/sim/src/game/commands.rs` |
| `sim-movement-service` | 8 | 7,652 | 138 | 11,222 | 1,314 | `server/crates/sim/src/game/services/movement/tests.rs`, `server/crates/sim/src/game/services/movement/waypoints.rs`, `server/crates/sim/src/game/services/movement/scout_car.rs` |

The group view changes the reading order. `protocol-and-contracts` is only five files but has very
high touches and recent churn, while `sim-core` and `ai` are broad groups whose churn may reflect
active feature movement rather than one extractable responsibility.

## Recent Coupling

Top 14-day co-change pairs:

| Pair | Commits | Groups |
| --- | ---: | --- |
| `client/src/protocol.js` + `server/crates/protocol/src/lib.rs` | 62 | protocol mirror |
| `client/src/match.js` + `tests/client_contracts.mjs` | 54 | client match shell + contracts |
| `server/src/lobby/room_task.rs` + `server/src/main.rs` | 50 | lobby runtime + server backend |
| `server/crates/protocol/src/lib.rs` + `tests/client_contracts.mjs` | 49 | protocol + contracts |
| `server/src/lobby/mod.rs` + `server/src/lobby/room_task.rs` | 49 | lobby runtime |
| `scripts/check-client-architecture.mjs` + `tests/client_contracts.mjs` | 43 | tooling + contracts |
| `client/src/protocol.js` + `tests/client_contracts.mjs` | 40 | protocol + contracts |
| `client/src/hud.js` + `tests/client_contracts.mjs` | 39 | HUD + contracts |
| `server/crates/protocol/src/lib.rs` + `server/src/lobby/room_task.rs` | 37 | protocol + lobby runtime |
| `client/src/config.js` + `tests/client_contracts.mjs` | 36 | balance/config mirror + contracts |

The highest-degree recent hubs are also the main Phase 2 candidates:
`tests/client_contracts.mjs`, `server/crates/protocol/src/lib.rs`, `client/src/protocol.js`,
`server/src/lobby/room_task.rs`, `server/src/main.rs`,
`server/crates/sim/src/game/tests.rs`, and
`server/crates/sim/src/game/services/commands.rs`.

## Blame Freshness

`git blame -w -M -C -C` on the top 20 shows that the highest-ranked files are genuinely current,
not just old monoliths:

| File | Median line age | Lines <= 14 days | Lines <= 30 days |
| --- | ---: | ---: | ---: |
| `tests/client_contracts.mjs` | 8 days | 85.7% | 100% |
| `server/src/lobby/room_task.rs` | 8 days | 85.5% | 100% |
| `server/crates/sim/src/game/services/commands.rs` | 13 days | 67.9% | 100% |
| `server/crates/sim/src/game/tests.rs` | 10 days | 67.2% | 100% |
| `server/crates/ai/src/selfplay/tests.rs` | 16 days | 37.2% | 100% |
| `server/crates/protocol/src/lib.rs` | 8 days | 77.6% | 100% |
| `client/styles.css` | 11 days | 60.4% | 100% |
| `server/crates/sim/src/game/services/movement/tests.rs` | 16 days | 5.8% | 100% |
| `server/src/main.rs` | 12 days | 64.7% | 100% |
| `client/src/match.js` | 10 days | 63.5% | 100% |

This makes `server/crates/sim/src/game/services/movement/tests.rs` a different kind of hotspot:
large and historically churned, but less freshly rewritten than the top contract, lobby, command,
and protocol files.

## Raw Path Comparison

Raw path-level churn saw 532 source-looking paths, including 162 stale or removed paths. That is
why cleanup prioritization should not use raw churn alone.

Top stale raw-path false positives:

| Stale path | Raw churn | Raw touches | Current replacement hint |
| --- | ---: | ---: | --- |
| `server/src/game/ai_core/decision.rs` | 11,854 | 34 | AI code moved under `server/crates/ai/src/ai_core/decision/` |
| `server/src/game/selfplay.rs` | 11,847 | 75 | Self-play code moved under `server/crates/ai/src/selfplay/` |
| `server/src/game/services/movement/tests.rs` | 11,164 | 32 | Current path is `server/crates/sim/src/game/services/movement/tests.rs` |
| `server/src/game/services/movement.rs` | 8,712 | 47 | Movement service was split under `server/crates/sim/src/game/services/movement/` |
| `server/src/game/services/combat.rs` | 6,136 | 50 | Combat service was split under `server/crates/sim/src/game/services/combat/` |
| `server/src/game/services/commands.rs` | 5,752 | 54 | Current path is `server/crates/sim/src/game/services/commands.rs` |
| `client/src/renderer.js` | 5,277 | 58 | Renderer was split under `client/src/renderer/` |

Recent rename events also explain why source groups are more useful than individual historical
paths. The evidence includes source renames such as the AI crate extraction on 2026-06-07, tool
moves on 2026-06-08, `client/src/input/command_composer.js` moving to
`client/src/command_composer.js` on 2026-06-11, and the replay analysis overlay becoming the
observer analysis overlay on 2026-06-13.

## Phase 2 Guidance

Start responsibility maps with these current files:

1. `tests/client_contracts.mjs`
2. `server/src/lobby/room_task.rs`
3. `server/crates/sim/src/game/services/commands.rs`
4. `server/crates/protocol/src/lib.rs`
5. `client/src/protocol.js`
6. `client/styles.css`
7. `client/src/match.js`
8. `client/src/hud.js`
9. `client/src/config.js`
10. `server/crates/rules/src/balance.rs`

Defer actual edits to `server/src/lobby/room_task.rs` unless the active room-runtime cleanup work is
explicitly folded into a later cleanup plan. Treat aggregate test files
(`tests/client_contracts.mjs`, `server/crates/sim/src/game/tests.rs`, AI self-play tests, movement
tests, and combat tests) as possible test-suite organization problems before assuming runtime
coupling. Treat `server/crates/protocol/src/lib.rs`, `client/src/protocol.js`, `client/src/config.js`,
and `server/crates/rules/src/balance.rs` as mirrored contract surfaces; any future extraction plan
needs explicit protocol or balance-mirror safeguards.

## Commands Used

The evidence was generated by a temporary Node.js one-shot that was not committed; Phase 4 owns the
repeatable script or runbook. The script used these Git plumbing commands:

```bash
git rev-parse origin/main
git ls-tree -r --name-only origin/main -- server client tests scripts
fd -t f -e rs -e js -e mjs -e sh -e css -e html . server client tests scripts
git log origin/main --no-merges --follow --find-renames=40% --numstat --format='__COMMIT__%x09%H%x09%cs%x09%s' -- <current-file>
git log origin/main --no-merges --since=2026-06-07 --name-only --format='__COMMIT__%x09%H%x09%cs%x09%s' -- server client tests scripts
git blame origin/main -w -M -C -C --line-porcelain -- <top-hotspot-file>
git log origin/main --no-merges --no-renames --numstat --format='__COMMIT__%x09%H%x09%cs%x09%s' -- server client tests scripts
git log origin/main --no-merges --find-renames=40% --name-status --format='__COMMIT__%x09%H%x09%cs%x09%s' -- server client tests scripts
```

Manual inspection checked the top current rows, stale raw-path examples, high-degree coupling hubs,
and recent rename events for obvious false positives.
