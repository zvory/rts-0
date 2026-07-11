# Phase 3 - Run-All Core Rust Orchestrator

Status: Not started.

## Goal

Implement the first real Rust-backed `run-all` execution path for the non-browser core. This phase
should prove process supervision, server build/reuse, server boot/cleanup, parallel suite
execution, failure aggregation, and timing summaries without taking on browser dependency
hydration or Windows-specific behavior yet.

## Scope

- Implement `./tool run-all` execution for `--only-rust` and `--only-live-node` once parity is
  proven.
- Preserve per-worktree Cargo target directory behavior, including `CARGO_TARGET_DIR` overrides.
- Build the debug server unless `RTS_SERVER_BIN` points to an executable prebuilt binary.
- Boot the server on a private or requested port for live Node suites, poll health, and stop only
  a server started by the tool.
- Run Rust and architecture suites with the current command list and nextest install hint.
- Run static JavaScript contract suites and live Node API suites with the expected `RTS_WS`,
  `RTS_URL`, and `RTS_MATCH_SEED` environment.
- Record failures without stopping unrelated parallel suites early, then print a final summary
  compatible with the current operator workflow.
- Keep unsupported modes on the existing shell path until Phase 4.

## Expected Touch Points

- `server/crates/devtool/src/run_all.rs`
- `server/crates/devtool/src/process.rs`
- `server/crates/devtool/src/server.rs`
- `server/crates/devtool/src/timing.rs`
- `tests/run-all.sh` only if routing selected migrated modes through `./tool`
- `tests/README.md` for any temporary "Rust-backed modes" note

## Implementation Notes

The Rust process runner should avoid shell invocation by default and pass arguments as arrays. Use
captured logs for quiet mode and inherited output only where the current script intentionally shows
operator output. Child cleanup must be robust on Unix now and designed so Phase 5 can add Windows
process-tree cleanup without changing suite logic.

Do not silently change the current suite order. The current runner starts background checks,
collects them around server/browser sequencing, serializes latency-sensitive browser suites later,
and always prints final timings; preserve those semantics for the migrated subset.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool run_all`
- `./tool run-all --only-rust --help` or equivalent help check
- `./tool run-all --only-live-node` with `RTS_SERVER_BIN` pointing at a known local server binary,
  when available
- `tests/run-all.sh --only-rust` if the wrapper routes that mode through Rust
- `node scripts/check-docs-health.mjs`

## Manual Testing Focus

Run at least one migrated mode through `./tool` and confirm failure output is actionable when a
suite is forced to fail. Confirm that a pre-existing server on the selected port is reused and left
running, while a server started by the tool is stopped on exit.

## Handoff Expectations

Name the modes that now run through Rust and the modes still falling back to shell. Include timing
or output differences that the Phase 4 agent must decide whether to preserve, fix, or document.
