# Capsule: testing

Use for tests, CI/hooks, or focused verification.

## Read first
- [docs/design/testing.md](../design/testing.md) §9 — self-play harness
  (only if touching scripted self-play)
- [docs/design/testing.md](../design/testing.md) §10 — dev scenario inspection
- [docs/design/testing.md](../design/testing.md) §11 — test selection policy
- [docs/design/testing.md](../design/testing.md) §12 — PR CI contract
- [docs/design/testing.md](../design/testing.md) §13 — doc drift sweeper

## Suites
- `node tests/server_integration.mjs` — full server pipeline; needs a running server.
- `node tests/regression.mjs` — hardening/DoS/robustness guards; needs a running server.
- `node tests/ai_integration.mjs` — dep-free AI opponent lobby flow; needs a running server.
- `node tests/lab_mortar_regression.mjs` — mortar event regression; needs a server.
- `node tests/minimap_input_contracts.mjs` — minimap/router pointer-lock contracts.
- `tests/run-all.sh --only-rust` — architecture policy plus Rust nextest and lint only.
- `tests/run-all.sh --only-rust-checks` — policy plus lint, without nextest.
- `RTS_NEXTEST_PARTITION=slice:1/2 tests/run-all.sh --only-nextest` — one CI partition.
- `tests/run-all.sh --only-live-node` — JS contracts plus live Node API suites only.
- `tests/run-all.sh --only-browser` — browser smoke plus configured tri-state browser suites only.
- `tests/run-all.sh --only-browser-scenarios=smoke,phase-0.5` — browser shard.
- `cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile
  default` — sim behavior plus fast scripted self-play; no server needed.
- `RTS_FULL_AI_TESTS=1 cargo nextest run --config-file .config/nextest.toml --manifest-path
  server/Cargo.toml --profile default` — long AI self-play/simulation coverage.
- `tests/run-all.sh --full-ai` — full orchestrator plus long AI coverage.
- `node tests/select-suites.mjs --from=<base-ref>` — suites.
- `node scripts/check-docs-health.mjs` — docs map, capsule cap, and Markdown links.
- `node scripts/check-wiki.mjs` — wiki route hardening, generated stats, and faction catalog
  parity.
- `node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10`.
- Lab Interact media contracts/smokes need FFmpeg/ffprobe with VP9 and H.264 encoders; recording
  smoke uses `record-wait` rather than polling.
- `node scripts/check-source-file-sizes.mjs` — enforce the 1500-line source cap.
- `node scripts/check-crate-boundaries.mjs` — enforce crate direction.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` —
  enforce `rts-sim::game` internal architecture ratchets.

## Invariants
- The required PR gate is `./tests/run-all.sh` in `Main test gate`. Split CI covers server build,
  Rust policy/lint plus complementary nextest partitions, live Node, and complementary browser
  smoke/tri-state shards; docs-only still runs cheap policy checks.
- `tests/run-all.sh` uses nextest; missing it fails locally with an install hint.
- Live Node suites need a running server. Use `tests/run-all.sh` to boot a private one, or start
  `cd server && cargo run` first for individual Node suites.
- Installed hooks run staged whitespace checks, excluding `playtest_notes.md`, plus docs health.
  They do not run `tests/run-all.sh`; GitHub Actions owns the full-suite gate.
- `scripts/agent-pr.sh` skips Codex only for pure `.md` diffs; otherwise it formats touched Rust
  with the pinned toolchain before the final push.
- Browser deps cache under `${RTS_NODE_DEPS_CACHE_DIR:-/tmp/rts-node-deps}`.
- Local `tests/run-all.sh` uses per-worktree Cargo target dirs under `/tmp/rts-cargo-target/`.
  Override with `CARGO_TARGET_DIR` only when a task needs a specific target location.
- Skip only when `tests/select-suites.mjs` maps changed files away from the suite.

## Self-play failure protocol
If a self-play test fails and the cause is not obvious, start a fresh server on its own port and
use macOS `open` to load the replay artifact:

```bash
open "http://localhost:<port>/?replayArtifact=<artifact_name>"
```

Do not use the Browser skill for this flow.

## Dev scenarios
Game-backed dev scenarios are live, no-fog watcher rooms for inspecting authored simulation
situations through the normal Pixi client. Start a local server, then open:

```bash
open "http://localhost:<port>/dev/scenarios"
```

The scenario index owns the current URL shapes and ids. Scenario setup remains server-side under
`server/crates/sim/src/game/setup/dev_scenarios.rs`; do not expose arbitrary spawning or map editing
through client commands.

## Cross-capsule triggers
- Touching client rendering, input, HUD, or lobby tests → [client-ui.md](client-ui.md).
- Touching message shapes or snapshot/event fields → [protocol.md](protocol.md).
- Touching sim behavior, AI, pathing, combat, or dev-scenario setup → [server-sim.md](server-sim.md).
- Touching deployment, hooks, or CI recovery guidance → [deployment.md](deployment.md) and
  [docs/pr-first-workflow.md](../pr-first-workflow.md).
