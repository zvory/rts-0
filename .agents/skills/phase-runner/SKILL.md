---
name: phase-runner
description: Execute one planned implementation phase in this RTS repo from an existing plans/<name>/phase-N.md file. Use only for executor passes, not for creating plans, final review, merging, or pushing.
---

# Phase Runner

Use this skill when an existing phased plan under `plans/<name>/` needs an executor pass.

This skill is intentionally narrow. Planning is manual and happens before this skill is used.
Final review is manual and happens after the selected executor passes finish.

## Inputs

- Plan directory: `plans/<name>/`
- Plan entry point: `plans/<name>/plan.md`
- Phase file: `plans/<name>/phase-N.md`
- Optional previous handoff files in `plans/<name>/handoffs/`

## Executor Contract

1. Read `AGENTS.md`, `docs/context/planning.md`, the plan entry point, and the target phase file.
2. Read only the additional code, docs, and tests needed to implement the target phase.
3. Implement only the target phase scope. Do not opportunistically implement later phases.
4. Preserve repo invariants, especially wire protocol, balance mirror, `Game` API, fog, hardening,
   and worktree rules.
5. Run the smallest targeted verification that fits the files or contracts changed.
6. If the phase is complete, mark the phase document as done in the implementation change.
7. Do not merge to `main`, push, open a PR, run final review, or start a new plan.

## Stop Conditions

Stop and report `blocked` instead of forcing progress when any of these happen:

- The target phase is ambiguous or conflicts with the plan.
- The implementation needs a cross-file contract change that the phase did not authorize.
- The change would overlap unrelated work or another agent's likely ownership.
- Targeted verification fails and the fix is not clear within the phase scope.
- The diff expands beyond the named phase.
- Required manual product/design input is missing.

## Handoff Content

At the end of the executor pass, report:

- Whether the phase is `completed` or `blocked`.
- What changed.
- What verification ran and whether it passed.
- Any gameplay or player-facing impact.
- What the next executor should know.
- What a human should manually test later.

Keep the handoff compact. It should help the next executor or final reviewer without becoming a
second plan document.
