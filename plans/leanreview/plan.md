# Lean Agent Review Plan

## Purpose

Cut avoidable Codex usage in the owned-PR workflow without weakening the authoritative GitHub gate
or building a generalized cost-management system. The evidence motivating this plan is narrow: 11
of 13 inspected failed PR runs included a deterministic source-size, faction/architecture, or
deploy-asset failure that existing local checks could catch; 23 of 39 recent code PRs were improved
by adversarial review; and 20 of 40 recent reports mentioned sandbox or listener limitations. The
plan therefore keeps adversarial review, moves cheap failures ahead of it, spends the strongest
review model only on high-risk changes, and stops asking the review sandbox to perform validation it
cannot complete.

## Overall Constraints

- Keep `./tests/run-all.sh` in the GitHub `Main test gate` as the authoritative full-suite signal.
  Do not replace it with local certification or weaken branch protection.
- Optimize only the normal owned-PR path in `scripts/agent-pr.sh` and
  `scripts/adversarial-quality-pass.mjs`. Do not introduce a service, dashboard, scheduler, model
  benchmark framework, or generalized workflow engine.
- Reuse existing policy scripts and changed-path information. New code should be a small pure
  classifier or orchestration helper with focused tests, not a second suite-selection registry.
- Keep the Markdown-only adversarial skip. Unknown non-Markdown paths must receive at least the
  normal review tier rather than silently taking the cheapest path.
- Preserve the clean-worktree requirement, bounded review-input manifest, final-head status,
  durable PR-body report, auto-merge behavior, and full CI gate.
- Keep the adversarial child sandbox at `workspace-write`. Phase 3 removes impossible validation
  attempts instead of granting the review process broad machine or network access.
- Do not add Cargo builds, nextest, live servers, browsers, or Interact to the cheap preflight. Those
  remain focused implementation evidence or GitHub-gate work.
- Each phase must update `docs/design/testing.md` when it changes the owned-PR testing/review
  contract and must extend `tests/adversarial_quality_pass.mjs` rather than adding broad end-to-end
  fixtures.
- Each phase is implemented on its own fresh `zvorygin/` branch, pushed as an owned PR with
  auto-merge armed, and followed through a definite merge. The implementing agent must verify the
  phase head is reachable from `origin/main` before reporting completion or starting the next
  phase.
- Mark the phase document done in its implementation commit. After each phase, provide a compact
  handoff describing what changed, what the next agent should do, and the core workflow behavior
  that should be manually tested.

## Phase Summaries

### [Phase 1 - Cheap Final-Head Preflight](phase-1.md)

Add one small deterministic preflight that runs existing fast policy checks before Codex review and
again on the review's final local head before any push or success status. Fail fast with the exact
command and output, while deliberately excluding Rust compilation, full suites, browsers, and live
servers. Prove with fake Codex/GitHub tests that an invalid branch spends no review tokens and an
invalid review-produced head is never pushed or marked successful.

### [Phase 2 - Conservative Review Tiers](phase-2.md)

Classify non-Markdown diffs into a small low, normal, or high-risk review tier using changed paths
and conservative defaults. Run low-risk work on Terra Medium, normal work on Terra High, and the
authority/security/contract and workflow surfaces on Sol High, passing model and effort explicitly
to the child CLI. Record the selected tier and reason in the durable report so misclassification can
be audited without creating a model-evaluation project.

### [Phase 3 - Sandbox-Honest Verification](phase-3.md)

Pass the implementing agent's concise focused-verification summary into the adversarial review and
state exactly which offline checks the review sandbox may run. Tell the child not to start local
listeners, browsers, Interact, or other sandbox-incompatible validation, and stop reporting their
predictable absence as a newly discovered concern. Preserve honest gaps: if required evidence was
not supplied and cannot be produced offline, the report must name that specific residual risk.

## Phase Index

1. [Phase 1 - Cheap Final-Head Preflight](phase-1.md)
2. [Phase 2 - Conservative Review Tiers](phase-2.md)
3. [Phase 3 - Sandbox-Honest Verification](phase-3.md)

## Success Measures

Use the next ordinary batch of owned PRs as observational evidence; do not duplicate reviews merely
to create a control group.

- Branches failing one of the selected fast checks stop before the first adversarial Codex launch.
- A review-produced final head failing the same checks is not pushed and receives no success status.
- Every non-docs adversarial report records a tier, model, effort, and short classification reason.
- Ordinary changes no longer inherit the user's global Sol High configuration by accident.
- Reports no longer contain generic `EPERM`, localhost-listener, browser-launch, or Interact-launch
  concerns caused solely by the known `workspace-write` sandbox.
- Full-gate failure rate and adversarial improvement rate remain visible through existing PR and
  Actions history; no new telemetry store is required.

## Non-Goals

- Eliminating adversarial review, skipping the full GitHub gate, or auto-merging after local checks.
- Predicting exact token cost, enforcing a token budget, or persisting child-session transcripts.
- Incremental review of only the post-CI delta; that is a useful later optimization, but it is not
  required for these three solid wins.
- Automatically choosing the implementing agent's model or changing the user's global Codex
  configuration.
- Expanding review sandbox permissions or teaching the child to operate Interact/browser tooling.
- Building an exhaustive path-risk ontology. The classifier should remain short, conservative, and
  easy to delete or adjust.

## Implementation Process

After this plan is approved and merged, execute the phases serially:

```bash
scripts/phase-runner.sh --plan leanreview phase-1 --pr --wait
scripts/phase-runner.sh --plan leanreview phase-2 --pr --wait
scripts/phase-runner.sh --plan leanreview phase-3 --pr --wait
```

Do not start a later phase until the prior phase is merged and reachable from `origin/main`.
