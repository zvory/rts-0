# Phase 1 - Behavior Model and Crate Scaffold

## Status

Done.

## Objective

Create the Rust project shape and encode the current runner behavior that can be tested without
executing Codex or touching GitHub.

## Scope

- Add a new Rust workspace crate for the phase runner with a library and thin binary.
- Add typed models for phase ids, phase selection, runner config, handoff JSON, PR state, timing
  JSON, and completion marker checks.
- Implement phase id normalization and ordering for numeric, decimal, and suffixed ids.
- Implement `--from/--to` discovery against a supplied plan directory, excluding the lower bound and
  including the upper bound.
- Implement completion marker detection for the current accepted forms:
  `Status: Done.`, `## Status` plus `Done.`, and `## Phase Status` plus `- [x] Done.`.
- Implement handoff JSON parsing and validation against the current required fields.
- Implement PR readiness decisions from parsed `gh pr list` JSON without calling `gh`.
- Keep `scripts/phase-runner.sh` untouched as the active runner.

## Expected Touch Points

- `server/Cargo.toml`
- `server/crates/phaserunner/Cargo.toml`
- `server/crates/phaserunner/src/lib.rs`
- `server/crates/phaserunner/src/main.rs`
- `server/crates/phaserunner/src/*.rs` for focused modules
- `server/Cargo.lock`
- `plans/phaserunner/phase-1.md`

## Verification

- Focused Rust unit tests for phase parsing, discovery ordering, invalid phase ids, completion
  marker detection, handoff parsing, and PR readiness outcomes.
- `cargo test --manifest-path server/Cargo.toml -p rts-phaserunner`
- `cargo run --manifest-path server/Cargo.toml -p rts-phaserunner -- --help`
- `git diff --check`

## Manual Testing Focus

No live phase execution in this phase. Manually compare the tested phase id examples and completion
marker examples against `scripts/phase-runner.sh` before moving on.

## Handoff Expectations

Name the crate, module boundaries, and any behavior that was intentionally deferred to later
phases. Include the exact focused test command and any compatibility gaps discovered while encoding
the shell behavior.
