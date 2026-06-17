# Agents SDK Phase Runner Plan

## Status

Done.

## Summary

The maintained phase-runner path is the OpenAI Agents SDK-capable Node runner, not a Rust rewrite.
`scripts/phase-runner.sh` is the stable operator entrypoint and dispatches to
`scripts/phase-runner-agents.mjs`; the old shell behavior is preserved by matching the current
phase parsing, dry-run, Codex execution, PR, wait, handoff, and timing contracts. The default
executor remains `codex-cli` for exact parity, while `--executor agents-sdk` provides the OpenAI
Agents SDK and Codex MCP path for future agent orchestration work.

## Current Behavior Preserved

- The runner is executor-only. It runs approved `plans/<name>/phase-*.md` files and must not create
  plans or perform final review.
- Normal execution is PR-first: `--plan NAME` plus explicit phases or `--from/--to`, with `--pr`
  required and `--wait` optional. Without `--wait`, the runner opens and arms the first PR, then
  stops with a pending handoff.
- `--base` exists only for compatibility and must be `main`. Non-dry execution must start from a
  clean local `main` checkout with an `origin` remote.
- Phase ids accept `N`, `N.M`, `Na`, and `phase-*` forms. Range discovery sorts matching phase
  files, excludes the `--from` phase, includes the `--to` phase, and supports decimal interstitials
  such as `5.5` plus suffixed phases such as `3a`.
- Worktrees live under `RTS_WORKTREE_ROOT` or `/tmp/rts-worktrees`. Each phase uses
  `zvorygin/<plan>-<phase>` and `<worktree-root>/<plan>-<phase>`.
- The default executor path still invokes `codex exec` with the repo-local `$phase-runner` prompt,
  the output schema, and an output handoff JSON file under the runner log directory.
- Completion is accepted only when the handoff status is `completed`, the worktree is clean, at
  least one commit exists over the recorded base, and the phase document is marked done using one
  of the accepted completion marker forms.
- The runner pushes the phase branch, writes a PR body from the handoff, invokes
  `scripts/agent-pr.sh`, verifies the PR is open, auto-merge is armed, and merge state is not dirty.
- With `--wait`, the runner invokes `scripts/wait-pr.sh`, fetches `origin/main`, verifies the phase
  head is reachable from `origin/main`, syncs local `main`, then continues to the next phase.
- `--dry-run` creates no worktrees, opens no PRs, prints the planned worktree/PR actions, prints the
  rendered prompt, and continues across discovered phases only when `--wait` is set.

## Maintained Architecture

- `scripts/phase-runner.sh` is the stable command for operators and agents.
- `scripts/phase-runner-agents.mjs` owns the maintained implementation.
- The root `package.json` pins `@openai/agents` and `zod` for the Agents SDK path.
- `--executor codex-cli` is the default parity executor and remains the safest production path.
- `--executor agents-sdk` uses the OpenAI Agents SDK with Codex MCP so future work can add richer
  agent orchestration, handoffs, MCP tool composition, tracing, and eventually sub-agent fan-out.
- `scripts/agent-pr.sh` and `scripts/wait-pr.sh` continue to own the repository PR lifecycle policy.
- `scripts/phase-runner-result.schema.json` remains the structured handoff contract for executor
  prompts and runner validation.

## Phase Summaries

1. Phase 1 captures the current runner contract in testable JavaScript units. It covers phase id
   parsing, range discovery, completion marker detection, handoff helpers, PR readiness checks, and
   prompt rendering. The outcome is a behavior model that can be tested without running Codex or
   touching GitHub.
2. Phase 2 implements the side-by-side Node runner and keeps the old operator path untouched while
   parity is being verified. It preserves dry-run output, branch/worktree/log layout, prompt text,
   PR body generation, and handoff enrichment. The outcome is a maintained runner that can be
   compared against the old script before cutover.
3. Phase 3 adds the OpenAI Agents SDK and Codex MCP executor mode without making it the default
   production path. It keeps `codex-cli` as the parity executor and adds `--executor agents-sdk`
   for future agentic workflows. The outcome is a runner that supports the chosen orchestration
   stack while keeping existing phase execution stable.
4. Phase 4 cuts the stable `scripts/phase-runner.sh` entrypoint over to the Agents SDK-capable Node
   runner. It removes the superseded native runner from the active workspace so future work does
   not split between competing implementations. The outcome is one maintained runner path and a
   clear language/tooling direction.
5. Phase 5 hardens docs and follow-up guidance. It updates planning docs, repo agent instructions,
   and runner notes so future executor work extends `scripts/phase-runner-agents.mjs` instead of
   reviving the Rust plan. The outcome is a coherent workflow surface for later prompt injection,
   experimental local iteration, repair/resume inspection, and sub-agent orchestration.

## Constraints

- Keep the existing PR-first contract until a deliberate follow-up changes it.
- Keep phase-runner execution separate from plan creation and final review.
- Do not duplicate PR policy inside the runner while `scripts/agent-pr.sh` and `scripts/wait-pr.sh`
  remain the repo-owned lifecycle helpers.
- Keep `scripts/phase-runner.sh` as the stable operator entrypoint.
- Preserve failure inspectability. On executor failure, blocked handoff, dirty worktree, missing
  commit, missing done marker, PR lifecycle failure, or wait failure, the runner must print the
  relevant path and leave the worktree/logs available for repair.
- Avoid broad local test bundles. Runner changes should use focused Node tests and dry-run parity
  checks first; the PR full gate remains authoritative.

## Follow-Up Capabilities

- Prompt-section injection for plan-specific warnings, product constraints, or temporary
  verification instructions.
- Experimental local iteration mode that creates or reuses a worktree and Codex prompt but does
  not push, open a PR, or mark the phase complete.
- Repair/resume commands that inspect an active marker, handoff JSON, branch, and worktree to
  report the exact blocked state.
- Agents SDK tracing and richer handoff state once the MCP executor path is proven in canaries.
- Sub-agent fan-out for independent phase substeps after ownership and worktree boundaries are
  explicit enough to avoid conflicting edits.

## Phase Index

1. [Phase 1 - Behavior Model and Tests](phase-1.md)
2. [Phase 2 - Side-by-Side Node Runner](phase-2.md)
3. [Phase 3 - Agents SDK Executor Mode](phase-3.md)
4. [Phase 4 - Stable Entrypoint Cutover](phase-4.md)
5. [Phase 5 - Docs and Follow-Up Guidance](phase-5.md)
