# Phase 5 - Cutover, Docs, and Canary

## Status

Draft.

## Objective

Make the Rust runner the maintained phase-runner implementation while preserving the existing
operator command path and documenting the new extension points.

## Scope

- Replace `scripts/phase-runner.sh` with a small compatibility wrapper that runs the Rust binary.
- Decide whether the wrapper should use `cargo run -p rts-phaserunner --` or a checked-in
  lightweight launcher pattern that keeps normal use simple.
- Update `plans/README.md`, `docs/context/planning.md`, `CLAUDE.md`, README workflow snippets, and
  any references found by `rg "phase-runner"`.
- Keep `scripts/phase-runner-result.schema.json` available or update references if the Rust crate
  becomes the schema source of truth.
- Add a short developer note in the plan or docs that identifies the intended follow-up extension
  points: prompt section injection, experimental local iteration mode, and repair/resume
  inspection.
- Run the rollout canary with the new default `scripts/phase-runner.sh` entrypoint on a tiny
  docs-only phased plan.
- Keep rollback simple: the wrapper/cutover commit should be easy to revert without touching
  unrelated workflow helpers.

## Expected Touch Points

- `scripts/phase-runner.sh`
- `server/crates/phaserunner/`
- `plans/README.md`
- `docs/context/planning.md`
- `CLAUDE.md`
- `README.md`
- `plans/phaserunner/phase-5.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-phaserunner`
- `scripts/phase-runner.sh --help`
- `scripts/phase-runner.sh --plan <fixture> phase-1 --pr --dry-run`
- One docs-only canary through `scripts/phase-runner.sh --plan <fixture> <phase> --pr --wait`
- `rg "phase-runner" README.md CLAUDE.md docs plans scripts`
- `git diff --check`

## Manual Testing Focus

Inspect the canary PR and logs from the operator's point of view. Confirm the command name,
handoff path, log path, PR body, auto-merge state, wait output, and final success message remain
clear.

## Handoff Expectations

Report the canary PR, the exact commands run, any rollback notes, and which follow-up capability is
now lowest risk to implement first. Mark the plan complete only after the canary PR merged and the
head is reachable from `origin/main`.
