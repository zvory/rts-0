# Phase 3 - Diagnostic Cleanup

Status: Done.

## Goal

Replace the old pile of Rust timing diagnostics with one clear nextest-centered story. The normal
logs should show command-level timing, cache context, and nextest per-test timing without asking
developers to know about stale opt-in profiling modes.

## Scope

- Remove stale references to package-by-package Cargo timing from docs, help text, and scripts.
- Delete retired scripts that are no longer part of the supported workflow.
- Rename timing rows so they describe nextest accurately.
- Keep cheap timing around major commands: architecture checks, format, nextest, clippy, doctests if
  present, server build, live Node, browser.
- Add lightweight CI context that helps explain Rust time: nextest version, Rust version, Cargo
  target directory, and cache-hit evidence already printed by Actions.
- Keep successful logs concise. Detailed per-test output should come from nextest or a structured
  artifact, not custom shell spam.
- Update docs so there is one recommended way to answer "why is Rust slow?"

## Out Of Scope

- Do not add another package-by-package profiler.
- Do not add sccache or another compiler cache.
- Do not split Rust jobs here.
- Do not tune individual slow tests unless a cleanup change exposes a trivial typo or stale command.

## Expected Touch Points

- `tests/run-all.sh`
- `tests/README.md`
- `docs/context/testing.md`
- `docs/pr-first-workflow.md`
- `.github/workflows/main-tests.yml`
- retired timing scripts, if any remain
- `plans/nextest/phase-3.md`

## Implementation Checklist

- [x] Remove old package-timing docs and environment variables.
- [x] Remove or archive obsolete timing scripts.
- [x] Make the Rust timing summary say nextest by name.
- [x] Add version and target-dir context where it is useful.
- [x] Ensure failure output remains visible and successful output stays concise.
- [x] Update docs with the new slow-Rust investigation workflow.
- [x] Mark this phase done in the implementation commit.

## Focused Verification

- `bash -n tests/run-all.sh`
- `tests/run-all.sh --only-rust`
- `node tests/select-suites.mjs --verify` if selector docs or policy are touched
- `git diff --check`

## Manual Test Focus

Review a successful local Rust-only run and a failed single-test run if easy to force. Confirm the
logs say what command ran, how long it took, and where to look next without mentioning retired
profilers.

## Handoff Expectations

The handoff must list every removed diagnostic route, the remaining supported timing information,
and any Rust timing questions still not answerable from the new logs.
