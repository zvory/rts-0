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
- `cd tests && npm install && node client_smoke.mjs` — headless-Chrome client smoke
- `cd server && cargo test` — simulation behavior + fast scripted self-play (no running server needed)
- `cd server && RTS_FULL_AI_TESTS=1 cargo test` — includes long AI self-play/simulation coverage
- `tests/run-all.sh --full-ai` — local gate plus long AI self-play/simulation coverage

## Invariants
- Repo-level Cargo config uses `/tmp/rts-cargo-target/rts-0-server` as the shared target dir, so
  plain Cargo commands in worktrees reuse dependency builds. Override with `CARGO_TARGET_DIR` if a
  task needs an isolated cache.
- Node tests need a **running** server on the test runner's private port. They are not
  `cargo test`. Start the server first.
- After any change, run all relevant Node suites + `cargo test` and confirm green. Use
  `RTS_FULL_AI_TESTS=1 cargo test` when touching AI strategy, profile-backed self-play, replay
  determinism, or balance behavior that depends on long AI matches. The commit hook silently runs
  the fast gate; don't rely on it as your only check.

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

## Gotchas
- A 1-player match is a never-ending sandbox; only 2+ player matches resolve to a winner.
- Empty rooms reset to lobby so a room name is never stuck mid-match.
