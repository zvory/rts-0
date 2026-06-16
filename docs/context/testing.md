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
- `cd server && cargo test` — simulation behavior + fast scripted self-play (no running server needed)
- `cd server && RTS_FULL_AI_TESTS=1 cargo test` — includes long AI self-play/simulation coverage
- `tests/run-all.sh --full-ai` — local gate plus long AI self-play/simulation coverage
- `node tests/select-suites.mjs --from=<base-ref>` — list expected suites for changed files
- `node scripts/check-crate-boundaries.mjs` — enforce crate dependency direction
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` —
  enforce `rts-sim::game` internal architecture ratchets

## Invariants
- Local gate scripts use a per-worktree Cargo target dir under `/tmp/rts-cargo-target/`, so
  parallel worktrees do not share final binaries, test harnesses, or self-play artifacts. Override
  with `CARGO_TARGET_DIR` if a task needs a specific target location.
- Installed hooks run `scripts/cleanup-worktrees.sh --auto` after commits and merges on `main` to
  remove clean merged `zvorygin/*` worktrees and amortize stale Cargo target cleanup.
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
- After any change, run all relevant Node suites + `cargo test` and confirm green. Use
  `RTS_FULL_AI_TESTS=1 cargo test` when touching AI strategy, profile-backed self-play, replay
  determinism, or balance behavior that depends on long AI matches. The commit hook silently runs
  the full local gate; don't rely on it as your only check for changes that need `--full-ai`.
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
