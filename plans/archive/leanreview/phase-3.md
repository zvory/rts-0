# Phase 3 - Sandbox-Honest Verification

## Phase Status

- [x] Done.

## Objective

Stop the `workspace-write` adversarial child from spending turns attempting listener, browser, and
Interact operations that predictably fail. Give it the implementing agent's concise verification
evidence, keep its own verification offline and deterministic, and require it to distinguish a real
evidence gap from a known sandbox limitation.

## Work

- Forward the existing `scripts/agent-pr.sh --verification` text into the quality-pass invocation as
  bounded review context. Preserve its current PR-body use and reject or truncate unexpectedly large
  values at a documented limit rather than expanding the prompt without bound.
- Render a compact `Prior focused verification` section in the adversarial prompt. Treat the text as
  claimed evidence to evaluate against the diff, not as permission to assert tests passed without
  scrutiny.
- Add an explicit verification boundary to the prompt:
  - the child may run offline focused tests, linters, static policy scripts, format checks, and
    repository inspections that work in `workspace-write`;
  - the child must not start HTTP/WebSocket listeners, browsers, Chrome, Interact, Tailnet preview,
    or other validation that needs machine/network access unavailable to this sandbox;
  - the child must not repeat expensive checks already supported by adequate supplied evidence
    merely to make its report longer.
- Keep `workspace-write` and `approval_policy=never`; do not solve this phase by increasing
  permissions.
- Tighten report semantics: known sandbox restrictions are not remaining concerns by themselves.
  If a material behavior lacks adequate supplied evidence and cannot be checked offline, record the
  exact behavior still unverified and why it matters.
- Preserve authority to fix correctness and architecture issues, but state that the review should
  not broaden scope into opportunistic cleanup or a new validation harness.
- Add prompt/rendering tests for supplied evidence, bounds, forbidden validation attempts, honest
  residual risks, and unchanged empty/default behavior.
- Update `docs/design/testing.md` and `docs/pr-first-workflow.md` to assign live/visual validation to
  the implementing agent and offline final review to the adversarial child.

## Expected Touch Points

- `scripts/agent-pr.sh`
- `scripts/adversarial-quality-pass.mjs`
- `tests/adversarial_quality_pass.mjs`
- `docs/design/testing.md`
- `docs/pr-first-workflow.md`

## Required Tests

- A bounded focused-verification summary appears once in the child prompt and remains in the PR
  metadata/report path.
- Missing verification text renders a short explicit `not supplied` value rather than a large
  placeholder.
- Oversized text is rejected or bounded deterministically with clear caller feedback.
- The prompt explicitly forbids listeners, browsers, Chrome, Interact, and Tailnet operations while
  preserving offline focused checks.
- The prompt tells the child to report a specific unverified behavior, not generic `EPERM` or
  sandbox concern text.
- Fake Codex review, full/incremental/no-change selection, push/status ordering, Markdown skip, and
  Phase 1 preflights remain green.

## Verification

- `node tests/adversarial_quality_pass.mjs`
- `bash -n scripts/agent-pr.sh tests/run-all.sh`
- `node --check scripts/adversarial-quality-pass.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Run the helper in dry-run mode with a concise Interact/browser verification summary and inspect the
rendered child invocation or test fixture. Confirm the child receives the evidence but is directed
to offline checks, and confirm a deliberately omitted material visual check produces one specific
residual-risk statement rather than an attempted listener launch or generic sandbox complaint.

## Handoff Expectations

Report the verification-text bound, the exact allowed/forbidden validation boundary, and how the
report distinguishes real evidence gaps from known sandbox limitations. The final handoff should
recommend observing the next ordinary PR batch for preventable CI failures, full versus incremental
review counts, and sandbox-noise reports; do not propose another implementation phase unless that
evidence shows a specific remaining waste.

## Deferred

- Broader sandbox permissions.
- Live browser or Interact execution inside the adversarial child.
- Persisted child-session/token telemetry.
- Automatic verification-command synthesis from changed paths.
