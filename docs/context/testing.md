# Capsule: testing

Use when writing or debugging tests, or before claiming a change is done.

## Read first
- [docs/design/testing.md](../design/testing.md) — API-driven self-play test harness
  (only if touching scripted self-play)

## Suites
- `node tests/server_integration.mjs` — dep-free, full server pipeline
- `node tests/regression.mjs` — dep-free, hardening/DoS/robustness guards
- `node tests/ai_integration.mjs` — dep-free, AI opponent lobby flow
- `node tests/minimap_input_contracts.mjs` — dep-free minimap/router pointer-lock input contracts
- `tests/run-all.sh --no-rust` — live Node suites plus headless-Chrome client smoke; hydrates
  `puppeteer-core` through the shared lockfile-keyed cache
- `tests/run-all.sh --with-tri-state-browser --no-rust` — opt into latency-sensitive browser
  tri-state lag scenarios locally; CI includes them automatically
- `tests/run-all.sh --only-rust` — architecture policy plus Rust format, nextest, and lint only
- `tests/run-all.sh --only-live-node` — JS contracts plus live Node API suites only
- `tests/run-all.sh --only-browser` — browser smoke plus configured tri-state browser suites only
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile
  default` — simulation behavior + fast scripted self-play (no running server needed)
- `RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml --manifest-path
  server/Cargo.toml --profile default` —
  includes long AI self-play/simulation coverage
- `tests/run-all.sh --full-ai` — full orchestrator plus long AI self-play/simulation coverage
- `node tests/select-suites.mjs --from=<base-ref>` — list expected suites for changed files
- `node scripts/check-wiki.mjs` — wiki route hardening, internal links, generated stats table
  completeness, and faction catalog parity
- `node scripts/check-crate-boundaries.mjs` — enforce crate dependency direction
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` —
  enforce `rts-sim::game` internal architecture ratchets

## Invariants
- The stable required PR gate is the aggregate `./tests/run-all.sh` check from the `Main test gate`
  workflow. It depends on split jobs for server binary build, Rust/architecture, live Node, and
  browser/tri-state coverage on pull requests targeting `main` and on pushes to `main`; Markdown-only
  PRs keep the same green check context and skip the long suites after changed-file detection.
- `tests/run-all.sh` prints a timing summary for every measured suite, server build/boot, and
  client dependency hydration attempt. Its default Rust test phase is
  `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile
  default`. Missing nextest is a local gate failure with an install hint; the runner does not fall
  back to `cargo test`.
- The workspace currently has no Rust doctests, so the local gate does not run a separate
  `cargo test --doc` step. Add one beside nextest if doctests are introduced.
- `CI / server binary`, `CI / rust and architecture`, `CI / live Node suites`, and
  `CI / browser and tri-state` are the auditable coverage jobs under the aggregate required check.
  The Rust/architecture job installs `cargo-nextest` and runs `./tests/run-all.sh --only-rust`, so
  CI uses the same nextest-backed Rust command path as local development.
  Branch protection should require the aggregate `./tests/run-all.sh` check unless rulesets are
  deliberately migrated to require every split coverage job directly. The split jobs also
  short-circuit as green checks for Markdown-only PRs.
- The old standalone `Rust` and `Integration` workflows were retired because the split
  `Main test gate` jobs own that Rust, architecture, live Node, and browser coverage under the
  required aggregate check.
- Workflow concurrency cancels superseded runs for the same PR branch, and cancels stale
  post-merge `main` push runs when a newer `main` push starts. Unrelated branches should not
  cancel each other.
- Beta deploys are triggered only from successful `Main test gate` workflow runs whose original
  event was a push to `main`; PR-head workflow runs and canceled stale `main` runs must not deploy.
- Local `tests/run-all.sh` uses a per-worktree Cargo target dir under `/tmp/rts-cargo-target/`, so
  parallel worktrees do not share final binaries, test harnesses, or self-play artifacts. Override
  with `CARGO_TARGET_DIR` if a task needs a specific target location. The `Main test gate`
  workflow sets `CARGO_TARGET_DIR` to `server/target` for Rust-building jobs so GitHub Actions can
  restore and save the same target directory that Cargo uses. Live Node and browser jobs download
  the debug server binary from the server-build job and pass it with `RTS_SERVER_BIN` to avoid
  rebuilding the server in each split job.
- Installed hooks run cheap staged-diff checks before commits and merges. They do not run
  `tests/run-all.sh` by default; GitHub Actions owns the full-suite gate for normal PR work.
