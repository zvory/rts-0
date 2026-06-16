# Multi-phase planning capsule

Use this capsule whenever a task asks for a multi-phase plan, phased plan, implementation sequence,
or similar staged handoff. Phased plans live under `plans/<one-word-name>/`, where the directory
name is short, lowercase, and descriptive.

## Required shape

- Create `plans/<one-word-name>/plan.md` as the entry point.
- Split implementation phases into separate files in the same directory, for example
  `phase-1.md`, `phase-2.md`, and `phase-3.md`.
- In `plan.md`, include a plain-language three sentence summary of each phase.
- In `plan.md`, include the overall constraints and important considerations that apply across
  every phase.
- In `plan.md`, state that after implementing each phase, the implementing agent must provide a
  handoff message describing what the next agent should do and what should be manually tested.
  Manual testing notes should cover core features, not an exhaustive test matrix.
- Each phase should be implemented and committed on its own branch, then pushed as an owned PR with
  auto-merge armed.
- Serial phase work must wait for a definite PR merge and verify the phase head is reachable from
  `origin/main` before the next phase begins.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.
- For executor-only automation, `scripts/phase-runner.sh --pr` can run existing phase files in
  isolated worktrees, push owned PRs, and arm auto-merge. Use `--pr --wait` for serial phase ranges
  that must wait for each PR to merge before starting the next phase. Planning and final review
  remain manual.

See [plans/README.md](../../plans/README.md) for the full convention.
