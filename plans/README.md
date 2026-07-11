# Phased plan convention

Use this directory for multi-phase or phased implementation plans. Each plan gets its own
`plans/<one-word-name>/` directory, with a short lowercase directory name that is easy to reference
in later tasks.

Reusable methodology belongs in `docs/`, not in active plan directories. Hotspot scoring,
architectural group tracking, and before/after cleanup comparison live in
[`docs/hotspot-analysis.md`](../docs/hotspot-analysis.md); completed hotspot evidence lives under
`plans/archive/hotspots/`.

Each plan directory must contain a brief `plan.md` entry point and one file per phase. Use simple
phase filenames such as `phase-1.md`, `phase-2.md`, and `phase-3.md` unless a more specific name is
clearer.

`plan.md` must include:

- A plain-language three sentence summary of each phase.
- Overall constraints and important considerations that apply across the whole effort.
- A requirement that, after implementing each phase, the agent provides a handoff message for the
  next agent.
- A requirement that each handoff message names the core features that should be manually tested.
  This should not be a comprehensive test matrix.
- A requirement to push each phase branch as an owned PR with auto-merge armed.
- A requirement that after opening each PR, the implementing agent waits for a definite PR merge and
  verifies the phase head is reachable from `origin/main` before reporting the phase complete or
  starting the next phase.

Each phase document should describe its scope, expected code or documentation touch points,
verification, manual testing focus, and handoff expectations. When a phase is complete, mark that
phase document as done in the implementation commit for that phase.

## Archive policy

Archived plans under `plans/archive/` are historical evidence only. Active scripts, tests, and
design docs must not read archived phase files as source-of-truth inputs for current product policy,
allowlists, lifecycle matrices, or checker budgets. If an archived rule is still valid, copy it into
the current plan or relevant `docs/design/*` source-of-truth file and point automation at that
active file instead.

`scripts/agent-pr.sh` automatically moves a newly completed plan to `plans/archive/<name>/` and
commits the move before opening or updating the final phase PR. The guard is intentionally
transition-based: at least one phase must change from incomplete on `origin/main` to done on the
branch, and every phase file anywhere under that active plan must be done. Already-completed active
plans are not swept into the archive merely because an unrelated PR runs the helper.

## Executor runner

For unattended executor passes, use `scripts/phase-runner.sh` from a clean checkout. That stable
script is now a compatibility launcher for the maintained Node runner in
`scripts/phase-runner-agents.mjs`; set `RTS_PHASERUNNER_BIN` only when testing an alternate local
runner binary or script.
The runner creates one `/tmp/rts-worktrees` worktree and one `zvorygin/` branch per phase, invokes
Codex with the repo-local `$phase-runner` skill, saves a compact JSON handoff under the runner log
directory, and commits completed phase work. Pass `--pr` to push the phase branch, open or update an
owned PR with `scripts/agent-pr.sh`, and arm auto-merge. Add `--wait` for normal unattended
completion; the runner will wait through `scripts/wait-pr.sh`, fetch `origin/main`, and verify the
phase head is reachable there before reporting success or starting another phase.

The runner is only for implementation phases that already have approved phase files. It does not
create plans or perform final review. Without `--wait`, the runner stops after the first PR is
armed; treat that as a pending handoff until `scripts/wait-pr.sh <pr>` confirms the merge. Examples:

```bash
scripts/phase-runner.sh --plan ci 5 --pr
scripts/phase-runner.sh --plan ci --from 4 --to 6 --pr
scripts/phase-runner.sh --plan ci --from 4 --to 6 --pr --wait
```

Prefer explicit phase ids when a requested chain includes `phase-0`, decimal phases such as
`phase-2.5`, named phases, or any first phase that must be included. `--from PHASE --to PHASE`
discovers phases strictly after `--from` and through `--to`; name the first phase explicitly when
inclusion matters.

`scripts/phase-runner-result.schema.json` remains the committed structured-handoff contract used by
the runner and by executor prompts. Keep the runner handoff validation and this schema in sync when
the handoff shape changes. Intended follow-up extension points live in
`scripts/phase-runner-agents.mjs`: prompt-section injection, an experimental local iteration mode
that does not open PRs, repair/resume inspection for blocked worktrees, and sub-agent
orchestration.

We tried a live `--executor agents-sdk` canary for the phase runner and removed it because the
OpenAI Agents SDK requires API credentials and bills through API usage. This repo's maintained
executor path should stay on `codex exec`, which uses the developer's Codex CLI login instead of
API-billed SDK calls.
