# Nextest Migration Plan

## Status

Draft.

## Purpose

Move Rust test execution wholesale from `cargo test` to `cargo-nextest`, locally and in CI. The goal
is a single Rust test path with clearer per-test timing, less CI cruft, and no hidden opt-in timing
wrappers that change the shape of the gate. This plan also cleans up old Rust timing diagnostics so
the remaining logs explain the required gate instead of offering stale side paths.

## Overall Constraints

- Do not add an opt-in canary workflow. Each phase should make the intended nextest behavior real in
  the normal path it touches.
- Keep the stable required PR check as the aggregate `./tests/run-all.sh` check from `Main test gate`.
- Keep coverage at least equivalent to today's required gate. If any Rust doctests exist, run them
  with `cargo test --doc` because nextest does not cover doctests through stable Cargo.
- Make nextest required locally. If nextest is missing, the local runner should fail with a clear
  install message instead of silently falling back to `cargo test`.
- Remove obsolete diagnostic paths once nextest supplies the replacement information. In particular,
  do not keep package-by-package Cargo timing as a normal documented workflow after the migration.
- Preserve the split CI shape for server binary, Rust/architecture, live Node, browser/tri-state, and
  aggregate result unless a phase proves a simpler shape keeps the same coverage and required check.
- Update the testing capsule, test README, and workflow documentation in the same phase as any
  user-visible command or CI contract change.
- Each phase must be implemented on its own `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, then waited through `scripts/wait-pr.sh` until GitHub reports it merged and the
  phase head is reachable from `origin/main`.
- After each phase, the implementing agent must provide a handoff message naming the next phase,
  focused verification, remaining risks, and the core features to manually test.

## Phase Summaries

### [Phase 1 - Local Nextest Runner](phase-1.md)

Phase 1 makes nextest the local Rust test runner used by `tests/run-all.sh` and direct Rust test
commands. It adds repo-owned nextest configuration, handles any doctest gap explicitly, and removes
the package-by-package timing wrapper from the normal documented path. The phase proves developers
get clear nextest output and per-test timings without changing non-Rust suites.

### [Phase 2 - CI Nextest Enforcement](phase-2.md)

Phase 2 moves the `Main test gate` Rust job to nextest and makes CI install nextest as part of the
required path. It keeps the aggregate `./tests/run-all.sh` check as the branch-protection signal and
removes any remaining redundant Rust or integration workflows if they still exist. The phase records
post-merge CI timing so the team can see the new Rust bottleneck honestly.

### [Phase 3 - Diagnostic Cleanup](phase-3.md)

Phase 3 cleans up old timing tools, stale docs, and confusing log labels now that nextest is the
Rust test source of truth. It keeps cheap command-level timing and nextest per-test reporting, but
removes opt-in package timing routes that made the gate slower and harder to explain. The phase
should leave CI logs short, structured, and useful for finding slow tests or cache misses.

### [Phase 4 - Final Ratchets and Documentation](phase-4.md)

Phase 4 adds guardrails so the repo does not drift back to `cargo test` for the normal gate. It
updates selector policy, architecture/testing docs, and any helper scripts that name Rust test
commands. The phase performs the final audit that nextest is enforced locally, in CI, and in docs
with one clear required workflow.

## Handoff Rules

Every phase handoff must include:

- the implementation PR number, head SHA, and merge confirmation;
- the exact Rust command path after the phase;
- focused verification commands and results;
- any nextest failures or doctest findings;
- `Main test gate` run ids when the phase changes CI;
- timing evidence for the Rust job when available;
- the next phase to run, or a stop reason;
- core manual checks for a developer running tests locally and reviewing the GitHub Actions page.

Do not report this plan complete until Phase 4 confirms there is no normal local or CI Rust gate
still using plain `cargo test` except an explicit doctest-only command if doctests exist.
