# Multi-phase planning capsule

Use this capsule whenever a task asks for a multi-phase plan, phased plan, implementation sequence,
or similar staged handoff. Phased plans live under `plans/<one-word-name>/`, where the directory
name is short, lowercase, and descriptive.

Reusable analysis methods do not belong in active plan directories. For hotspot scoring,
architectural group tracking, and before/after cleanup comparisons, use
[docs/hotspot-analysis.md](../hotspot-analysis.md), then create a phased plan only when the current
evidence supports implementation work.

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
- After opening each PR, the implementing agent must wait for a definite PR merge and verify the
  phase head is reachable from `origin/main` before reporting the phase complete or starting the
  next phase.
- When a phase is complete, mark that phase document as done in the implementation commit for that
  phase.
- When that update makes every phase in the plan done, `scripts/agent-pr.sh` moves the plan under
  `plans/archive/` in a follow-up commit before opening or updating the final phase PR. The helper
  archives only plans completed by the current branch; it does not sweep older completed plans.
- For executor-only automation, `scripts/phase-runner.sh --pr` can run existing phase files in
  isolated worktrees, push owned PRs, and arm auto-merge. The script is the stable operator path and
  launches the maintained Node runner in `scripts/phase-runner-agents.mjs`. Use `--pr --wait`
  for normal unattended completion so the runner waits for each PR to merge before reporting
  success. Planning and final review remain manual.
- Prefer explicit phase ids when the requested chain includes `phase-0`, decimal phases such as
  `phase-2.5`, named phases, or any first phase that must be included. `--from PHASE --to PHASE`
  discovers phases strictly after `--from` and through `--to`; name the first phase explicitly when
  inclusion matters.
- Recovery, cleanup, canary, and alternate-runner procedures for PR-first phase work live in
  [docs/pr-first-workflow.md](../pr-first-workflow.md).

See [plans/README.md](../../plans/README.md) for the full convention.
