# Phase 2 - CI Nextest Enforcement

Status: Done.

## Goal

Make GitHub Actions use nextest for the required Rust path. The required aggregate check should stay
named `./tests/run-all.sh`, but the Rust job underneath it should install nextest and run the same
nextest-backed command developers run locally.

## Scope

- Update `Main test gate` so `CI / rust and architecture` installs nextest.
- Run `./tests/run-all.sh --only-rust` in CI after Phase 1 has made that command nextest-backed.
- Preserve the existing Cargo registry and target cache behavior unless the implementation proves it
  conflicts with nextest.
- Upload nextest machine-readable output only if it is useful and not noisy. Prefer one JUnit or
  structured artifact over ad hoc log spam.
- Remove `.github/workflows/rust.yml` and `.github/workflows/integration.yml` in this phase if the
  separate cleanup branch has not already removed them.
- Keep beta deploy behavior tied to successful `Main test gate` push runs on `main`.
- Update docs that name required checks or CI workflow ownership.

## Out Of Scope

- Do not split Rust into more CI jobs in this phase.
- Do not add a separate nextest canary workflow.
- Do not weaken docs-only PR skipping.
- Do not change branch protection manually unless the PR handoff clearly says a human must update
  required checks.

## Expected Touch Points

- `.github/workflows/main-tests.yml`
- `.github/workflows/rust.yml`
- `.github/workflows/integration.yml`
- `docs/context/testing.md`
- `tests/README.md`
- `docs/pr-first-workflow.md`
- `plans/nextest/phase-2.md`

## Implementation Checklist

- [x] Install nextest in the Rust CI job.
- [x] Confirm the Rust CI job uses the local nextest-backed runner, not a separate command.
- [x] Preserve the aggregate `./tests/run-all.sh` job and its dependency list.
- [x] Remove redundant Rust/Integration workflows if still present.
- [x] Ensure docs-only PRs still get a green aggregate check without running long suites.
- [x] Document that the outer runner must record the first post-merge `Main test gate` run id and
  Rust job duration.
- [x] Mark this phase done in the implementation commit.

## Implementation Notes

- `CI / rust and architecture` installs `cargo-nextest` and invokes
  `./tests/run-all.sh --only-rust`.
- The retired `.github/workflows/rust.yml` and `.github/workflows/integration.yml` workflows were
  already absent in this phase worktree.
- Post-merge `Main test gate` timing must be collected by the outer runner after the phase PR
  merges; this executor cannot open or wait on a PR.

## Focused Verification

- YAML syntax review for changed workflows
- `bash -n tests/run-all.sh`
- `tests/run-all.sh --only-rust` locally if feasible
- `git diff --check`
- Post-merge: inspect `gh run view <run-id>` for `Main test gate`

## Manual Test Focus

Open the GitHub Actions run and confirm a human can see that Rust tests are nextest-backed. Confirm
the checks page still presents the required aggregate check clearly.

## Handoff Expectations

The handoff must include the CI run id, Rust job timing, whether redundant workflows were removed or
already absent, and whether any required-check settings need human attention.
