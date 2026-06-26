# Phase 2 - Run-All Characterization and Suite Model

Status: Not started.

## Goal

Capture the behavior of `tests/run-all.sh` before replacing it. This phase should create a
testable Rust model for the suite graph, command modes, environment variables, skip rules, timing
records, and server lifecycle decisions while leaving the shell runner as the active
implementation.

## Scope

- Inventory the current `tests/run-all.sh` options, environment variables, generated environment,
  suite names, background-vs-serial execution order, skip messages, and failure summary behavior.
- Model the suite plan in Rust data structures that can be inspected in unit tests.
- Add fake-runner tests for argument parsing, mode selection, server-needed decisions, skip
  recording, and failure aggregation.
- Add fixture coverage for per-worktree Cargo target directory selection, `RTS_SERVER_BIN` reuse,
  `PORT` handling, `RTS_RUN_TRI_STATE_BROWSER`, `RTS_RUN_WASM_TRI_STATE`, `RTS_FULL_AI_TESTS`, and
  Chrome-required browser decisions.
- Preserve current shell behavior; compatibility wrappers should still call `tests/run-all.sh` for
  actual execution.

## Expected Touch Points

- `server/crates/devtool/src/run_all.rs`
- `server/crates/devtool/src/process.rs` or equivalent runner abstraction
- `server/crates/devtool/tests/` or Rust unit-test modules
- `tests/README.md` only if the characterization exposes a documented contract that is missing

## Implementation Notes

Do not try to run the whole suite from Rust in this phase. The useful output is a tested internal
plan that says which suites would run, whether a server is needed, which suites are parallelizable,
and what environment each command receives. If the current shell behavior is ambiguous, document
the observed behavior in the phase handoff before encoding a guess.

The model should explicitly include the current operator-facing details that make `run-all` useful:
timing summary rows, skipped-suite explanations, nextest install hint, Rust tool context, server
build/boot records, client dependency hydration, and browser scenario groups. Later phases can
reuse this model as the source of truth for orchestration.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-devtool run_all`
- `./tool run-all --help`
- `tests/run-all.sh --help`
- `node scripts/check-docs-health.mjs`

## Manual Testing Focus

Compare the modeled help/options against `tests/run-all.sh --help`. Spot-check that common modes
such as `--only-rust`, `--only-live-node`, `--only-browser`, `--no-rust`, and `--full-ai` map to
the suites developers expect.

## Handoff Expectations

List every `run-all` option or environment variable that is now covered by tests and call out any
option that remains shell-only or ambiguous. The next agent should know exactly which subset is
safe to execute from Rust in Phase 3.