- Installed hooks run `scripts/cleanup-worktrees.sh --auto` after commits and merges on `main` to
  remove clean merged `zvorygin/*` worktrees and amortize stale Cargo target cleanup. Cleanup treats
  local `main` and `origin/main` as proof refs and does not require the merged PR branch to still
  exist on `origin`.
- Browser smoke dependencies are shared across worktrees under
  `${RTS_NODE_DEPS_CACHE_DIR:-/tmp/rts-node-deps}` and keyed by the SHA-256 hash of
  `tests/package-lock.json`. `tests/run-all.sh` links each worktree's ignored
  `tests/node_modules` to the matching cache entry and runs `npm ci` only when that cache is
  missing.
- `sccache` is not enabled automatically. It was tested as a cross-worktree Rust reuse layer, but
  Rust cache hits stayed at 0% across different per-worktree target dirs because target-dir-specific
  rustc arguments changed the cache keys. It only produced Rust hits when rebuilding the exact same
  target directory path.
- Node tests need a **running** server on the test runner's private port. They are not
  `cargo test`. Start the server first.
- `tests/run-all.sh` boots its private server with `RTS_TEST_TICK_MS=5` by default so live-server
  suites advance simulated time quickly without changing normal `cargo run` pacing.
- `node tests/team_integration.mjs` is the canonical live multi-client team suite. It covers
  singleton FFA, solo, `1v2`, `1v3`, `2v2`, shared team snapshots, malicious lobby/team/combat
  inputs, and team victory. `tests/run-all.sh --no-rust` includes it in the live Node API pass.
- After any change, run the focused local suites that match the changed files or contracts. Use
  `RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml --manifest-path
  server/Cargo.toml --profile default` when touching AI strategy, profile-backed self-play, replay
  determinism, or balance behavior that depends on long AI matches. The PR full gate is required
  for merge; do not rely on cheap local hooks as test coverage.
- For CI failure recovery, workflow canaries, and moving the full gate to another runner, see
  [docs/pr-first-workflow.md](../pr-first-workflow.md).
- A suite can be skipped only when `tests/select-suites.mjs` maps the changed files away from that
  behavior and both architecture checks still pass:
  `scripts/check-crate-boundaries.mjs` and `rts-archcheck check-sim-architecture`.

## Self-play failure protocol
If a self-play test fails and the cause is not immediately obvious, **do not** speculate-debug.
Start a fresh server on its own port and use macOS `open` to load a spectation replay so the
user can inspect the failure state:

```
open "http://localhost:<port>/dev/selfplay?replay=<artifact_name>"
# e.g.
open "http://localhost:<port>/dev/selfplay?replay=manual_worker_rush_latest"
```

Do **not** use the Browser skill for this flow.

## Dev scenarios
Game-backed dev scenarios are live, no-fog watcher rooms for inspecting authored simulation
situations through the normal Pixi client. Start a local server, then open the index:

```
open "http://localhost:<port>/dev/scenarios"
```

The index lists every supported launch and links to the current URL shape:

```
/dev/scenarios?id=<scenario_id>&unit=<unit>&count=<count>[&blocker=<unit|none>]
```

The handler redirects into the normal client with `watchScenario=1`; the client auto-joins a
reserved spectator room named:

```
__dev_scenario__:<scenario_id>:unit=<unit>:count=<count>[:blocker=<unit|none>]
```

Current scenario ids:

- `scout_car_snaking_corridor` — movement/pathing through the snaking stone corridor.
- `direct_reverse_order` — one vehicle ordered directly behind its current facing.
- `scout_car_wall_chokepoint` — vehicle groups moving through a narrow wall gap.
- `vehicle_corner_wall` — vehicle groups cornering around a wall spur.
- `vehicle_small_block_baseline` — vehicles moving through optional small-unit blockers.
- `factory_zero_gap_perpendicular` — one vehicle starting flush against a factory and moving east.

The watcher shows movement debug path overlays by default. Replay speed controls are reused for
dev scenarios: `Pause` sets the simulation speed to zero, and `Step` advances exactly one
authoritative tick while paused. Normal seek/reset controls are replay-only.

Scenario setup is server-side only under `server/crates/sim/src/game/setup/dev_scenarios.rs`; do
not expose arbitrary spawning or map editing through client commands. Scenario artifact recording
under `target/scenario-artifacts/` is not currently implemented.

## Gotchas
- A 1-player match is a never-ending sandbox; only 2+ player matches resolve to a winner.
- Empty rooms reset to lobby so a room name is never stuck mid-match.
